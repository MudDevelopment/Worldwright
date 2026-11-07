mod error;
mod operational;
mod operational_queries;
mod operations;
mod releases;
mod store;
mod types;

pub use error::PersistenceError;
pub use store::PostgresStore;
pub use types::{
    ActivationPlan, AuthenticatedActor, BootstrapOperator, DurableContentReferences,
    IdentityRecord, InventoryItem, InventorySnapshot, MAX_DURABLE_INVENTORY_ITEMS,
    MAX_DURABLE_SESSIONS, OperationalStatus, OperatorContext, PublicationRecord,
    RECEIPT_FORMAT_VERSION, RecoveryJob, ReleaseReadGuard, ReleaseState, RewardOutcome,
    RewardResult, RewardSpec, SESSION_TTL_SECONDS, SeedAccount, SeedWorld, SessionPrincipal,
    StoredArtifact, TransferOutcome, TransferResult, TransferSpec, WriterLease, session_token_hash,
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
        time::Duration,
    };
    use worldwright_core::ids::{
        AccountId, ActivationId, CharacterId, CorrelationId, GrantId, ItemId, OperationId,
        PublicationId, RecoveryJobId, WorldId, WriterId,
    };
    use worldwright_operations::{
        Assurance, Capability, Environment, OperatorClaims, OperatorKind, StudioReviewContext,
    };
    use worldwright_release::{
        DeploymentTarget, PackageOptions, PublicationKind, PublicationStatus, ReviewDecision,
        Sha256Digest, package_bytes,
    };

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

    fn release_artifact(release_id: &str) -> Vec<u8> {
        release_artifact_with_builder(release_id, "test-builder")
    }

    fn release_artifact_with_builder(release_id: &str, builder_identity: &str) -> Vec<u8> {
        package_bytes(
            "release-test",
            include_bytes!("../../../examples/two-room/world.json"),
            PackageOptions {
                release_id: release_id.parse().unwrap(),
                source_revision: "test-revision".to_owned(),
                builder_identity: builder_identity.to_owned(),
            },
        )
        .unwrap()
    }

    async fn release_audit_actions(db: &TestDb, world: WorldId) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT action FROM worldwright_release_audit WHERE world_id = $1 ORDER BY audit_id",
        )
        .bind(types::uuid_of(world))
        .fetch_all(db.store.pool_for_test())
        .await
        .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_publication_identity_review_and_activation_workflow() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let artifact = release_artifact("release-staging");
        let digest = worldwright_release::Sha256Digest::from_bytes(&artifact);
        let request = worldwright_core::ids::PublicationId::from_u128(10).unwrap();
        let record = db
            .store
            .submit_publication(
                request,
                world,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("publication");
        assert_eq!(record.artifact_digest(), digest);
        let second_request = worldwright_core::ids::PublicationId::from_u128(11).unwrap();
        assert!(
            db.store
                .submit_publication(
                    second_request,
                    world,
                    &artifact,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    None,
                    "requester",
                )
                .await
                .is_ok()
        );
        assert!(matches!(
            db.store
                .review_publication(request, "requester", ReviewDecision::Approve, "approve")
                .await,
            Err(PersistenceError::InvalidTransition)
        ));
        db.store
            .review_publication(request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        let plan = db
            .store
            .begin_activation(
                request,
                worldwright_core::ids::ActivationId::from_u128(20).unwrap(),
                WriterId::from_u128(2).unwrap(),
                "operator",
            )
            .await
            .expect("begin");
        db.store.complete_activation(plan).await.expect("complete");
        let active = db.store.active_artifact(world).await.expect("active");
        assert_eq!(active.digest(), digest);
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_expected_and_production_gates_are_enforced() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let artifact = release_artifact("release-production");
        let request = worldwright_core::ids::PublicationId::from_u128(30).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Production,
                Some(worldwright_release::Sha256Digest::from_bytes(b"wrong")),
                "requester",
            )
            .await
            .expect_err("stale expected release must fail");
        let request = worldwright_core::ids::PublicationId::from_u128(31).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Production,
                None,
                "requester",
            )
            .await
            .expect("publication");
        db.store
            .review_publication(request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        assert!(matches!(
            db.store
                .begin_activation(
                    request,
                    worldwright_core::ids::ActivationId::from_u128(32).unwrap(),
                    WriterId::from_u128(2).unwrap(),
                    "operator",
                )
                .await,
            Err(PersistenceError::ProductionGateRequired)
        ));
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_begin_reconstruct_recover_and_complete() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let artifact = release_artifact("release-recover");
        let request = worldwright_core::ids::PublicationId::from_u128(40).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("publication");
        db.store
            .review_publication(request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        db.store
            .begin_activation(
                request,
                worldwright_core::ids::ActivationId::from_u128(41).unwrap(),
                WriterId::from_u128(2).unwrap(),
                "operator",
            )
            .await
            .expect("begin");
        let reconstructed = db.reconnect_store().await;
        let plan = reconstructed
            .recover_activation(world, WriterId::from_u128(3).unwrap(), "recovery")
            .await
            .expect("recover");
        assert_eq!(plan.executor(), "recovery");
        reconstructed
            .complete_activation(plan)
            .await
            .expect("complete");
        let active = reconstructed.active_artifact(world).await.expect("active");
        assert_eq!(
            active.digest(),
            worldwright_release::Sha256Digest::from_bytes(&artifact)
        );
        reconstructed.close_for_test().await;
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_failure_preserves_pointer_and_clears_maintenance() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let artifact = release_artifact("release-fail");
        let request = worldwright_core::ids::PublicationId::from_u128(50).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("publication");
        db.store
            .review_publication(request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        let plan = db
            .store
            .begin_activation(
                request,
                worldwright_core::ids::ActivationId::from_u128(51).unwrap(),
                WriterId::from_u128(2).unwrap(),
                "operator",
            )
            .await
            .expect("begin");
        db.store
            .fail_activation(plan, "smoke_failed")
            .await
            .expect("fail");
        let state = db.store.release_state(world).await.expect("state");
        assert_eq!(state.active(), None);
        assert_eq!(state.pending(), None);
        assert!(!state.maintenance());
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_second_activation_and_rollback_restore_known_good() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let first = release_artifact("release-first");
        let second = release_artifact("release-second");
        let first_digest = worldwright_release::Sha256Digest::from_bytes(&first);
        let second_digest = worldwright_release::Sha256Digest::from_bytes(&second);

        db.store
            .submit_publication(
                worldwright_core::ids::PublicationId::from_u128(60).unwrap(),
                world,
                &first,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("first publication");
        db.store
            .review_publication(
                worldwright_core::ids::PublicationId::from_u128(60).unwrap(),
                "reviewer",
                ReviewDecision::Approve,
                "approve",
            )
            .await
            .expect("first review");
        let first_plan = db
            .store
            .begin_activation(
                worldwright_core::ids::PublicationId::from_u128(60).unwrap(),
                worldwright_core::ids::ActivationId::from_u128(61).unwrap(),
                WriterId::from_u128(2).unwrap(),
                "operator",
            )
            .await
            .expect("first begin");
        db.store
            .complete_activation(first_plan)
            .await
            .expect("first complete");

        db.store
            .submit_publication(
                worldwright_core::ids::PublicationId::from_u128(62).unwrap(),
                world,
                &second,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                Some(first_digest),
                "requester",
            )
            .await
            .expect("second publication");
        db.store
            .review_publication(
                worldwright_core::ids::PublicationId::from_u128(62).unwrap(),
                "reviewer",
                ReviewDecision::Approve,
                "approve",
            )
            .await
            .expect("second review");
        let second_plan = db
            .store
            .begin_activation(
                worldwright_core::ids::PublicationId::from_u128(62).unwrap(),
                worldwright_core::ids::ActivationId::from_u128(63).unwrap(),
                WriterId::from_u128(3).unwrap(),
                "operator",
            )
            .await
            .expect("second begin");
        db.store
            .complete_activation(second_plan)
            .await
            .expect("second complete");
        let state = db.store.release_state(world).await.expect("second state");
        assert_eq!(state.active(), Some(second_digest));
        assert_eq!(state.known_good(), Some(first_digest));

        let rollback_request = worldwright_core::ids::PublicationId::from_u128(64).unwrap();
        db.store
            .submit_publication(
                rollback_request,
                world,
                &first,
                PublicationKind::Rollback,
                DeploymentTarget::Staging,
                Some(second_digest),
                "requester",
            )
            .await
            .expect("rollback publication");
        db.store
            .review_publication(
                rollback_request,
                "reviewer",
                ReviewDecision::Approve,
                "rollback",
            )
            .await
            .expect("rollback review");
        let rollback_plan = db
            .store
            .begin_activation(
                rollback_request,
                worldwright_core::ids::ActivationId::from_u128(65).unwrap(),
                WriterId::from_u128(4).unwrap(),
                "operator",
            )
            .await
            .expect("rollback begin");
        db.store
            .complete_activation(rollback_plan)
            .await
            .expect("rollback complete");
        let state = db.store.release_state(world).await.expect("rollback state");
        assert_eq!(state.active(), Some(first_digest));
        assert_eq!(state.known_good(), Some(first_digest));
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_conflicts_and_rejection_cannot_activate() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let first = release_artifact_with_builder("release-conflict", "first");
        let changed = release_artifact_with_builder("release-conflict", "second");
        let request = worldwright_core::ids::PublicationId::from_u128(70).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &first,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("first publication");
        assert!(matches!(
            db.store
                .submit_publication(
                    worldwright_core::ids::PublicationId::from_u128(71).unwrap(),
                    world,
                    &changed,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    None,
                    "requester",
                )
                .await,
            Err(PersistenceError::ReleaseConflict)
        ));
        let rejected = worldwright_core::ids::PublicationId::from_u128(72).unwrap();
        db.store
            .submit_publication(
                rejected,
                world,
                &release_artifact("release-rejected"),
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("rejected publication");
        db.store
            .review_publication(rejected, "reviewer", ReviewDecision::Reject, "reject")
            .await
            .expect("reject");
        assert!(matches!(
            db.store
                .begin_activation(
                    rejected,
                    worldwright_core::ids::ActivationId::from_u128(73).unwrap(),
                    WriterId::from_u128(2).unwrap(),
                    "operator",
                )
                .await,
            Err(PersistenceError::InvalidTransition)
        ));
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_audit_records_request_review_start_and_complete() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let request = worldwright_core::ids::PublicationId::from_u128(80).unwrap();
        db.store
            .submit_publication(
                request,
                world,
                &release_artifact("release-audit"),
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("publication");
        db.store
            .review_publication(request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        let plan = db
            .store
            .begin_activation(
                request,
                worldwright_core::ids::ActivationId::from_u128(81).unwrap(),
                WriterId::from_u128(2).unwrap(),
                "operator",
            )
            .await
            .expect("begin");
        db.store.complete_activation(plan).await.expect("complete");
        assert_eq!(
            release_audit_actions(&db, world).await,
            vec![
                "publication_requested",
                "publication_approved",
                "activation_started",
                "activation_succeeded",
            ]
        );
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn release_read_guard_blocks_activation_until_dropped() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let base_artifact = release_artifact("release-read-guard-base");
        let base_digest = worldwright_release::Sha256Digest::from_bytes(&base_artifact);
        let base_request = worldwright_core::ids::PublicationId::from_u128(90).unwrap();
        db.store
            .submit_publication(
                base_request,
                world,
                &base_artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                "requester",
            )
            .await
            .expect("publication");
        db.store
            .review_publication(base_request, "reviewer", ReviewDecision::Approve, "approve")
            .await
            .expect("review");
        let base_plan = db
            .store
            .begin_activation(
                base_request,
                worldwright_core::ids::ActivationId::from_u128(91).unwrap(),
                WriterId::from_u128(1).unwrap(),
                "operator",
            )
            .await
            .expect("base begin");
        db.store
            .complete_activation(base_plan)
            .await
            .expect("base complete");
        let second_artifact = release_artifact("release-read-guard-second");
        let second_request = worldwright_core::ids::PublicationId::from_u128(92).unwrap();
        db.store
            .submit_publication(
                second_request,
                world,
                &second_artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                Some(base_digest),
                "requester",
            )
            .await
            .expect("second publication");
        db.store
            .review_publication(
                second_request,
                "reviewer",
                ReviewDecision::Approve,
                "approve",
            )
            .await
            .expect("second review");
        let lease = db
            .store
            .acquire_writer(world, WriterId::from_u128(2).unwrap())
            .await
            .expect("writer");
        let guard = db
            .store
            .begin_release_read(lease, base_digest)
            .await
            .expect("read guard");
        let blocked = tokio::time::timeout(
            Duration::from_millis(250),
            db.store.begin_activation(
                second_request,
                worldwright_core::ids::ActivationId::from_u128(93).unwrap(),
                WriterId::from_u128(3).unwrap(),
                "operator",
            ),
        )
        .await;
        assert!(blocked.is_err(), "activation crossed the read guard");
        drop(guard);
        let plan = db
            .store
            .begin_activation(
                second_request,
                worldwright_core::ids::ActivationId::from_u128(94).unwrap(),
                WriterId::from_u128(3).unwrap(),
                "operator",
            )
            .await
            .expect("activation after guard");
        db.store
            .fail_activation(plan, "operator_aborted")
            .await
            .expect("fail");
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn operational_bootstrap_is_unique_and_scoped() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let capabilities = [Capability::ViewStatus];
        let input = BootstrapOperator {
            subject: "operator",
            display_name: "Operator",
            kind: OperatorKind::Human,
            environment: Environment::Staging,
            world: WorldId::from_u128(1).unwrap(),
            capabilities: &capabilities,
            reason: "initial bootstrap",
            correlation: CorrelationId::from_u128(1).unwrap(),
        };
        db.store
            .bootstrap_operator(
                &input,
                OperationId::from_u128(1).unwrap(),
                &[GrantId::from_u128(1).unwrap()],
            )
            .await
            .expect("bootstrap");
        assert!(matches!(
            db.store
                .bootstrap_operator(
                    &input,
                    OperationId::from_u128(2).unwrap(),
                    &[GrantId::from_u128(2).unwrap()],
                )
                .await,
            Err(PersistenceError::OperatorDenied)
        ));
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn m6_authorized_release_and_operator_audit_acceptance() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let all = [
            Capability::ManageOperators,
            Capability::ViewStatus,
            Capability::ModerateAccount,
            Capability::RequestRelease,
            Capability::ReviewRelease,
            Capability::ActivateRelease,
            Capability::InitiateRecovery,
            Capability::BackupWorld,
            Capability::RestoreWorld,
            Capability::DrainWorld,
        ];
        let grants = (1..=all.len())
            .map(|id| GrantId::from_u128(id as u128).unwrap())
            .collect::<Vec<_>>();
        let admin_input = BootstrapOperator {
            subject: "admin",
            display_name: "Admin",
            kind: OperatorKind::Human,
            environment: Environment::Staging,
            world,
            capabilities: &all,
            reason: "first operator",
            correlation: CorrelationId::from_u128(1).unwrap(),
        };
        db.store
            .bootstrap_operator(&admin_input, OperationId::from_u128(1).unwrap(), &grants)
            .await
            .expect("bootstrap");
        let claim = |subject: &str, session: u128| OperatorClaims {
            issuer: "issuer".into(),
            audience: "audience".into(),
            subject: subject.into(),
            session_id: format!("{session:032x}"),
            kind: OperatorKind::Human,
            assurance: Assurance::Mfa,
            issued_at: 1000,
            expires_at: 1600,
            auth_time: 1000,
            break_glass_reason: None,
        };
        let admin_claim = claim("admin", 100);
        let requester_claim = claim("requester", 101);
        let reviewer_claim = claim("reviewer", 102);
        fn op_context<'a>(
            claims: &'a OperatorClaims,
            world: WorldId,
            correlation: u128,
            reason: &'a str,
        ) -> OperatorContext<'a> {
            OperatorContext {
                claims,
                environment: Environment::Staging,
                world,
                correlation: CorrelationId::from_u128(correlation).unwrap(),
                reason,
            }
        }
        let admin = op_context(&admin_claim, world, 10, "manage operators");
        db.store
            .create_operator(
                &admin,
                "requester",
                "Requester",
                OperatorKind::Human,
                OperationId::from_u128(10).unwrap(),
            )
            .await
            .expect("requester");
        db.store
            .create_operator(
                &admin,
                "reviewer",
                "Reviewer",
                OperatorKind::Human,
                OperationId::from_u128(11).unwrap(),
            )
            .await
            .expect("reviewer");
        let request_caps = [Capability::RequestRelease];
        db.store
            .grant(
                &admin,
                GrantId::from_u128(100).unwrap(),
                "requester",
                request_caps[0],
                Environment::Staging,
                world,
                None,
                OperationId::from_u128(12).unwrap(),
            )
            .await
            .expect("request grant");
        for (id, capability) in [Capability::ReviewRelease, Capability::ActivateRelease]
            .into_iter()
            .enumerate()
        {
            db.store
                .grant(
                    &admin,
                    GrantId::from_u128(101 + id as u128).unwrap(),
                    "reviewer",
                    capability,
                    Environment::Staging,
                    world,
                    None,
                    OperationId::from_u128(13 + id as u128).unwrap(),
                )
                .await
                .expect("review grant");
        }
        for (operation, subject) in [
            (OperationId::from_u128(10).unwrap(), "requester"),
            (OperationId::from_u128(11).unwrap(), "reviewer"),
        ] {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM worldwright_operator_audit WHERE operation_id = $1 AND action = $2")
                .bind(types::uuid_of(operation))
                .bind("operator_created")
                .fetch_one(db.store.pool_for_test())
                .await
                .expect("audit count");
            assert_eq!((count, subject), (1, subject));
        }
        let requester = op_context(&requester_claim, world, 20, "request release");
        let production = OperatorContext {
            environment: Environment::Production,
            ..requester
        };
        assert!(matches!(
            db.store
                .authorize_operator(&production, Capability::RequestRelease)
                .await,
            Err(PersistenceError::OperatorDenied)
        ));
        let artifact = release_artifact("m6-authorized");
        let request = PublicationId::from_u128(200).unwrap();
        let record = db
            .store
            .submit_publication_authorized(
                &requester,
                request,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                OperationId::from_u128(20).unwrap(),
            )
            .await
            .expect("authorized request");
        assert_eq!(record.status(), PublicationStatus::Requested);
        assert!(
            db.store
                .review_publication_authorized(
                    &requester,
                    request,
                    ReviewDecision::Approve,
                    OperationId::from_u128(21).unwrap()
                )
                .await
                .is_err()
        );
        let reviewer = op_context(&reviewer_claim, world, 21, "review release");
        db.store
            .review_publication_authorized(
                &reviewer,
                request,
                ReviewDecision::Approve,
                OperationId::from_u128(22).unwrap(),
            )
            .await
            .expect("review");
        assert_eq!(
            release_audit_actions(&db, world).await,
            vec!["publication_requested", "publication_approved"]
        );
        let old_lease = db
            .store
            .acquire_writer(world, WriterId::from_u128(500).unwrap())
            .await
            .expect("writer");
        let plan = db
            .store
            .begin_activation_authorized(
                &reviewer,
                request,
                ActivationId::from_u128(201).unwrap(),
                WriterId::from_u128(501).unwrap(),
                OperationId::from_u128(23).unwrap(),
            )
            .await
            .expect("begin");
        db.store
            .revoke_operator_session(
                &admin,
                "reviewer",
                reviewer_claim.session_id().unwrap(),
                OperationId::from_u128(24).unwrap(),
            )
            .await
            .expect("revoke");
        assert!(matches!(
            db.store
                .complete_activation_authorized(
                    &reviewer,
                    plan.clone(),
                    OperationId::from_u128(25).unwrap()
                )
                .await,
            Err(PersistenceError::OperatorDenied)
        ));
        let state = db.store.release_state(world).await.expect("state");
        assert!(state.pending().is_some());
        assert!(matches!(
            db.store
                .rotate_authority(
                    &admin,
                    WriterId::from_u128(502).unwrap(),
                    OperationId::from_u128(26).unwrap(),
                )
                .await,
            Err(PersistenceError::RecoveryConflict)
        ));
        db.store
            .fail_activation_authorized(
                &admin,
                plan,
                "operator_aborted",
                OperationId::from_u128(27).unwrap(),
            )
            .await
            .expect("abort pending activation");
        let incarnation = db
            .store
            .rotate_authority(
                &admin,
                WriterId::from_u128(502).unwrap(),
                OperationId::from_u128(28).unwrap(),
            )
            .await
            .expect("rotate");
        assert_ne!(incarnation.incarnation(), old_lease.incarnation());
        assert_eq!(incarnation.epoch(), 1);
        let moderation = op_context(&admin_claim, world, 30, "suspend account");
        db.store
            .set_account_disabled(
                &moderation,
                AccountId::from_u128(1).unwrap(),
                true,
                OperationId::from_u128(29).unwrap(),
            )
            .await
            .expect("suspend account");
        let backup = op_context(&admin_claim, world, 31, "backup world");
        let job = RecoveryJobId::from_u128(300).unwrap();
        let started = db
            .store
            .begin_recovery_job(
                &backup,
                job,
                "backup",
                OperationId::from_u128(30).unwrap(),
                serde_json::json!({"phase":"test"}),
            )
            .await
            .expect("begin recovery job");
        assert_eq!(started.job, job);
        db.store
            .complete_recovery_job(
                &backup,
                "backup",
                job,
                "succeeded",
                Some([1; 32]),
                Some([2; 32]),
                None,
                OperationId::from_u128(31).unwrap(),
                serde_json::json!({"phase":"succeeded"}),
            )
            .await
            .expect("complete recovery job");
        for (operation, action) in [
            (OperationId::from_u128(20).unwrap(), "release_requested"),
            (OperationId::from_u128(22).unwrap(), "release_approved"),
            (OperationId::from_u128(23).unwrap(), "activation_started"),
            (OperationId::from_u128(29).unwrap(), "account_suspended"),
            (OperationId::from_u128(30).unwrap(), "backup_started"),
            (OperationId::from_u128(31).unwrap(), "backup_succeeded"),
        ] {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM worldwright_operator_audit WHERE operation_id = $1 AND action = $2")
                .bind(types::uuid_of(operation)).bind(action).fetch_one(db.store.pool_for_test()).await.expect("audit count");
            assert_eq!(count, 1);
        }
        db.finish().await;
    }

    #[tokio::test]
    #[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
    async fn studio_review_context_is_bound_authorized_and_round_trips() {
        let db = TestDb::new().await;
        db.store.seed(&seed()).await.expect("seed");
        let world = WorldId::from_u128(1).unwrap();
        let capabilities = [Capability::ManageOperators, Capability::ViewStatus];
        let grants = [
            GrantId::from_u128(301).unwrap(),
            GrantId::from_u128(302).unwrap(),
        ];
        let admin_claim = OperatorClaims {
            issuer: "issuer".into(),
            audience: "audience".into(),
            subject: "admin".into(),
            session_id: format!("{:032x}", 301u128),
            kind: OperatorKind::Human,
            assurance: Assurance::Mfa,
            issued_at: 1000,
            expires_at: 1600,
            auth_time: 1000,
            break_glass_reason: None,
        };
        let admin = OperatorContext {
            claims: &admin_claim,
            environment: Environment::Staging,
            world,
            correlation: CorrelationId::from_u128(301).unwrap(),
            reason: "studio setup",
        };
        db.store
            .bootstrap_operator(
                &BootstrapOperator {
                    subject: "admin",
                    display_name: "Admin",
                    kind: OperatorKind::Human,
                    environment: Environment::Staging,
                    world,
                    capabilities: &capabilities,
                    reason: "first operator",
                    correlation: CorrelationId::from_u128(302).unwrap(),
                },
                OperationId::from_u128(301).unwrap(),
                &grants,
            )
            .await
            .expect("bootstrap");
        db.store
            .create_operator(
                &admin,
                "requester",
                "Requester",
                OperatorKind::Human,
                OperationId::from_u128(303).unwrap(),
            )
            .await
            .expect("operator");
        db.store
            .grant(
                &admin,
                GrantId::from_u128(304).unwrap(),
                "requester",
                Capability::RequestRelease,
                Environment::Staging,
                world,
                None,
                OperationId::from_u128(304).unwrap(),
            )
            .await
            .expect("grant");
        db.store
            .grant(
                &admin,
                GrantId::from_u128(305).unwrap(),
                "requester",
                Capability::ViewStatus,
                Environment::Staging,
                world,
                None,
                OperationId::from_u128(305).unwrap(),
            )
            .await
            .expect("view grant");
        let requester_claim = OperatorClaims {
            issuer: "issuer".into(),
            audience: "audience".into(),
            subject: "requester".into(),
            session_id: format!("{:032x}", 305u128),
            kind: OperatorKind::Human,
            assurance: Assurance::Mfa,
            issued_at: 1000,
            expires_at: 1600,
            auth_time: 1000,
            break_glass_reason: None,
        };
        let requester = OperatorContext {
            claims: &requester_claim,
            environment: Environment::Staging,
            world,
            correlation: CorrelationId::from_u128(305).unwrap(),
            reason: "studio submission",
        };
        let artifact = release_artifact("studio-context");
        let verified =
            worldwright_release::verify_bytes("studio-context", &artifact, None).unwrap();
        let context = StudioReviewContext {
            schema_version: 1,
            draft_revision: 7,
            snapshot_sha256: verified.manifest().payload_sha256().to_string(),
            validation_diagnostic_count: 0,
            preview_artifact_sha256: verified.artifact_digest().to_string(),
            preview_completed_at: 1_700_000_000,
            change_summary: "Studio change".into(),
        };
        let request = PublicationId::from_u128(306).unwrap();
        let record = db
            .store
            .submit_publication_authorized_with_context(
                &requester,
                request,
                &artifact,
                PublicationKind::Activate,
                DeploymentTarget::Staging,
                None,
                context.clone(),
                OperationId::from_u128(306).unwrap(),
            )
            .await
            .expect("submission");
        let count = || async {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*)::BIGINT FROM worldwright_publication_requests WHERE world_id = $1",
            )
            .bind(types::uuid_of(world))
            .fetch_one(db.store.pool_for_test())
            .await
            .unwrap()
        };
        assert_eq!(count().await, 1);
        assert_eq!(record.review_context(), Some(&context));
        assert_eq!(
            db.store
                .publication(request)
                .await
                .unwrap()
                .review_context(),
            Some(&context)
        );
        assert_eq!(
            db.store
                .publication_authorized(&requester, request)
                .await
                .unwrap()
                .review_context(),
            Some(&context)
        );
        let mut bad_preview = context.clone();
        bad_preview.preview_artifact_sha256 = "11".repeat(32);
        assert!(matches!(
            db.store
                .submit_publication_authorized_with_context(
                    &requester,
                    PublicationId::from_u128(307).unwrap(),
                    &artifact,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    None,
                    bad_preview,
                    OperationId::from_u128(307).unwrap()
                )
                .await,
            Err(PersistenceError::InvalidData)
        ));
        assert_eq!(count().await, 1);
        let mut bad_snapshot = context.clone();
        bad_snapshot.snapshot_sha256 = "22".repeat(32);
        assert!(matches!(
            db.store
                .submit_publication_authorized_with_context(
                    &requester,
                    PublicationId::from_u128(308).unwrap(),
                    &artifact,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    None,
                    bad_snapshot,
                    OperationId::from_u128(308).unwrap()
                )
                .await,
            Err(PersistenceError::InvalidData)
        ));
        assert_eq!(count().await, 1);
        assert!(matches!(
            db.store
                .submit_publication_authorized_with_context(
                    &requester,
                    PublicationId::from_u128(309).unwrap(),
                    &artifact,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    Some(Sha256Digest::from_digest_bytes([0x33; 32])),
                    context.clone(),
                    OperationId::from_u128(309).unwrap()
                )
                .await,
            Err(PersistenceError::ExpectedReleaseConflict)
        ));
        assert_eq!(count().await, 1);
        db.store
            .revoke_grant(
                &admin,
                GrantId::from_u128(304).unwrap(),
                OperationId::from_u128(310).unwrap(),
            )
            .await
            .unwrap();
        assert!(matches!(
            db.store
                .submit_publication_authorized_with_context(
                    &requester,
                    PublicationId::from_u128(311).unwrap(),
                    &artifact,
                    PublicationKind::Activate,
                    DeploymentTarget::Staging,
                    None,
                    context.clone(),
                    OperationId::from_u128(311).unwrap()
                )
                .await,
            Err(PersistenceError::OperatorDenied)
        ));
        assert_eq!(count().await, 1);
        assert_eq!(
            db.store
                .publication(request)
                .await
                .unwrap()
                .review_context(),
            Some(&context)
        );
        db.finish().await;
    }
}
