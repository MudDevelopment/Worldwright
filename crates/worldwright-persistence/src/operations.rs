use crate::{PersistenceError, types::*};
use serde_json::Value;
use sqlx::{FromRow, Postgres, Transaction};
use worldwright_core::ids::{CharacterId, ItemId};

#[allow(dead_code)]
#[derive(FromRow)]
struct ReceiptRow {
    operation_id: uuid::Uuid,
    intent_kind: String,
    intent_fingerprint: Vec<u8>,
    outcome: String,
    result_version: i16,
    result: Value,
}
#[allow(dead_code)]
#[derive(FromRow)]
struct CharacterRow {
    balance: i64,
    version: i64,
}
#[allow(dead_code)]
#[derive(FromRow)]
struct RewardClaimRow {
    operation_id: uuid::Uuid,
    credits: i64,
    item_id: uuid::Uuid,
}
#[allow(dead_code)]
#[derive(FromRow)]
struct TargetRow {
    character_id: uuid::Uuid,
    character_name: String,
}
#[allow(dead_code)]
#[derive(FromRow)]
struct ItemRow {
    owner_character_id: uuid::Uuid,
    definition_id: String,
    version: i64,
}
#[allow(dead_code)]
#[derive(FromRow)]
struct InventoryRow {
    item_id: uuid::Uuid,
    definition_id: String,
    version: i64,
}

