mod error;
mod operations;
mod store;
mod types;

pub use error::PersistenceError;
pub use store::PostgresStore;
pub use types::{
    AuthenticatedActor, IdentityRecord, InventoryItem, InventorySnapshot,
    MAX_DURABLE_INVENTORY_ITEMS, MAX_DURABLE_SESSIONS, RECEIPT_FORMAT_VERSION, RewardOutcome,
    RewardResult, RewardSpec, SESSION_TTL_SECONDS, SeedAccount, SeedWorld, SessionPrincipal,
    TransferOutcome, TransferResult, TransferSpec, WriterLease, session_token_hash,
};

#[cfg(test)]
mod database_tests {
    use super::*;
    use sqlx::{
        PgPool,
        postgres::{PgConnectOptions, PgPoolOptions},
    };
    use std::{
        str::FromStr,
        sync::atomic::{AtomicU64, Ordering},
    };
    use worldwright_core::ids::{AccountId, CharacterId, ItemId, OperationId, WorldId, WriterId};

    static SCHEMA_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDb {
        store: PostgresStore,
        admin: PgPool,
        options: PgConnectOptions,
        schema: String,
    }
    impl TestDb {
        async fn new() -> Self {
            let url = std::env::var("WORLDWRIGHT_TEST_DATABASE_URL")
                .expect("WORLDWRIGHT_TEST_DATABASE_URL is required for ignored PostgreSQL tests");
            let options = PgConnectOptions::from_str(&url)
                .expect("WORLDWRIGHT_TEST_DATABASE_URL must be a valid PostgreSQL URL");
            assert!(
                options
                    .get_database()
                    .is_some_and(|name| name.contains("worldwright_test")),
                "test database name must contain worldwright_test"
            );
            let schema = format!(
                "ww_test_{}_{}",
                std::process::id(),
                SCHEMA_COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect_with(options.clone())
                .await
                .expect("connect test admin");
            sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
                .execute(&admin)
                .await
                .expect("create test schema");
            let search_path = format!("SET search_path TO \"{schema}\"");
            let pool = PgPoolOptions::new()
                .max_connections(4)
                .after_connect(move |connection, _| {
                    let search_path = search_path.clone();
                    Box::pin(async move {
                        sqlx::query("SET statement_timeout = '5s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("SET lock_timeout = '2s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("SET idle_in_transaction_session_timeout = '10s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query(&search_path).execute(&mut *connection).await?;
                        Ok(())
                    })
                })
                .connect_with(options.clone())
                .await
                .expect("connect test pool");
            let store = PostgresStore::from_pool_for_test(pool);
            store.migrate().await.expect("apply migration");
            store.verify_migrations().await.expect("verify migration");
            Self {
                store,
                admin,
                options,
                schema,
            }
        }
        async fn reconnect_store(&self) -> PostgresStore {
            let search_path = format!("SET search_path TO \"{}\"", self.schema);
            let pool = PgPoolOptions::new()
                .max_connections(4)
                .after_connect(move |connection, _| {
                    let search_path = search_path.clone();
                    Box::pin(async move {
                        sqlx::query("SET statement_timeout = '5s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("SET lock_timeout = '2s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("SET idle_in_transaction_session_timeout = '10s'")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query(&search_path).execute(&mut *connection).await?;
                        Ok(())
                    })
                })
                .connect_with(self.options.clone())
                .await
                .expect("reconnect test pool");
            let store = PostgresStore::from_pool_for_test(pool);
            store
                .verify_migrations()
                .await
                .expect("verify reconstructed store");
            store
        }
        async fn finish(self) {
            self.store.close_for_test().await;
            sqlx::query(&format!("DROP SCHEMA \"{}\" CASCADE", self.schema))
                .execute(&self.admin)
                .await
                .expect("drop test schema");
            self.admin.close().await;
        }
    }

    fn seed() -> SeedWorld {
        SeedWorld {
            world: WorldId::from_u128(1).unwrap(),
            content_namespace: "reference".to_owned(),
            content_schema_version: 1,
            entry_room_definition: "reference:rooms.atrium".to_owned(),
            accounts: vec![
                SeedAccount {
                    account: AccountId::from_u128(1).unwrap(),
                    username: "alice".to_owned(),
                    password_hash: "$argon2id$test".to_owned(),
                    character: CharacterId::from_u128(1).unwrap(),
                    character_name: "Alice".to_owned(),
                },
                SeedAccount {
                    account: AccountId::from_u128(2).unwrap(),
                    username: "bob".to_owned(),
                    password_hash: "$argon2id$test-bob".to_owned(),
                    character: CharacterId::from_u128(2).unwrap(),
                    character_name: "Bob".to_owned(),
                },
            ],
        }
    }
    async fn prepared() -> (TestDb, WriterLease) {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let lease = db
            .store
            .acquire_writer(
                WorldId::from_u128(1).unwrap(),
                WriterId::from_u128(1).unwrap(),
            )
            .await
            .expect("writer");
        (db, lease)
    }
    fn principal(account: u128, character: u128, name: &str) -> SessionPrincipal {
        SessionPrincipal {
            account: AccountId::from_u128(account).unwrap(),
            character: CharacterId::from_u128(character).unwrap(),
            character_name: name.to_owned(),
        }
    }
    async fn actor(
        db: &PostgresStore,
        account: u128,
        character: u128,
        name: &str,
        token: &str,
    ) -> AuthenticatedActor<'static> {
        let p = principal(account, character, name);
        db.create_session(
            &p,
            WorldId::from_u128(1).unwrap(),
            &session_token_hash(token),
            SESSION_TTL_SECONDS,
            MAX_DURABLE_SESSIONS,
        )
        .await
        .expect("session");
        let hash = Box::leak(Box::new(session_token_hash(token)));
        AuthenticatedActor {
            world: WorldId::from_u128(1).unwrap(),
            account: AccountId::from_u128(account).unwrap(),
            character: CharacterId::from_u128(character).unwrap(),
            token_hash: hash,
        }
    }
    fn reward(item: u128, key: &str, generation: u64) -> RewardSpec<'static> {
        RewardSpec {
            key: Box::leak(key.to_owned().into_boxed_str()),
            generation,
            credits: 100,
            item_definition: "reference:items.welcome_token",
            item: ItemId::from_u128(item).unwrap(),
        }
    }
    async fn count(db: &PostgresStore, table: &str, world: WorldId) -> i64 {
        let sql = match table {
            "receipts" => {
                "SELECT COUNT(*)::BIGINT FROM worldwright_operation_receipts WHERE world_id = $1"
            }
            "audit" => "SELECT COUNT(*)::BIGINT FROM worldwright_audit_log WHERE world_id = $1",
            "outbox" => "SELECT COUNT(*)::BIGINT FROM worldwright_outbox WHERE world_id = $1",
            _ => panic!("unknown count"),
        };
        let (count,) = sqlx::query_as::<_, (i64,)>(sql)
            .bind(types::uuid_of(world))
            .fetch_one(db.pool_for_test())
            .await
            .unwrap();
        count
    }
    async fn fill_inventory(
        db: &PostgresStore,
        world: WorldId,
        character: CharacterId,
        start: u128,
        count: u128,
    ) {
        for id in start..start + count {
            sqlx::query("INSERT INTO worldwright_item_instances ( item_id, world_id, definition_id, owner_character_id ) VALUES ($1, $2, $3, $4)").bind(types::uuid_of(ItemId::from_u128(id).unwrap())).bind(types::uuid_of(world)).bind("reference:items.test_item").bind(types::uuid_of(character)).execute(db.pool_for_test()).await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn migration_and_idempotent_seed() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        db.store.seed(&seed()).await.expect("idempotent seed");
        let identities = db
            .store
            .identities(WorldId::from_u128(1).unwrap())
            .await
            .unwrap();
        assert_eq!(identities.len(), 2);
        let before = db
            .store
            .identities(WorldId::from_u128(1).unwrap())
            .await
            .unwrap();
        let mut mismatch = seed();
        mismatch.content_namespace = "other".to_owned();
        assert!(matches!(
            db.store.seed(&mismatch).await,
            Err(PersistenceError::SeedConflict)
        ));
        let mut account_conflict = seed();
        account_conflict.accounts[0].username = "different".to_owned();
        assert!(matches!(
            db.store.seed(&account_conflict).await,
            Err(PersistenceError::SeedConflict)
        ));
        let mut character_conflict = seed();
        character_conflict.accounts[0].character_name = "Changed".to_owned();
        assert!(matches!(
            db.store.seed(&character_conflict).await,
            Err(PersistenceError::SeedConflict)
        ));
        assert_eq!(
            db.store
                .identities(WorldId::from_u128(1).unwrap())
                .await
                .unwrap()
                .len(),
            before.len()
        );
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn durable_sessions_restart_lookup_revocation_and_expiry() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let p = principal(1, 1, "Alice");
        let hash = session_token_hash("session-a");
        db.store
            .create_session(
                &p,
                WorldId::from_u128(1).unwrap(),
                &hash,
                SESSION_TTL_SECONDS,
                1,
            )
            .await
            .unwrap();
        assert_eq!(
            db.store
                .session(WorldId::from_u128(1).unwrap(), &hash)
                .await
                .unwrap()
                .unwrap()
                .account,
            p.account
        );
        let hash_b = session_token_hash("session-b");
        assert!(matches!(
            db.store
                .create_session(
                    &p,
                    WorldId::from_u128(1).unwrap(),
                    &hash_b,
                    SESSION_TTL_SECONDS,
                    1
                )
                .await,
            Err(PersistenceError::SessionCapacity)
        ));
        let reconstructed = db.reconnect_store().await;
        assert!(
            reconstructed
                .session(WorldId::from_u128(1).unwrap(), &hash)
                .await
                .unwrap()
                .is_some()
        );
        reconstructed.revoke_session(&hash).await.unwrap();
        assert!(
            reconstructed
                .session(WorldId::from_u128(1).unwrap(), &hash)
                .await
                .unwrap()
                .is_none()
        );
        let hash_b = session_token_hash("session-b-after-revoke");
        reconstructed
            .create_session(
                &p,
                WorldId::from_u128(1).unwrap(),
                &hash_b,
                SESSION_TTL_SECONDS,
                1,
            )
            .await
            .unwrap();
        sqlx::query("UPDATE worldwright_sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token_hash = $1").bind(hash_b.as_slice()).execute(reconstructed.pool_for_test()).await.unwrap();
        assert!(
            reconstructed
                .session(WorldId::from_u128(1).unwrap(), &hash_b)
                .await
                .unwrap()
                .is_none()
        );
        reconstructed.close_for_test().await;
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn writer_fencing() {
        let (db, lease_a) = prepared().await;
        let lease_b = db
            .store
            .acquire_writer(
                WorldId::from_u128(1).unwrap(),
                WriterId::from_u128(2).unwrap(),
            )
            .await
            .unwrap();
        let actor = actor(&db.store, 1, 1, "Alice", "writer-token").await;
        let before_inventory = db.store.inventory(&actor).await.unwrap();
        let before_receipts = count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await;
        let before_audit = count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await;
        let before_outbox = count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await;
        assert!(matches!(
            db.store
                .claim_reward(
                    lease_a,
                    &actor,
                    "stale",
                    OperationId::from_u128(1).unwrap(),
                    reward(1, "welcome", 1)
                )
                .await,
            Err(PersistenceError::StaleWriter)
        ));
        assert_eq!(db.store.inventory(&actor).await.unwrap(), before_inventory);
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            before_receipts
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            before_audit
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            before_outbox
        );
        assert!(
            db.store
                .claim_reward(
                    lease_b,
                    &actor,
                    "fresh",
                    OperationId::from_u128(2).unwrap(),
                    reward(1, "welcome", 1)
                )
                .await
                .is_ok()
        );
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn reward_replay_conflict_duplicate_and_capacity() {
        let (db, lease) = prepared().await;
        let actor = actor(&db.store, 1, 1, "Alice", "reward-token").await;
        let first = db
            .store
            .claim_reward(
                lease,
                &actor,
                "welcome",
                OperationId::from_u128(1).unwrap(),
                reward(1, "welcome", 1),
            )
            .await
            .unwrap();
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(first.outcome, RewardOutcome::RewardGranted);
        assert_eq!(first.balance, 100);
        assert!(first.item.is_some() && !first.replayed);
        let replay = db
            .store
            .claim_reward(
                lease,
                &actor,
                "welcome",
                OperationId::from_u128(99).unwrap(),
                reward(1, "welcome", 1),
            )
            .await
            .unwrap();
        assert!(
            replay.replayed
                && replay.item == first.item
                && replay.operation == first.operation
                && replay.balance == first.balance
        );
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert!(matches!(
            db.store
                .claim_reward(
                    lease,
                    &actor,
                    "welcome",
                    OperationId::from_u128(2).unwrap(),
                    reward(2, "welcome", 1)
                )
                .await,
            Err(PersistenceError::ReceiptConflict)
        ));
        let duplicate = db
            .store
            .claim_reward(
                lease,
                &actor,
                "welcome-again",
                OperationId::from_u128(3).unwrap(),
                reward(2, "welcome", 1),
            )
            .await
            .unwrap();
        assert_eq!(duplicate.outcome, RewardOutcome::AlreadyClaimed);
        assert_eq!(duplicate.balance, first.balance);
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            2
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            2
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        fill_inventory(
            &db.store,
            WorldId::from_u128(1).unwrap(),
            CharacterId::from_u128(1).unwrap(),
            10,
            255,
        )
        .await;
        let full = db
            .store
            .claim_reward(
                lease,
                &actor,
                "other",
                OperationId::from_u128(4).unwrap(),
                reward(300, "other", 2),
            )
            .await
            .unwrap();
        assert_eq!(full.outcome, RewardOutcome::InventoryFull);
        assert_eq!(full.balance, first.balance);
        assert_eq!(full.item, None);
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn transfer_replay_not_owned_self_target_and_capacity() {
        let (db, lease) = prepared().await;
        let alice = actor(&db.store, 1, 1, "Alice", "alice-transfer").await;
        let bob = actor(&db.store, 2, 2, "Bob", "bob-transfer").await;
        db.store
            .claim_reward(
                lease,
                &alice,
                "claim",
                OperationId::from_u128(1).unwrap(),
                reward(1, "welcome", 1),
            )
            .await
            .unwrap();
        let transfer_spec = || TransferSpec {
            item: ItemId::from_u128(1).unwrap(),
            recipient_username: "bob",
        };
        let first = db
            .store
            .transfer_item(
                lease,
                &alice,
                "transfer",
                OperationId::from_u128(2).unwrap(),
                transfer_spec(),
            )
            .await
            .unwrap();
        assert_eq!(first.outcome, TransferOutcome::ItemTransferred);
        assert!(
            db.store
                .transfer_item(
                    lease,
                    &alice,
                    "transfer",
                    OperationId::from_u128(99).unwrap(),
                    transfer_spec()
                )
                .await
                .unwrap()
                .replayed
        );
        let not_owned = db
            .store
            .transfer_item(
                lease,
                &alice,
                "not-owned",
                OperationId::from_u128(3).unwrap(),
                transfer_spec(),
            )
            .await
            .unwrap();
        assert_eq!(not_owned.outcome, TransferOutcome::NotOwned);
        assert_eq!(not_owned.owner, None);
        let missing = db
            .store
            .transfer_item(
                lease,
                &alice,
                "missing",
                OperationId::from_u128(4).unwrap(),
                TransferSpec {
                    item: ItemId::from_u128(1).unwrap(),
                    recipient_username: "missing",
                },
            )
            .await
            .unwrap();
        assert_eq!(missing.outcome, TransferOutcome::TargetNotFound);
        assert_eq!(missing.owner, None);
        let self_transfer = db
            .store
            .transfer_item(
                lease,
                &bob,
                "self",
                OperationId::from_u128(6).unwrap(),
                TransferSpec {
                    item: ItemId::from_u128(1).unwrap(),
                    recipient_username: "bob",
                },
            )
            .await
            .unwrap();
        assert_eq!(self_transfer.outcome, TransferOutcome::SameOwner);
        db.store
            .claim_reward(
                lease,
                &alice,
                "claim-two",
                OperationId::from_u128(7).unwrap(),
                reward(2, "second", 2),
            )
            .await
            .unwrap();
        fill_inventory(
            &db.store,
            WorldId::from_u128(1).unwrap(),
            CharacterId::from_u128(2).unwrap(),
            10,
            255,
        )
        .await;
        let full = db
            .store
            .transfer_item(
                lease,
                &alice,
                "full",
                OperationId::from_u128(8).unwrap(),
                TransferSpec {
                    item: ItemId::from_u128(2).unwrap(),
                    recipient_username: "bob",
                },
            )
            .await
            .unwrap();
        assert_eq!(full.outcome, TransferOutcome::InventoryFull);
        assert_eq!(full.owner, Some(CharacterId::from_u128(1).unwrap()));
        assert_eq!(
            db.store
                .inventory(&alice)
                .await
                .unwrap()
                .items
                .iter()
                .filter(|item| item.item == ItemId::from_u128(2).unwrap())
                .count(),
            1
        );
        assert_eq!(db.store.inventory(&bob).await.unwrap().items.len(), 256);
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn pre_commit_rollback_fault_injection() {
        let (db, lease) = prepared().await;
        let actor = actor(&db.store, 1, 1, "Alice", "fault-token").await;
        let before = db.store.inventory(&actor).await.unwrap();
        let before_receipts = count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await;
        let before_audit = count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await;
        let before_outbox = count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await;
        assert!(
            db.store
                .claim_reward_fault_after_value_for_test(
                    lease,
                    &actor,
                    "fault",
                    OperationId::from_u128(1).unwrap(),
                    reward(1, "fault", 1)
                )
                .await
                .is_err()
        );
        assert_eq!(db.store.inventory(&actor).await.unwrap(), before);
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            before_receipts
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            before_audit
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            before_outbox
        );
        assert!(
            db.store
                .claim_reward(
                    lease,
                    &actor,
                    "fault",
                    OperationId::from_u128(1).unwrap(),
                    reward(1, "fault", 1)
                )
                .await
                .is_ok()
        );
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn restart_inventory_reconstruction() {
        let (db, lease) = prepared().await;
        let alice_actor = actor(&db.store, 1, 1, "Alice", "restart-token").await;
        let bob = actor(&db.store, 2, 2, "Bob", "restart-bob-token").await;
        db.store
            .claim_reward(
                lease,
                &alice_actor,
                "restart-claim",
                OperationId::from_u128(1).unwrap(),
                reward(1, "restart", 1),
            )
            .await
            .unwrap();
        db.store
            .transfer_item(
                lease,
                &alice_actor,
                "restart-transfer",
                OperationId::from_u128(2).unwrap(),
                TransferSpec {
                    item: ItemId::from_u128(1).unwrap(),
                    recipient_username: "bob",
                },
            )
            .await
            .unwrap();
        let reconstructed = db.reconnect_store().await;
        assert_eq!(
            reconstructed.inventory(&alice_actor).await.unwrap().balance,
            100
        );
        assert!(
            reconstructed
                .inventory(&alice_actor)
                .await
                .unwrap()
                .items
                .is_empty()
        );
        assert_eq!(
            reconstructed.inventory(&bob).await.unwrap().items[0].item,
            ItemId::from_u128(1).unwrap()
        );
        let replay = reconstructed
            .transfer_item(
                lease,
                &alice_actor,
                "restart-transfer",
                OperationId::from_u128(99).unwrap(),
                TransferSpec {
                    item: ItemId::from_u128(1).unwrap(),
                    recipient_username: "bob",
                },
            )
            .await
            .unwrap();
        assert!(replay.replayed);
        reconstructed.close_for_test().await;
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn receipt_audit_and_outbox_cardinality() {
        let (db, lease) = prepared().await;
        let alice_actor = actor(&db.store, 1, 1, "Alice", "count-token").await;
        let bob = actor(&db.store, 2, 2, "Bob", "count-bob-token").await;
        db.store
            .claim_reward(
                lease,
                &alice_actor,
                "count",
                OperationId::from_u128(1).unwrap(),
                reward(1, "count", 1),
            )
            .await
            .unwrap();
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert!(
            db.store
                .claim_reward(
                    lease,
                    &alice_actor,
                    "count",
                    OperationId::from_u128(99).unwrap(),
                    reward(1, "count", 1)
                )
                .await
                .unwrap()
                .replayed
        );
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert!(matches!(
            db.store
                .claim_reward(
                    lease,
                    &alice_actor,
                    "fresh-duplicate",
                    OperationId::from_u128(2).unwrap(),
                    reward(2, "count", 1)
                )
                .await
                .unwrap()
                .outcome,
            RewardOutcome::AlreadyClaimed
        ));
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            2
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            2
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            1
        );
        assert_eq!(
            db.store
                .transfer_item(
                    lease,
                    &alice_actor,
                    "count-transfer",
                    OperationId::from_u128(3).unwrap(),
                    TransferSpec {
                        item: ItemId::from_u128(1).unwrap(),
                        recipient_username: "bob"
                    }
                )
                .await
                .unwrap()
                .outcome,
            TransferOutcome::ItemTransferred
        );
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            2
        );
        assert!(
            db.store
                .transfer_item(
                    lease,
                    &alice_actor,
                    "count-transfer",
                    OperationId::from_u128(99).unwrap(),
                    TransferSpec {
                        item: ItemId::from_u128(1).unwrap(),
                        recipient_username: "bob"
                    }
                )
                .await
                .unwrap()
                .replayed
        );
        assert_eq!(
            count(&db.store, "receipts", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "audit", WorldId::from_u128(1).unwrap()).await,
            3
        );
        assert_eq!(
            count(&db.store, "outbox", WorldId::from_u128(1).unwrap()).await,
            2
        );
        let _ = bob;
        db.finish().await;
    }
}
