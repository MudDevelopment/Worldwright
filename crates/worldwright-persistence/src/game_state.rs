use crate::{PersistenceError, types::*};
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, Transaction};
use std::collections::BTreeSet;
use worldwright_core::{
    ids::{DefinitionId, WorldId},
    runtime::{CounterState, DurableWorldState, MAX_COUNTER_VALUE, ScheduledCounterState},
    time::Tick,
};
use worldwright_release::{GameContract, Sha256Digest};

pub const MAX_DURABLE_GAME_COUNTERS: usize = 4096;
pub const MAX_DURABLE_GAME_RESULT_BYTES: usize = 64 * 1024;
pub const MAX_DURABLE_GAME_INTENT_BYTES: usize = 8 * 1024;

#[derive(FromRow)]
struct GameplayReceiptRow {
    operation_id: uuid::Uuid,
    intent_kind: String,
    intent_fingerprint: Vec<u8>,
    result_version: i16,
    result: Value,
    state_version: i64,
}

type LockedWorldRow = (
    i64,
    Option<uuid::Uuid>,
    uuid::Uuid,
    String,
    i64,
    i64,
    Option<Vec<u8>>,
    bool,
);

impl crate::store::PostgresStore {
    pub async fn load_game_state(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
        game: GameContract,
    ) -> Result<DurableGameSnapshot, PersistenceError> {
        let world = lease.world();
        let mut tx = self.pool().begin().await?;
        let locked = lock_game_world(&mut tx, lease).await?;
        validate_locked_world(&locked, expected_release, game, None)?;
        let state_version = locked.5;
        let (runtime_revision,): (i64,) =
            sqlx::query_as("SELECT runtime_revision FROM worldwright_worlds WHERE world_id = $1")
                .bind(uuid_of(world))
                .fetch_one(&mut *tx)
                .await?;
        let rooms: Vec<(uuid::Uuid, String)> = sqlx::query_as(
            "SELECT character_id, room_definition FROM worldwright_characters \
             WHERE world_id = $1 ORDER BY character_id LIMIT 257",
        )
        .bind(uuid_of(world))
        .fetch_all(&mut *tx)
        .await?;
        if rooms.is_empty() || rooms.len() > 256 {
            return Err(PersistenceError::InvalidData);
        }
        let characters = rooms
            .iter()
            .map(|(character, room)| {
                validate_definition(room)?;
                character_id(*character)
            })
            .collect::<Result<BTreeSet<_>, PersistenceError>>()?;
        if characters.len() != rooms.len() {
            return Err(PersistenceError::InvalidData);
        }
        let rooms = rooms
            .into_iter()
            .map(|(character, room_definition)| {
                Ok(DurableCharacterRoom {
                    character: character_id(character)?,
                    room_definition,
                })
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;

        let counter_rows: Vec<(uuid::Uuid, String, i64)> = sqlx::query_as(
            "SELECT character_id, counter_key, counter_value FROM worldwright_game_counters \
             WHERE world_id = $1 ORDER BY character_id, counter_key LIMIT 4097",
        )
        .bind(uuid_of(world))
        .fetch_all(&mut *tx)
        .await?;
        if counter_rows.len() > MAX_DURABLE_GAME_COUNTERS {
            return Err(PersistenceError::InvalidData);
        }
        let counters = counter_rows
            .into_iter()
            .map(|(character, key, value)| {
                let character = character_id(character)?;
                if !characters.contains(&character) {
                    return Err(PersistenceError::InvalidData);
                }
                Ok(CounterState {
                    character,
                    key: key
                        .parse::<DefinitionId>()
                        .map_err(|_| PersistenceError::InvalidData)?,
                    value: u64_from_i64(value, false)?,
                })
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;

        let scheduled_rows: Vec<(i16, uuid::Uuid, String, i64, i64)> = sqlx::query_as(
            "SELECT schedule_ordinal, character_id, counter_key, delta, \
             GREATEST(0::NUMERIC, CEIL(EXTRACT(EPOCH FROM \
             (due_at - clock_timestamp())) * 1000))::BIGINT AS remaining_millis \
             FROM worldwright_scheduled_counter_adjustments WHERE world_id = $1 \
             ORDER BY due_at, schedule_ordinal LIMIT 129",
        )
        .bind(uuid_of(world))
        .fetch_all(&mut *tx)
        .await?;
        if scheduled_rows.len() > worldwright_core::runtime::MAX_SCHEDULED_ACTIONS {
            return Err(PersistenceError::InvalidData);
        }
        let mut scheduled = Vec::with_capacity(scheduled_rows.len());
        for (index, (ordinal, character, key, delta, remaining)) in
            scheduled_rows.into_iter().enumerate()
        {
            if ordinal != i16::try_from(index).map_err(|_| PersistenceError::InvalidData)? {
                return Err(PersistenceError::InvalidData);
            }
            let character = character_id(character)?;
            if !characters.contains(&character)
                || delta == 0
                || delta.unsigned_abs() > MAX_COUNTER_VALUE
            {
                return Err(PersistenceError::InvalidData);
            }
            scheduled.push(ScheduledCounterState {
                at: Tick::from_millis(u64_from_i64(remaining, false)?),
                character,
                key: key
                    .parse::<DefinitionId>()
                    .map_err(|_| PersistenceError::InvalidData)?,
                delta,
            });
        }
        let snapshot = DurableGameSnapshot {
            game,
            state_version: u64_from_i64(state_version, false)?,
            runtime: DurableWorldState {
                world,
                revision: u64_from_i64(runtime_revision, false)?,
                counters,
                scheduled,
            },
            rooms,
        };
        tx.commit().await.map_err(PersistenceError::Database)?;
        Ok(snapshot)
    }

    pub async fn initialize_game_state(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
        game: GameContract,
        state: GameplayState<'_>,
    ) -> Result<u64, PersistenceError> {
        validate_gameplay_state(lease.world(), state)?;
        let mut tx = self.pool().begin().await?;
        let locked = lock_game_world(&mut tx, lease).await?;
        validate_locked_world(&locked, expected_release, game, Some(0))?;
        lock_exact_characters(&mut tx, lease.world(), state.rooms).await?;
        let (counter_count, schedule_count): (i64, i64) = sqlx::query_as(
            "SELECT \
             (SELECT COUNT(*)::BIGINT FROM worldwright_game_counters WHERE world_id = $1), \
             (SELECT COUNT(*)::BIGINT FROM worldwright_scheduled_counter_adjustments \
              WHERE world_id = $1)",
        )
        .bind(uuid_of(lease.world()))
        .fetch_one(&mut *tx)
        .await?;
        if counter_count != 0 || schedule_count != 0 {
            return Err(PersistenceError::InvalidData);
        }
        replace_game_state(&mut tx, lease.world(), state).await?;
        let changed = sqlx::query(
            "UPDATE worldwright_worlds SET game_state_version = 1, runtime_revision = $2, \
             updated_at = clock_timestamp() WHERE world_id = $1 AND game_state_version = 0",
        )
        .bind(uuid_of(lease.world()))
        .bind(i64_from_u64(state.runtime.revision, false)?)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::InvalidData);
        }
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(1)
    }

    /// Commits owner-driven due work under the same writer/release/version fence.
    /// An uncertain commit stops the owner; reconstruction, not a blind retry,
    /// determines whether the timer transition committed.
    pub async fn commit_gameplay_tick(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
        game: GameContract,
        expected_state_version: u64,
        state: GameplayState<'_>,
    ) -> Result<u64, PersistenceError> {
        validate_gameplay_state(lease.world(), state)?;
        let mut tx = self.pool().begin().await?;
        let locked = lock_game_world(&mut tx, lease).await?;
        validate_locked_world(&locked, expected_release, game, Some(expected_state_version))?;
        lock_exact_characters(&mut tx, lease.world(), state.rooms).await?;
        let next = expected_state_version.checked_add(1).ok_or(PersistenceError::CounterExhausted)?;
        replace_game_state(&mut tx, lease.world(), state).await?;
        let changed = sqlx::query(
            "UPDATE worldwright_worlds SET game_state_version=$3, runtime_revision=$4, \
             updated_at=clock_timestamp() WHERE world_id=$1 AND game_state_version=$2",
        )
        .bind(uuid_of(lease.world()))
        .bind(i64_from_u64(expected_state_version, false)?)
        .bind(i64_from_u64(next, true)?)
        .bind(i64_from_u64(state.runtime.revision, false)?)
        .execute(&mut *tx).await?;
        if changed.rows_affected() != 1 { return Err(PersistenceError::StaleGameState); }
        sqlx::query(
            "INSERT INTO worldwright_gameplay_timer_audit \
             (world_id,state_version,runtime_revision,writer_id,writer_epoch,authority_incarnation) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(uuid_of(lease.world()))
        .bind(i64_from_u64(next, true)?)
        .bind(i64_from_u64(state.runtime.revision, false)?)
        .bind(uuid_of(lease.writer()))
        .bind(i64_from_u64(lease.epoch(), true)?)
        .bind(uuid_of(lease.incarnation()))
        .execute(&mut *tx).await?;
        tx.commit().await.map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(next)
    }

    pub async fn commit_gameplay(
        &self,
        input: GameplayMutation<'_>,
    ) -> Result<GameplayCommit, PersistenceError> {
        validate_request_id(input.request_id)?;
        if input.lease.world() != input.actor.world
            || input.intent.is_empty()
            || input.intent.len() > MAX_DURABLE_GAME_INTENT_BYTES
        {
            return Err(PersistenceError::InvalidData);
        }
        validate_gameplay_state(input.actor.world, input.state)?;
        validate_result(&input.result)?;
        let kind = input.kind.as_str();
        let fingerprint = fingerprint(&[b"gameplay", kind.as_bytes(), input.intent]);
        let mut tx = self.pool().begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(gameplay_advisory_key(&input.actor, input.request_id))
            .execute(&mut *tx)
            .await?;
        let locked = lock_game_world(&mut tx, input.lease).await?;
        validate_locked_world(&locked, input.expected_release, input.game, None)?;
        authorize_gameplay(&mut tx, &input.actor).await?;
        let receipt: Option<GameplayReceiptRow> = sqlx::query_as(
            "SELECT operation_id, intent_kind, intent_fingerprint, result_version, result, \
             state_version FROM worldwright_gameplay_receipts \
             WHERE world_id = $1 AND account_id = $2 AND character_id = $3 \
             AND request_id = $4 FOR UPDATE",
        )
        .bind(uuid_of(input.actor.world))
        .bind(uuid_of(input.actor.account))
        .bind(uuid_of(input.actor.character))
        .bind(input.request_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(receipt) = receipt {
            return replay_gameplay(receipt, kind, fingerprint);
        }
        if u64_from_i64(locked.5, false)? != input.expected_state_version {
            return Err(PersistenceError::StaleGameState);
        }
        lock_exact_characters(&mut tx, input.actor.world, input.state.rooms).await?;
        let next_version = input
            .expected_state_version
            .checked_add(1)
            .ok_or(PersistenceError::CounterExhausted)?;
        replace_game_state(&mut tx, input.actor.world, input.state).await?;
        let changed = sqlx::query(
            "UPDATE worldwright_worlds SET game_state_version = $3, runtime_revision = $4, \
             updated_at = clock_timestamp() WHERE world_id = $1 AND game_state_version = $2",
        )
        .bind(uuid_of(input.actor.world))
        .bind(i64_from_u64(input.expected_state_version, false)?)
        .bind(i64_from_u64(next_version, true)?)
        .bind(i64_from_u64(input.state.runtime.revision, false)?)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PersistenceError::StaleGameState);
        }
        sqlx::query(
            "INSERT INTO worldwright_gameplay_receipts \
             (world_id, account_id, character_id, request_id, operation_id, intent_kind, \
              intent_fingerprint, result, state_version) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(uuid_of(input.actor.world))
        .bind(uuid_of(input.actor.account))
        .bind(uuid_of(input.actor.character))
        .bind(input.request_id)
        .bind(uuid_of(input.operation))
        .bind(kind)
        .bind(fingerprint.as_slice())
        .bind(&input.result)
        .bind(i64_from_u64(next_version, true)?)
        .execute(&mut *tx)
        .await?;
        let summary = json!({
            "counter_count": input.state.runtime.counters.len(),
            "runtime_revision": input.state.runtime.revision,
            "schedule_count": input.state.runtime.scheduled.len()
        });
        sqlx::query(
            "INSERT INTO worldwright_gameplay_audit \
             (world_id, account_id, character_id, operation_id, request_id, action, \
              state_version, summary) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(uuid_of(input.actor.world))
        .bind(uuid_of(input.actor.account))
        .bind(uuid_of(input.actor.character))
        .bind(uuid_of(input.operation))
        .bind(input.request_id)
        .bind(kind)
        .bind(i64_from_u64(next_version, true)?)
        .bind(summary)
        .execute(&mut *tx)
        .await?;
        tx.commit()
            .await
            .map_err(|_| PersistenceError::UnknownOutcome)?;
        Ok(GameplayCommit {
            operation: input.operation,
            state_version: next_version,
            result: input.result,
            replayed: false,
        })
    }
}

async fn lock_game_world(
    tx: &mut Transaction<'_, Postgres>,
    lease: WriterLease,
) -> Result<LockedWorldRow, PersistenceError> {
    let row: Option<LockedWorldRow> = sqlx::query_as(
        "SELECT writer_epoch, active_writer_id, authority_incarnation, game_id, \
         game_contract_version, game_state_version, active_release_digest, release_maintenance \
         FROM worldwright_worlds WHERE world_id = $1 FOR UPDATE",
    )
    .bind(uuid_of(lease.world()))
    .fetch_optional(&mut **tx)
    .await?;
    let row = row.ok_or(PersistenceError::StaleWriter)?;
    if u64_from_i64(row.0, true)? != lease.epoch()
        || row.1 != Some(uuid_of(lease.writer()))
        || row.2 != uuid_of(lease.incarnation())
    {
        return Err(PersistenceError::StaleWriter);
    }
    Ok(row)
}

fn validate_locked_world(
    row: &LockedWorldRow,
    expected_release: Sha256Digest,
    game: GameContract,
    expected_state_version: Option<u64>,
) -> Result<(), PersistenceError> {
    if row.7
        || row.6.as_deref() != Some(expected_release.as_bytes().as_slice())
        || parse_game(&row.3, row.4)? != game
    {
        return Err(PersistenceError::ReleaseConflict);
    }
    if let Some(expected) = expected_state_version {
        if u64_from_i64(row.5, false)? != expected {
            return Err(PersistenceError::StaleGameState);
        }
    }
    Ok(())
}

async fn authorize_gameplay(
    tx: &mut Transaction<'_, Postgres>,
    actor: &AuthenticatedActor<'_>,
) -> Result<(), PersistenceError> {
    let row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT s.account_id FROM worldwright_sessions AS s \
         JOIN worldwright_accounts AS a ON a.account_id = s.account_id \
         JOIN worldwright_characters AS c ON c.character_id = s.character_id \
          AND c.account_id = s.account_id AND c.world_id = s.world_id \
         WHERE s.token_hash = $1 AND s.account_id = $2 AND s.character_id = $3 \
          AND s.world_id = $4 AND s.revoked_at IS NULL \
          AND s.expires_at > clock_timestamp() AND a.disabled = FALSE \
         FOR SHARE OF s, a, c",
    )
    .bind(actor.token_hash.as_slice())
    .bind(uuid_of(actor.account))
    .bind(uuid_of(actor.character))
    .bind(uuid_of(actor.world))
    .fetch_optional(&mut **tx)
    .await?;
    row.ok_or(PersistenceError::Unauthorized).map(|_| ())
}

async fn lock_exact_characters(
    tx: &mut Transaction<'_, Postgres>,
    world: WorldId,
    rooms: &[DurableCharacterRoom],
) -> Result<(), PersistenceError> {
    let actual: Vec<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT character_id FROM worldwright_characters WHERE world_id = $1 \
         ORDER BY character_id FOR UPDATE",
    )
    .bind(uuid_of(world))
    .fetch_all(&mut **tx)
    .await?;
    let expected = rooms
        .iter()
        .map(|room| uuid_of(room.character))
        .collect::<Vec<_>>();
    if actual.len() != expected.len() || actual.iter().map(|row| row.0).ne(expected.iter().copied())
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(())
}

async fn replace_game_state(
    tx: &mut Transaction<'_, Postgres>,
    world: WorldId,
    state: GameplayState<'_>,
) -> Result<(), PersistenceError> {
    for room in state.rooms {
        sqlx::query(
            "UPDATE worldwright_characters SET room_definition = $3, version = version + 1, \
             updated_at = clock_timestamp() WHERE world_id = $1 AND character_id = $2 \
             AND room_definition <> $3",
        )
        .bind(uuid_of(world))
        .bind(uuid_of(room.character))
        .bind(&room.room_definition)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("DELETE FROM worldwright_game_counters WHERE world_id = $1")
        .bind(uuid_of(world))
        .execute(&mut **tx)
        .await?;
    for counter in &state.runtime.counters {
        sqlx::query(
            "INSERT INTO worldwright_game_counters \
             (world_id, character_id, counter_key, counter_value) VALUES ($1, $2, $3, $4)",
        )
        .bind(uuid_of(world))
        .bind(uuid_of(counter.character))
        .bind(counter.key.as_str())
        .bind(i64_from_u64(counter.value, false)?)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("DELETE FROM worldwright_scheduled_counter_adjustments WHERE world_id = $1")
        .bind(uuid_of(world))
        .execute(&mut **tx)
        .await?;
    for (ordinal, scheduled) in state.runtime.scheduled.iter().enumerate() {
        let remaining = scheduled
            .at
            .as_millis()
            .saturating_sub(state.now.as_millis());
        sqlx::query(
            "INSERT INTO worldwright_scheduled_counter_adjustments \
             (world_id, schedule_ordinal, character_id, counter_key, delta, due_at) \
             VALUES ($1, $2, $3, $4, $5, \
              clock_timestamp() + ($6::DOUBLE PRECISION * INTERVAL '1 millisecond'))",
        )
        .bind(uuid_of(world))
        .bind(i16::try_from(ordinal).map_err(|_| PersistenceError::InvalidData)?)
        .bind(uuid_of(scheduled.character))
        .bind(scheduled.key.as_str())
        .bind(scheduled.delta)
        .bind(i64_from_u64(remaining, false)?)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn validate_gameplay_state(
    world: WorldId,
    state: GameplayState<'_>,
) -> Result<(), PersistenceError> {
    if state.runtime.world != world
        || state.rooms.is_empty()
        || state.rooms.len() > 256
        || state.runtime.counters.len() > MAX_DURABLE_GAME_COUNTERS
        || state.runtime.scheduled.len() > worldwright_core::runtime::MAX_SCHEDULED_ACTIONS
    {
        return Err(PersistenceError::InvalidData);
    }
    let mut characters = BTreeSet::new();
    let mut previous = None;
    for room in state.rooms {
        validate_definition(&room.room_definition)?;
        if previous.is_some_and(|value| value >= room.character)
            || !characters.insert(room.character)
        {
            return Err(PersistenceError::InvalidData);
        }
        previous = Some(room.character);
    }
    let mut counter_keys = BTreeSet::new();
    for counter in &state.runtime.counters {
        if !characters.contains(&counter.character)
            || counter.value > MAX_COUNTER_VALUE
            || !counter_keys.insert((counter.character, counter.key.as_str()))
        {
            return Err(PersistenceError::InvalidData);
        }
    }
    let mut previous_at = None;
    for scheduled in &state.runtime.scheduled {
        if !characters.contains(&scheduled.character)
            || scheduled.delta == 0
            || scheduled.delta.unsigned_abs() > MAX_COUNTER_VALUE
            || previous_at.is_some_and(|at| at > scheduled.at)
        {
            return Err(PersistenceError::InvalidData);
        }
        previous_at = Some(scheduled.at);
    }
    Ok(())
}

fn validate_result(result: &Value) -> Result<(), PersistenceError> {
    if !result.is_object()
        || serde_json::to_vec(result)
            .map_err(|_| PersistenceError::InvalidData)?
            .len()
            > MAX_DURABLE_GAME_RESULT_BYTES
    {
        return Err(PersistenceError::InvalidData);
    }
    Ok(())
}

fn replay_gameplay(
    row: GameplayReceiptRow,
    kind: &str,
    fingerprint: [u8; 32],
) -> Result<GameplayCommit, PersistenceError> {
    if row.intent_kind != kind || row.intent_fingerprint != fingerprint {
        return Err(PersistenceError::ReceiptConflict);
    }
    if row.result_version != 1 {
        return Err(PersistenceError::InvalidData);
    }
    validate_result(&row.result)?;
    Ok(GameplayCommit {
        operation: operation_id(row.operation_id)?,
        state_version: u64_from_i64(row.state_version, true)?,
        result: row.result,
        replayed: true,
    })
}

fn parse_game(value: &str, version: i64) -> Result<GameContract, PersistenceError> {
    let version = u32::try_from(version).map_err(|_| PersistenceError::InvalidData)?;
    GameContract::parse(value, version).map_err(|_| PersistenceError::InvalidData)
}

fn gameplay_advisory_key(actor: &AuthenticatedActor<'_>, request_id: &str) -> String {
    format!(
        "gameplay|{}|{}|{}|{}:{}",
        actor.world,
        actor.account,
        actor.character,
        request_id.len(),
        request_id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldwright_core::ids::CharacterId;

    #[test]
    fn gameplay_advisory_scope_is_unambiguous() {
        let actor = AuthenticatedActor {
            world: WorldId::from_u128(1).unwrap(),
            account: worldwright_core::ids::AccountId::from_u128(2).unwrap(),
            character: CharacterId::from_u128(3).unwrap(),
            token_hash: &[0; 32],
        };
        assert_eq!(
            gameplay_advisory_key(&actor, "a:b"),
            "gameplay|00000000000000000000000000000001|\
             00000000000000000000000000000002|\
             00000000000000000000000000000003|3:a:b"
        );
    }
}