impl crate::store::PostgresStore {
    pub async fn claim_reward(
        &self,
        lease: WriterLease,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        spec: RewardSpec<'_>,
    ) -> Result<RewardResult, PersistenceError> {
        self.claim_reward_inner(lease, actor, request_id, operation, spec, false)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn claim_reward_fault_after_value_for_test(
        &self,
        lease: WriterLease,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        spec: RewardSpec<'_>,
    ) -> Result<RewardResult, PersistenceError> {
        self.claim_reward_inner(lease, actor, request_id, operation, spec, true)
            .await
    }

    async fn claim_reward_inner(
        &self,
        lease: WriterLease,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        spec: RewardSpec<'_>,
        fault_after_value: bool,
    ) -> Result<RewardResult, PersistenceError> {
        validate_actor_input(lease, actor, request_id)?;
        validate_reward_key(spec.key)?;
        validate_definition(spec.item_definition)?;
        let credits = i64_from_u64(spec.credits, true)?;
        let generation = i64_from_u64(spec.generation, true)?;
        let fingerprint = fingerprint(&[
            b"claim_reward",
            spec.key.as_bytes(),
            &u64_bytes(spec.generation),
            &u64_bytes(spec.credits),
            spec.item_definition.as_bytes(),
        ]);
        let mut tx = self.pool().begin().await?;
        self.fence(&mut tx, lease).await?;
        self.advisory(&mut tx, actor, request_id).await?;
        self.authorize(&mut tx, actor).await?;
        if let Some(receipt) = self.receipt(&mut tx, actor, request_id).await? {
            return replay_reward(receipt, fingerprint, operation);
        }
        let character: CharacterRow = sqlx::query_as("SELECT balance, version FROM worldwright_characters WHERE character_id = $1 AND account_id = $2 AND world_id = $3 FOR UPDATE").bind(uuid_of(actor.character)).bind(uuid_of(actor.account)).bind(uuid_of(actor.world)).fetch_optional(&mut *tx).await?.ok_or(PersistenceError::Unauthorized)?;
        let balance = u64_from_i64(character.balance, false)?;
        let _character_version = u64_from_i64(character.version, true)?;
        let claim: Option<RewardClaimRow> = sqlx::query_as("SELECT operation_id, credits, item_id FROM worldwright_reward_claims WHERE world_id = $1 AND character_id = $2 AND reward_key = $3 AND completion_generation = $4").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).bind(spec.key).bind(generation).fetch_optional(&mut *tx).await?;
        let (outcome, item, final_balance) = if let Some(claim) = claim {
            let _claim_operation = operation_id(claim.operation_id)?;
            let _claim_credits = u64_from_i64(claim.credits, true)?;
            let claim_item = item_id(claim.item_id)?;
            (RewardOutcome::AlreadyClaimed, Some(claim_item), balance)
        } else {
            let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM worldwright_item_instances WHERE world_id = $1 AND owner_character_id = $2").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).fetch_one(&mut *tx).await?;
            if u64_from_i64(count, false)? as usize >= MAX_DURABLE_INVENTORY_ITEMS {
                (RewardOutcome::InventoryFull, None, balance)
            } else {
                sqlx::query("INSERT INTO worldwright_item_instances ( item_id, world_id, definition_id, owner_character_id ) VALUES ($1, $2, $3, $4)").bind(uuid_of(spec.item)).bind(uuid_of(actor.world)).bind(spec.item_definition).bind(uuid_of(actor.character)).execute(&mut *tx).await?;
                let row: (i64, i64) = sqlx::query_as("UPDATE worldwright_characters SET balance = balance + $4, version = version + 1, updated_at = clock_timestamp() WHERE character_id = $1 AND account_id = $2 AND world_id = $3 AND balance <= 9223372036854775807 - $4 RETURNING balance, version").bind(uuid_of(actor.character)).bind(uuid_of(actor.account)).bind(uuid_of(actor.world)).bind(credits).fetch_one(&mut *tx).await?;
                let final_balance = u64_from_i64(row.0, false)?;
                let _final_version = u64_from_i64(row.1, true)?;
                (RewardOutcome::RewardGranted, Some(spec.item), final_balance)
            }
        };
        let result = RewardResult {
            operation,
            outcome,
            balance: final_balance,
            item,
            replayed: false,
        };
        let value = serde_json::to_value(&result).map_err(|_| PersistenceError::InvalidData)?;
        if fault_after_value {
            return Err(PersistenceError::Database(sqlx::Error::Protocol(
                "test fault after durable value mutation".into(),
            )));
        }
        self.insert_receipt(
            &mut tx,
            actor,
            request_id,
            operation,
            "claim_reward",
            &fingerprint,
            outcome_name_reward(outcome),
            &value,
        )
        .await?;
        if outcome == RewardOutcome::RewardGranted {
            sqlx::query("INSERT INTO worldwright_reward_claims ( world_id, character_id, reward_key, completion_generation, operation_id, credits, item_id ) VALUES ($1, $2, $3, $4, $5, $6, $7)").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).bind(spec.key).bind(generation).bind(uuid_of(operation)).bind(credits).bind(uuid_of(spec.item)).execute(&mut *tx).await?;
        }
        self.insert_audit(
            &mut tx,
            actor,
            request_id,
            operation,
            "claim_reward",
            outcome_name_reward(outcome),
            &value,
        )
        .await?;
        if outcome == RewardOutcome::RewardGranted {
            self.insert_outbox(&mut tx, actor.world, operation, "reward_claimed", &value)
                .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(result)
    }

