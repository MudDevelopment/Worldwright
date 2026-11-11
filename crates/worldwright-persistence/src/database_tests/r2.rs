//! Actual PostgreSQL adapter regressions. Not process/provider or release approval.
use super::*;
use worldwright_core::{runtime::{CounterState, DurableWorldState, ScheduledCounterState}, time::Tick};

async fn game_case(game: GameContract) -> (TestDb, WriterLease, Sha256Digest, DurableWorldState, Vec<DurableCharacterRoom>) {
    let db = TestDb::new().await;
    let mut data = seed();
    let bytes: &[u8] = if game == GameContract::Reference {
        include_bytes!("../../../../examples/two-room/world.json")
    } else {
        data.content_namespace = "expedition".into();
        data.entry_room_definition = "expedition:rooms.camp".into();
        include_bytes!("../../../../examples/expedition/world.json")
    };
    data.game = game;
    db.store.seed(&data).await.unwrap();
    let artifact = package_bytes("r2-storage", bytes, PackageOptions {
        release_id:"r2-storage".parse().unwrap(), source_revision:"r2-test".into(), builder_identity:"r2-test".into(), game,
    }).unwrap();
    let digest = Sha256Digest::from_bytes(&artifact);
    let publication = PublicationId::from_u128(701).unwrap();
    db.store.submit_publication(publication, data.world, &artifact, PublicationKind::Activate,
        DeploymentTarget::Staging, None, "requester").await.unwrap();
    db.store.review_publication(publication,"reviewer",ReviewDecision::Approve,"R2 storage test").await.unwrap();
    let plan = db.store.begin_activation(publication,ActivationId::from_u128(702).unwrap(),WriterId::from_u128(703).unwrap(),"operator").await.unwrap();
    db.store.complete_activation(plan).await.unwrap();
    let lease = db.store.acquire_writer(data.world,WriterId::from_u128(704).unwrap()).await.unwrap();
    let mut rooms: Vec<_> = db.store.identities(data.world).await.unwrap().iter().map(|identity|
        DurableCharacterRoom { character:identity.character(), room_definition:identity.room_definition().into() }).collect();
    rooms.sort_by_key(|room|room.character);
    let runtime = DurableWorldState { world:data.world,revision:0,counters:vec![],scheduled:vec![] };
    assert_eq!(db.store.initialize_game_state(lease,digest,game,GameplayState {runtime:&runtime,rooms:&rooms,now:Tick::ZERO}).await.unwrap(),1);
    (db,lease,digest,runtime,rooms)
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_migration_fresh_upgrade_corrupt_future_and_failed_ledgers() {
    let db = TestDb::new().await;
    let migrations = &crate::store::MIGRATOR.migrations;
    let expected = migrations.len();
    assert!(expected > 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations").fetch_one(db.store.pool_for_test()).await.unwrap();
    assert_eq!(count as usize,expected);
    // Rebuild an isolated OLD fixture using exact source SQL/checksums. No source migration is edited.
    sqlx::query(&format!("DROP SCHEMA \"{}\" CASCADE",db.schema)).execute(&db.admin).await.unwrap();
    sqlx::query(&format!("CREATE SCHEMA \"{}\"",db.schema)).execute(&db.admin).await.unwrap();
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY,description TEXT NOT NULL,installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),success BOOLEAN NOT NULL,checksum BYTEA NOT NULL,execution_time BIGINT NOT NULL)")
        .execute(db.store.pool_for_test()).await.unwrap();
    for migration in migrations.iter().take(expected-1) {
        sqlx::raw_sql(migration.sql.as_ref()).execute(db.store.pool_for_test()).await.unwrap();
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES($1,$2,TRUE,$3,0)")
            .bind(migration.version).bind(migration.description.as_ref()).bind(migration.checksum.as_ref()).execute(db.store.pool_for_test()).await.unwrap();
    }
    assert!(db.store.verify_migrations().await.is_err());
    db.store.migrate().await.unwrap();
    db.store.verify_migrations().await.unwrap();
    let last = migrations.last().unwrap();
    sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES($1,'future',TRUE,$2,0)")
        .bind(last.version+1).bind(vec![0u8;48]).execute(db.store.pool_for_test()).await.unwrap();
    assert!(db.store.verify_migrations().await.is_err());
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version=$1").bind(last.version+1).execute(db.store.pool_for_test()).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$2 WHERE version=$1").bind(last.version).bind(vec![0u8;48]).execute(db.store.pool_for_test()).await.unwrap();
    assert!(db.store.verify_migrations().await.is_err());
    assert!(db.store.migrate().await.is_err());
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$2,success=FALSE WHERE version=$1").bind(last.version).bind(last.checksum.as_ref()).execute(db.store.pool_for_test()).await.unwrap();
    assert!(db.store.verify_migrations().await.is_err());
    sqlx::query("UPDATE _sqlx_migrations SET success=TRUE WHERE version=$1").bind(last.version).execute(db.store.pool_for_test()).await.unwrap();
    db.store.verify_migrations().await.unwrap();
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_both_game_contracts_state_replay_conflict_and_authority() {
    for game in [GameContract::Reference,GameContract::Expedition] {
        let (db,lease,digest,mut runtime,mut rooms) = game_case(game).await;
        let alice = actor(&db.store,1,1,"Alice","r2-storage-alice").await;
        let namespace = if game==GameContract::Reference {"reference"} else {"expedition"};
        let key = format!("{namespace}:counters.r2").parse().unwrap();
        runtime.counters.push(CounterState {character:alice.character,key,value:9});
        rooms[0].room_definition = if game==GameContract::Reference {"reference:rooms.observatory"} else {"expedition:rooms.pass"}.into();
        runtime.revision = 1;
        let state = GameplayState {runtime:&runtime,rooms:&rooms,now:Tick::ZERO};
        let mut mutation = gameplay_mutation(lease,digest,&alice,state);
        mutation.game=game;
        let first = db.store.commit_gameplay(mutation).await.unwrap();
        let reopened=db.reconnect_store().await;
        let loaded=reopened.load_game_state(lease,digest,game).await.unwrap();
        assert_eq!((loaded.game,loaded.state_version,loaded.runtime.revision),(game,2,1));
        assert_eq!(loaded.runtime.counters,runtime.counters);
        assert_eq!(loaded.rooms,rooms);
        let character_version:i64=sqlx::query_scalar("SELECT version FROM worldwright_characters WHERE character_id=$1").bind(types::uuid_of(rooms[0].character)).fetch_one(reopened.pool_for_test()).await.unwrap();
        assert_eq!(character_version,2);
        assert!(reopened.initialize_game_state(lease,digest,game,state).await.is_err());
        let mut replay=gameplay_mutation(lease,digest,&alice,state); replay.game=game; replay.expected_state_version=0;
        replay.operation=OperationId::from_u128(999).unwrap(); replay.result=serde_json::json!({"must":"not replace receipt"});
        let replay=reopened.commit_gameplay(replay).await.unwrap();
        assert!(replay.replayed); assert_eq!(replay.operation,first.operation); assert_eq!(replay.result,first.result);
        let mut changed=gameplay_mutation(lease,digest,&alice,state); changed.game=game; changed.intent=b"different intent";
        assert!(matches!(reopened.commit_gameplay(changed).await,Err(PersistenceError::ReceiptConflict)));
        let fresh=reopened.acquire_writer(lease.world(),WriterId::from_u128(705).unwrap()).await.unwrap();
        let mut stale=gameplay_mutation(lease,digest,&alice,state); stale.game=game;
        assert!(matches!(reopened.commit_gameplay(stale).await,Err(PersistenceError::StaleWriter)));
        reopened.revoke_session(alice.token_hash).await.unwrap();
        let mut revoked=gameplay_mutation(fresh,digest,&alice,state); revoked.game=game;
        assert!(matches!(reopened.commit_gameplay(revoked).await,Err(PersistenceError::Unauthorized)));
        let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM worldwright_gameplay_receipts),(SELECT COUNT(*) FROM worldwright_gameplay_audit)")
            .fetch_one(reopened.pool_for_test()).await.unwrap();
        assert_eq!(counts,(1,1));
        reopened.close_for_test().await; db.finish().await;
    }
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_concurrent_state_candidates_have_one_commit() {
    let (db,lease,digest,mut runtime,rooms)=game_case(GameContract::Reference).await;
    let alice=actor(&db.store,1,1,"Alice","r2-concurrent").await; runtime.revision=1;
    let state=GameplayState {runtime:&runtime,rooms:&rooms,now:Tick::ZERO};
    let mut a=gameplay_mutation(lease,digest,&alice,state); a.request_id="r2-concurrent-a";
    let mut b=gameplay_mutation(lease,digest,&alice,state); b.request_id="r2-concurrent-b"; b.operation=OperationId::from_u128(2).unwrap();
    let (a,b)=tokio::join!(db.store.commit_gameplay(a),db.store.commit_gameplay(b));
    assert!(matches!((a,b),(Ok(_),Err(PersistenceError::StaleGameState))|(Err(PersistenceError::StaleGameState),Ok(_))));
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM worldwright_gameplay_receipts),(SELECT COUNT(*) FROM worldwright_gameplay_audit)")
        .fetch_one(db.store.pool_for_test()).await.unwrap();
    assert_eq!(counts,(1,1)); db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_equal_deadlines_and_lock_latency_do_not_slide_recovery() {
    let (db,lease,digest,mut runtime,rooms)=game_case(GameContract::Reference).await;
    let alice=actor(&db.store,1,1,"Alice","r2-clock").await;
    let key: worldwright_core::ids::DefinitionId="reference:counters.r2".parse().unwrap();
    runtime.counters.push(CounterState {character:alice.character,key:key.clone(),value:1});
    runtime.scheduled=(0..2).map(|_|ScheduledCounterState {at:Tick::from_millis(300),character:alice.character,key:key.clone(),delta:1}).collect();
    runtime.revision=1;
    let mut blocker=db.store.pool_for_test().begin().await.unwrap();
    sqlx::query("SELECT world_id FROM worldwright_worlds WHERE world_id=$1 FOR UPDATE").bind(types::uuid_of(lease.world())).fetch_one(&mut *blocker).await.unwrap();
    let mutation=gameplay_mutation(lease,digest,&alice,GameplayState {runtime:&runtime,rooms:&rooms,now:Tick::ZERO});
    let (committed,())=tokio::join!(db.store.commit_gameplay(mutation),async {tokio::time::sleep(Duration::from_millis(600)).await; blocker.rollback().await.unwrap();});
    committed.unwrap();
    let equal:bool=sqlx::query_scalar("SELECT COUNT(DISTINCT due_at)=1 FROM worldwright_scheduled_counter_adjustments").fetch_one(db.store.pool_for_test()).await.unwrap();
    assert!(equal);
    let loaded=db.store.load_game_state(lease,digest,GameContract::Reference).await.unwrap();
    assert_eq!(loaded.runtime.scheduled.len(),2);
    assert!(loaded.runtime.scheduled.iter().all(|event|event.at==Tick::ZERO));
    assert_eq!(loaded.runtime.scheduled[0].delta,1);
    assert_eq!(loaded.runtime.scheduled[1].delta,1);
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_gameplay_audit_failure_rolls_back_every_candidate_write() {
    let (db,lease,digest,mut runtime,rooms)=game_case(GameContract::Reference).await;
    let alice=actor(&db.store,1,1,"Alice","r2-rollback").await;
    sqlx::query("ALTER TABLE worldwright_gameplay_audit ADD CONSTRAINT r2_reject CHECK (state_version<2)").execute(db.store.pool_for_test()).await.unwrap();
    runtime.revision=1;
    let result=db.store.commit_gameplay(gameplay_mutation(lease,digest,&alice,GameplayState {runtime:&runtime,rooms:&rooms,now:Tick::ZERO})).await;
    assert!(result.is_err());
    let loaded=db.store.load_game_state(lease,digest,GameContract::Reference).await.unwrap();
    assert_eq!((loaded.state_version,loaded.runtime.revision),(1,0));
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM worldwright_gameplay_receipts),(SELECT COUNT(*) FROM worldwright_gameplay_audit)").fetch_one(db.store.pool_for_test()).await.unwrap();
    assert_eq!(counts,(0,0)); db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_lost_result_restart_preserves_business_entitlement_uniqueness() {
    // Known committed result loss; NOT an assertion of real wire COMMIT uncertainty.
    let (db,lease)=prepared().await;
    let alice=actor(&db.store,1,1,"Alice","r2-entitlement").await;
    let first=db.store.claim_reward(lease,&alice,"r2-original",OperationId::from_u128(1).unwrap(),reward(1,"r2-business",1)).await.unwrap();
    let restarted=db.reconnect_store().await;
    let replay=restarted.claim_reward(lease,&alice,"r2-original",OperationId::from_u128(2).unwrap(),reward(2,"r2-business",1)).await.unwrap();
    assert!(replay.replayed); assert_eq!(replay.operation,first.operation);
    let duplicate=restarted.claim_reward(lease,&alice,"r2-new-request",OperationId::from_u128(3).unwrap(),reward(3,"r2-business",1)).await.unwrap();
    assert_eq!(duplicate.outcome,RewardOutcome::AlreadyClaimed);
    assert_eq!(restarted.inventory(&alice).await.unwrap().balance,100);
    assert_eq!(restarted.inventory(&alice).await.unwrap().items.len(),1);
    let claims:i64=sqlx::query_scalar("SELECT COUNT(*) FROM worldwright_reward_claims").fetch_one(restarted.pool_for_test()).await.unwrap();
    assert_eq!(claims,1); restarted.close_for_test().await; db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL"]
async fn r2_expiry_fixtures_preserve_lifetime_and_reject_expired_identity() {
    use worldwright_core::ids::InviteId;

    let db = TestDb::new().await;
    db.store.seed(&seed()).await.unwrap();
    let world = seed().world;
    let issuer = "https://r2-expiry.example.invalid";
    let invite_hash = oidc_token_hash("r2-alice-active-invite");
    db.store
        .issue_oidc_invite(OidcInviteSpec {
            invite: InviteId::from_u128(801).unwrap(),
            world,
            username: "alice",
            issuer,
            token_hash: &invite_hash,
            ttl_seconds: 300,
        })
        .await
        .unwrap();
    let session_hash = session_token_hash("r2-expiry-session");
    db.store
        .login_oidc(OidcLoginSpec {
            world,
            issuer,
            subject: "r2-alice",
            invite_hash: Some(&invite_hash),
            session_hash: &session_hash,
            session_ttl_seconds: SESSION_TTL_SECONDS,
            max_sessions: MAX_DURABLE_SESSIONS,
        })
        .await
        .unwrap();
    assert!(db.store.session(world, &session_hash).await.unwrap().is_some());

    // The old fixture must fail specifically at the immutable lifetime CHECK.
    let invalid = sqlx::query("UPDATE worldwright_sessions SET expires_at = statement_timestamp() - INTERVAL '1 second' WHERE token_hash = $1")
        .bind(session_hash.as_slice())
        .execute(db.store.pool_for_test())
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().and_then(|error| error.code()).as_deref(),
        Some("23514")
    );
    sqlx::query("UPDATE worldwright_sessions SET created_at = statement_timestamp() - INTERVAL '2 seconds', expires_at = statement_timestamp() - INTERVAL '1 second' WHERE token_hash = $1")
        .bind(session_hash.as_slice())
        .execute(db.store.pool_for_test())
        .await
        .unwrap();
    let valid_expired: bool = sqlx::query_scalar("SELECT created_at < expires_at AND expires_at < statement_timestamp() AND revoked_at IS NULL FROM worldwright_sessions WHERE token_hash = $1")
        .bind(session_hash.as_slice())
        .fetch_one(db.store.pool_for_test())
        .await
        .unwrap();
    assert!(valid_expired);
    assert!(db.store.session(world, &session_hash).await.unwrap().is_none());

    let bob_invite_hash = oidc_token_hash("r2-bob-expired-invite");
    let bob_invite = InviteId::from_u128(802).unwrap();
    let invalid = sqlx::query("INSERT INTO worldwright_player_invites (invite_id, world_id, account_id, issuer, token_hash, expires_at) VALUES ($1, $2, $3, $4, $5, statement_timestamp() - INTERVAL '1 second')")
        .bind(types::uuid_of(bob_invite))
        .bind(types::uuid_of(world))
        .bind(types::uuid_of(AccountId::from_u128(2).unwrap()))
        .bind(issuer)
        .bind(bob_invite_hash.as_slice())
        .execute(db.store.pool_for_test())
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().and_then(|error| error.code()).as_deref(),
        Some("23514")
    );
    sqlx::query("INSERT INTO worldwright_player_invites (invite_id, world_id, account_id, issuer, token_hash, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, statement_timestamp() - INTERVAL '2 seconds', statement_timestamp() - INTERVAL '1 second')")
        .bind(types::uuid_of(bob_invite))
        .bind(types::uuid_of(world))
        .bind(types::uuid_of(AccountId::from_u128(2).unwrap()))
        .bind(issuer)
        .bind(bob_invite_hash.as_slice())
        .execute(db.store.pool_for_test())
        .await
        .unwrap();
    let bob_session_hash = session_token_hash("r2-expired-invite-must-not-mint");
    assert!(matches!(
        db.store.login_oidc(OidcLoginSpec {
            world,
            issuer,
            subject: "r2-bob",
            invite_hash: Some(&bob_invite_hash),
            session_hash: &bob_session_hash,
            session_ttl_seconds: SESSION_TTL_SECONDS,
            max_sessions: MAX_DURABLE_SESSIONS,
        }).await,
        Err(PersistenceError::InviteInvalid)
    ));
    let valid_unconsumed: bool = sqlx::query_scalar("SELECT created_at < expires_at AND expires_at < statement_timestamp() AND consumed_at IS NULL FROM worldwright_player_invites WHERE invite_id = $1")
        .bind(types::uuid_of(bob_invite))
        .fetch_one(db.store.pool_for_test())
        .await
        .unwrap();
    assert!(valid_unconsumed);
    assert!(db.store.session(world, &bob_session_hash).await.unwrap().is_none());
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM worldwright_accounts), (SELECT COUNT(*) FROM worldwright_oidc_identities), (SELECT COUNT(*) FROM worldwright_oidc_audit)")
        .fetch_one(db.store.pool_for_test())
        .await
        .unwrap();
    assert_eq!(counts, (2, 1, 1));
    db.finish().await;
}