    pub async fn transfer_item(
        &self,
        lease: WriterLease,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        spec: TransferSpec<'_>,
    ) -> Result<TransferResult, PersistenceError> {
        validate_actor_input(lease, actor, request_id)?;
        validate_username(spec.recipient_username)?;
        let fingerprint = fingerprint(&[
            b"transfer_item",
            &u128_bytes(spec.item),
            spec.recipient_username.as_bytes(),
        ]);
        let mut tx = self.pool().begin().await?;
        self.fence(&mut tx, lease).await?;
        self.advisory(&mut tx, actor, request_id).await?;
        self.authorize(&mut tx, actor).await?;
        if let Some(receipt) = self.receipt(&mut tx, actor, request_id).await? {
            return replay_transfer(receipt, fingerprint, operation, actor, spec.item);
        }
        let target: Option<TargetRow> = sqlx::query_as("SELECT c.character_id, c.character_name FROM worldwright_accounts AS a JOIN worldwright_characters AS c ON c.account_id = a.account_id WHERE a.username = $1 AND c.world_id = $2 AND a.disabled = FALSE").bind(spec.recipient_username).bind(uuid_of(actor.world)).fetch_optional(&mut *tx).await?;
        let (outcome, owner, recipient_name) = if target.is_none() {
            (TransferOutcome::TargetNotFound, None, None)
        } else {
            let target = target.expect("checked");
            let target_is_actor = target.character_id == uuid_of(actor.character);
            let locked: Vec<(uuid::Uuid,)> = sqlx::query_as("SELECT character_id FROM worldwright_characters WHERE world_id = $1 AND (character_id = $2 OR character_id = $3) ORDER BY character_id FOR UPDATE").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).bind(target.character_id).fetch_all(&mut *tx).await?;
            let expected_locked = if target_is_actor { 1 } else { 2 };
            if locked.len() != expected_locked {
                return Err(PersistenceError::InvalidData);
            }
            let item: Option<ItemRow> = sqlx::query_as("SELECT owner_character_id, definition_id, version FROM worldwright_item_instances WHERE world_id = $1 AND item_id = $2 FOR UPDATE").bind(uuid_of(actor.world)).bind(uuid_of(spec.item)).fetch_optional(&mut *tx).await?;
            match item {
                None => (TransferOutcome::NotOwned, None, Some(target.character_name)),
                Some(item) if item.owner_character_id != uuid_of(actor.character) => {
                    (TransferOutcome::NotOwned, None, Some(target.character_name))
                }
                Some(_item) if target_is_actor => (
                    TransferOutcome::SameOwner,
                    Some(actor.character),
                    Some(target.character_name),
                ),
                Some(item) => {
                    let item_version = u64_from_i64(item.version, true)?;
                    validate_definition(&item.definition_id)?;
                    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM worldwright_item_instances WHERE world_id = $1 AND owner_character_id = $2").bind(uuid_of(actor.world)).bind(target.character_id).fetch_one(&mut *tx).await?;
                    if u64_from_i64(count, false)? as usize >= MAX_DURABLE_INVENTORY_ITEMS {
                        (
                            TransferOutcome::InventoryFull,
                            Some(actor.character),
                            Some(target.character_name),
                        )
                    } else {
                        let updated: Vec<(i64,)> = sqlx::query_as("UPDATE worldwright_item_instances SET owner_character_id = $3, version = version + 1, updated_at = clock_timestamp() WHERE world_id = $1 AND item_id = $2 AND owner_character_id = $4 AND version = $5 RETURNING version").bind(uuid_of(actor.world)).bind(uuid_of(spec.item)).bind(target.character_id).bind(uuid_of(actor.character)).bind(i64_from_u64(item_version, true)?).fetch_all(&mut *tx).await?;
                        if updated.len() != 1 || u64_from_i64(updated[0].0, true).is_err() {
                            return Err(PersistenceError::InvalidData);
                        }
                        let changed = sqlx::query("UPDATE worldwright_characters SET version = version + 1, updated_at = clock_timestamp() WHERE world_id = $1 AND (character_id = $2 OR character_id = $3)").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).bind(target.character_id).execute(&mut *tx).await?;
                        if changed.rows_affected() != 2 {
                            return Err(PersistenceError::InvalidData);
                        }
                        (
                            TransferOutcome::ItemTransferred,
                            Some(character_id(target.character_id)?),
                            Some(target.character_name),
                        )
                    }
                }
            }
        };
        let result = TransferResult {
            operation,
            outcome,
            item: spec.item,
            owner,
            recipient_name,
            replayed: false,
        };
        let value = serde_json::to_value(&result).map_err(|_| PersistenceError::InvalidData)?;
        self.insert_receipt(
            &mut tx,
            actor,
            request_id,
            operation,
            "transfer_item",
            &fingerprint,
            outcome_name_transfer(outcome),
            &value,
        )
        .await?;
        self.insert_audit(
            &mut tx,
            actor,
            request_id,
            operation,
            "transfer_item",
            outcome_name_transfer(outcome),
            &value,
        )
        .await?;
        if outcome == TransferOutcome::ItemTransferred {
            self.insert_outbox(&mut tx, actor.world, operation, "item_transferred", &value)
                .await?;
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(result)
    }

    pub async fn inventory(
        &self,
        actor: &AuthenticatedActor<'_>,
    ) -> Result<InventorySnapshot, PersistenceError> {
        let mut tx = self.pool().begin().await?;
        self.authorize(&mut tx, actor).await?;
        let (balance, version, _room): (i64, i64, String) = sqlx::query_as("SELECT balance, version, room_definition FROM worldwright_characters WHERE character_id = $1 AND account_id = $2 AND world_id = $3").bind(uuid_of(actor.character)).bind(uuid_of(actor.account)).bind(uuid_of(actor.world)).fetch_optional(&mut *tx).await?.ok_or(PersistenceError::Unauthorized)?;
        let rows: Vec<InventoryRow> = sqlx::query_as("SELECT item_id, definition_id, version FROM worldwright_item_instances WHERE world_id = $1 AND owner_character_id = $2 ORDER BY item_id LIMIT 257").bind(uuid_of(actor.world)).bind(uuid_of(actor.character)).fetch_all(&mut *tx).await?;
        if rows.len() > MAX_DURABLE_INVENTORY_ITEMS {
            return Err(PersistenceError::InvalidData);
        }
        Ok(InventorySnapshot {
            balance: u64_from_i64(balance, false)?,
            character_version: u64_from_i64(version, true)?,
            items: rows
                .into_iter()
                .map(|row| {
                    validate_definition(&row.definition_id)?;
                    Ok(InventoryItem {
                        item: item_id(row.item_id)?,
                        definition: row.definition_id,
                        version: u64_from_i64(row.version, true)?,
                    })
                })
                .collect::<Result<_, PersistenceError>>()?,
        })
    }
}

impl crate::store::PostgresStore {
    pub(crate) fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
    async fn fence(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        lease: WriterLease,
    ) -> Result<(), PersistenceError> {
        let row: Option<(i64, Option<uuid::Uuid>)> = sqlx::query_as("SELECT writer_epoch, active_writer_id FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE").bind(uuid_of(lease.world)).fetch_optional(&mut **tx).await?;
        let Some((epoch, writer)) = row else {
            return Err(PersistenceError::StaleWriter);
        };
        if u64_from_i64(epoch, true)? != lease.epoch || writer != Some(uuid_of(lease.writer)) {
            return Err(PersistenceError::StaleWriter);
        }
        Ok(())
    }
    async fn advisory(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
    ) -> Result<(), PersistenceError> {
        let key = advisory_scope_key(actor, request_id);
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(key)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
    async fn authorize(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &AuthenticatedActor<'_>,
    ) -> Result<(), PersistenceError> {
        let row: Option<(uuid::Uuid,)> = sqlx::query_as("SELECT s.account_id FROM worldwright_sessions AS s JOIN worldwright_accounts AS a ON a.account_id = s.account_id JOIN worldwright_characters AS c ON c.character_id = s.character_id AND c.account_id = s.account_id AND c.world_id = s.world_id WHERE s.token_hash = $1 AND s.account_id = $2 AND s.character_id = $3 AND s.world_id = $4 AND s.revoked_at IS NULL AND s.expires_at > clock_timestamp() AND a.disabled = FALSE FOR SHARE OF s, a, c").bind(actor.token_hash.as_slice()).bind(uuid_of(actor.account)).bind(uuid_of(actor.character)).bind(uuid_of(actor.world)).fetch_optional(&mut **tx).await?;
        row.ok_or(PersistenceError::Unauthorized).map(|_| ())
    }
    async fn receipt(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
    ) -> Result<Option<ReceiptRow>, PersistenceError> {
        Ok(sqlx::query_as("SELECT operation_id, intent_kind, intent_fingerprint, outcome, result_version, result FROM worldwright_operation_receipts WHERE world_id = $1 AND account_id = $2 AND character_id = $3 AND request_id = $4 FOR UPDATE").bind(uuid_of(actor.world)).bind(uuid_of(actor.account)).bind(uuid_of(actor.character)).bind(request_id).fetch_optional(&mut **tx).await?)
    }
    #[allow(clippy::too_many_arguments)]
    async fn insert_receipt(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        kind: &str,
        fingerprint: &[u8; 32],
        outcome: &str,
        result: &Value,
    ) -> Result<(), PersistenceError> {
        sqlx::query("INSERT INTO worldwright_operation_receipts ( world_id, account_id, character_id, request_id, operation_id, intent_kind, intent_fingerprint, outcome, result, result_version ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)").bind(uuid_of(actor.world)).bind(uuid_of(actor.account)).bind(uuid_of(actor.character)).bind(request_id).bind(uuid_of(operation)).bind(kind).bind(fingerprint.as_slice()).bind(outcome).bind(result).execute(&mut **tx).await?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    async fn insert_audit(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &AuthenticatedActor<'_>,
        request_id: &str,
        operation: worldwright_core::ids::OperationId,
        action: &str,
        outcome: &str,
        summary: &Value,
    ) -> Result<(), PersistenceError> {
        sqlx::query("INSERT INTO worldwright_audit_log ( world_id, account_id, character_id, operation_id, request_id, action, outcome, summary ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)").bind(uuid_of(actor.world)).bind(uuid_of(actor.account)).bind(uuid_of(actor.character)).bind(uuid_of(operation)).bind(request_id).bind(action).bind(outcome).bind(summary).execute(&mut **tx).await?;
        Ok(())
    }
    async fn insert_outbox(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        world: worldwright_core::ids::WorldId,
        operation: worldwright_core::ids::OperationId,
        topic: &str,
        payload: &Value,
    ) -> Result<(), PersistenceError> {
        sqlx::query("INSERT INTO worldwright_outbox ( operation_id, world_id, topic, payload ) VALUES ($1, $2, $3, $4)").bind(uuid_of(operation)).bind(uuid_of(world)).bind(topic).bind(payload).execute(&mut **tx).await?;
        Ok(())
    }
}

fn advisory_scope_key(actor: &AuthenticatedActor<'_>, request_id: &str) -> String {
    format!(
        "32:{}|32:{}|32:{}|{}:{}",
        actor.world,
        actor.account,
        actor.character,
        request_id.len(),
        request_id,
    )
}

fn validate_actor_input(
    lease: WriterLease,
    actor: &AuthenticatedActor<'_>,
    request_id: &str,
) -> Result<(), PersistenceError> {
    if lease.world != actor.world || actor.token_hash.len() != 32 {
        return Err(PersistenceError::Unauthorized);
    }
    validate_request_id(request_id)
}
fn outcome_name_reward(outcome: RewardOutcome) -> &'static str {
    match outcome {
        RewardOutcome::RewardGranted => "reward_granted",
        RewardOutcome::AlreadyClaimed => "already_claimed",
        RewardOutcome::InventoryFull => "inventory_full",
    }
}
fn reward_result_shape(result: &RewardResult) -> bool {
    match result.outcome {
        RewardOutcome::RewardGranted | RewardOutcome::AlreadyClaimed => {
            result.item.is_some() && result.balance > 0
        }
        RewardOutcome::InventoryFull => result.item.is_none(),
    }
}
fn transfer_result_shape(
    result: &TransferResult,
    actor: CharacterId,
    requested_item: ItemId,
) -> bool {
    if result.item != requested_item {
        return false;
    }
    match result.outcome {
        TransferOutcome::ItemTransferred => {
            result.owner.is_some_and(|owner| owner != actor) && result.recipient_name.is_some()
        }
        TransferOutcome::NotOwned => result.owner.is_none(),
        TransferOutcome::SameOwner => {
            result.owner == Some(actor) && result.recipient_name.is_some()
        }
        TransferOutcome::TargetNotFound => {
            result.owner.is_none() && result.recipient_name.is_none()
        }
        TransferOutcome::InventoryFull => {
            result.owner == Some(actor) && result.recipient_name.is_some()
        }
    }
}
fn outcome_name_transfer(outcome: TransferOutcome) -> &'static str {
    match outcome {
        TransferOutcome::ItemTransferred => "item_transferred",
        TransferOutcome::NotOwned => "not_owned",
        TransferOutcome::SameOwner => "same_owner",
        TransferOutcome::TargetNotFound => "target_not_found",
        TransferOutcome::InventoryFull => "inventory_full",
    }
}
fn replay_reward(
    row: ReceiptRow,
    fingerprint: [u8; 32],
    _operation: worldwright_core::ids::OperationId,
) -> Result<RewardResult, PersistenceError> {
    if row.intent_kind != "claim_reward"
        || row.intent_fingerprint != fingerprint
        || row.result_version != RECEIPT_FORMAT_VERSION
    {
        return Err(if row.intent_fingerprint == fingerprint {
            PersistenceError::InvalidData
        } else {
            PersistenceError::ReceiptConflict
        });
    }
    let result: RewardResult =
        serde_json::from_value(row.result).map_err(|_| PersistenceError::InvalidData)?;
    if result.replayed
        || operation_id(row.operation_id)? != result.operation
        || row.outcome != outcome_name_reward(result.outcome)
        || !reward_result_shape(&result)
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(RewardResult {
        replayed: true,
        ..result
    })
}
fn replay_transfer(
    row: ReceiptRow,
    fingerprint: [u8; 32],
    _operation: worldwright_core::ids::OperationId,
    actor: &AuthenticatedActor<'_>,
    requested_item: ItemId,
) -> Result<TransferResult, PersistenceError> {
    if row.intent_kind != "transfer_item"
        || row.intent_fingerprint != fingerprint
        || row.result_version != RECEIPT_FORMAT_VERSION
    {
        return Err(if row.intent_fingerprint == fingerprint {
            PersistenceError::InvalidData
        } else {
            PersistenceError::ReceiptConflict
        });
    }
    let result: TransferResult =
        serde_json::from_value(row.result).map_err(|_| PersistenceError::InvalidData)?;
    if result.replayed
        || operation_id(row.operation_id)? != result.operation
        || row.outcome != outcome_name_transfer(result.outcome)
        || !transfer_result_shape(&result, actor.character, requested_item)
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(TransferResult {
        replayed: true,
        ..result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_scope_is_unambiguous_and_exact() {
        let actor = AuthenticatedActor {
            world: worldwright_core::ids::WorldId::from_u128(1).unwrap(),
            account: worldwright_core::ids::AccountId::from_u128(2).unwrap(),
            character: worldwright_core::ids::CharacterId::from_u128(3).unwrap(),
            token_hash: &[0; 32],
        };
        assert_eq!(
            advisory_scope_key(&actor, "a:b"),
            "32:00000000000000000000000000000001|32:00000000000000000000000000000002|32:00000000000000000000000000000003|3:a:b"
        );
    }

    #[test]
    fn fatal_result_corruption_is_rejected() {
        let operation = worldwright_core::ids::OperationId::from_u128(1).unwrap();
        let row = ReceiptRow {
            operation_id: uuid_of(operation),
            intent_kind: "claim_reward".to_owned(),
            intent_fingerprint: vec![7; 32],
            outcome: "reward_granted".to_owned(),
            result_version: RECEIPT_FORMAT_VERSION,
            result: serde_json::json!({
                "operation": operation.to_string(),
                "outcome": "reward_granted",
                "balance": 1,
                "item": null,
                "replayed": true
            }),
        };
        assert!(replay_reward(row, [7; 32], operation).is_err());
    }
}
