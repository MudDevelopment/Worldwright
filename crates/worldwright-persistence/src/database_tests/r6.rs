//! Native PostgreSQL regressions; never skip a missing/broken migration as success.
use super::*;

fn claim(subject: &str) -> OperatorClaims {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    OperatorClaims {
        issuer: "issuer".into(),
        audience: "audience".into(),
        subject: subject.into(),
        session_id: format!("{:032x}", 800),
        kind: OperatorKind::Human,
        assurance: Assurance::Mfa,
        issued_at: now,
        expires_at: now + 600,
        auth_time: now,
        break_glass_reason: None,
    }
}
fn context(claims: &OperatorClaims) -> OperatorContext<'_> {
    OperatorContext {
        claims,
        environment: Environment::Staging,
        world: WorldId::from_u128(1).unwrap(),
        correlation: CorrelationId::from_u128(900).unwrap(),
        reason: "r6 maintenance test",
    }
}
async fn admin(db: &TestDb, subject: &str) -> OperatorClaims {
    let caps = [
        Capability::ManageOperators,
        Capability::ViewStatus,
        Capability::RestoreWorld,
        Capability::InitiateRecovery,
        Capability::BackupWorld,
    ];
    let ids = (100..105)
        .map(|v| GrantId::from_u128(v).unwrap())
        .collect::<Vec<_>>();
    db.store
        .bootstrap_operator(
            &BootstrapOperator {
                subject,
                display_name: subject,
                kind: OperatorKind::Human,
                environment: Environment::Staging,
                world: WorldId::from_u128(1).unwrap(),
                capabilities: &caps,
                reason: "test bootstrap",
                correlation: CorrelationId::from_u128(100).unwrap(),
            },
            OperationId::from_u128(100).unwrap(),
            &ids,
        )
        .await
        .unwrap();
    claim(subject)
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL and all current migrations"]
async fn r6_retention_is_audited_idempotent_and_preserves_entitlements() {
    let (db, lease) = prepared().await;
    let claims = admin(&db, "admin").await;
    let ctx = context(&claims);
    let expired = actor(&db.store, 1, 1, "Alice", "r6-expired").await;
    let active = actor(&db.store, 2, 2, "Bob", "r6-active").await;
    db.store
        .claim_reward(
            lease,
            &expired,
            "r6-reward",
            OperationId::from_u128(400).unwrap(),
            reward(500, "r6-once", 1),
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE worldwright_sessions SET created_at=clock_timestamp()-INTERVAL '2 days', \
        expires_at=clock_timestamp()-INTERVAL '1 day' WHERE token_hash=$1",
    )
    .bind(expired.token_hash.as_slice())
    .execute(db.store.pool_for_test())
    .await
    .unwrap();
    let before = db.store.retention_status(&ctx).await.unwrap();
    assert_eq!(before.protected_receipts, 1);
    assert!(!before.external_dispatch_supported);
    let operation = OperationId::from_u128(600).unwrap();
    let result = db
        .store
        .prune_expired_authentication(&ctx, 1, operation)
        .await
        .unwrap();
    assert_eq!(result.sessions_deleted, 1);
    assert!(!result.replayed);
    assert!(
        db.store
            .prune_expired_authentication(&ctx, 1, operation)
            .await
            .unwrap()
            .replayed
    );
    assert!(matches!(
        db.store
            .prune_expired_authentication(&ctx, 2, operation)
            .await,
        Err(PersistenceError::RecoveryConflict)
    ));
    assert!(
        db.store
            .session(lease.world(), active.token_hash)
            .await
            .unwrap()
            .is_some()
    );
    let fresh = actor(&db.store, 1, 1, "Alice", "r6-fresh").await;
    let repeated = db
        .store
        .claim_reward(
            lease,
            &fresh,
            "r6-new-request",
            OperationId::from_u128(401).unwrap(),
            reward(501, "r6-once", 1),
        )
        .await
        .unwrap();
    assert_eq!(repeated.outcome, RewardOutcome::AlreadyClaimed);
    let (runs,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM worldwright_retention_runs")
        .fetch_one(db.store.pool_for_test())
        .await
        .unwrap();
    assert_eq!(runs, 1);
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL and all current migrations"]
async fn r6_expired_authority_and_interrupted_jobs_fail_closed() {
    let (db, _) = prepared().await;
    let claims = admin(&db, "admin").await;
    let mut expired = claims.clone();
    expired.issued_at -= 601;
    expired.auth_time -= 601;
    expired.expires_at = claims.issued_at - 1;
    assert!(matches!(
        db.store
            .authorize_operator(&context(&expired), Capability::ViewStatus)
            .await,
        Err(PersistenceError::OperatorDenied)
    ));
    let ctx = context(&claims);
    let job = RecoveryJobId::from_u128(800).unwrap();
    db.store
        .begin_recovery_job(
            &ctx,
            job,
            "backup",
            OperationId::from_u128(801).unwrap(),
            serde_json::json!({"phase":"test"}),
        )
        .await
        .unwrap();
    db.store
        .interrupt_recovery_job(&ctx, job, OperationId::from_u128(802).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        db.store
            .interrupt_recovery_job(&ctx, job, OperationId::from_u128(803).unwrap())
            .await,
        Err(PersistenceError::RecoveryConflict)
    ));
    let state: String =
        sqlx::query_scalar("SELECT state FROM worldwright_recovery_jobs WHERE job_id=$1")
            .bind(uuid::Uuid::from_u128(800))
            .fetch_one(db.store.pool_for_test())
            .await
            .unwrap();
    assert_eq!(state, "failed");
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL and all current migrations"]
async fn r6_grant_revocation_waits_for_the_authority_lock() {
    let (db, _) = prepared().await;
    let claims = admin(&db, "admin").await;
    let ctx = context(&claims);
    db.store
        .create_operator(
            &ctx,
            "worker",
            "Worker",
            OperatorKind::Human,
            OperationId::from_u128(910).unwrap(),
        )
        .await
        .unwrap();
    let grant = GrantId::from_u128(911).unwrap();
    db.store
        .grant(
            &ctx,
            grant,
            "worker",
            Capability::ViewStatus,
            Environment::Staging,
            ctx.world,
            None,
            OperationId::from_u128(912).unwrap(),
        )
        .await
        .unwrap();
    let mut held = db.store.pool_for_test().begin().await.unwrap();
    sqlx::query("SELECT subject FROM worldwright_operators WHERE subject='worker' FOR SHARE")
        .execute(&mut *held)
        .await
        .unwrap();
    let store = db.store.clone();
    let fresh_claims = claims.clone();
    let mut revoke = tokio::spawn(async move {
        store
            .revoke_grant(
                &context(&fresh_claims),
                grant,
                OperationId::from_u128(913).unwrap(),
            )
            .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut revoke)
            .await
            .is_err()
    );
    held.commit().await.unwrap();
    revoke.await.unwrap().unwrap();
    assert!(
        db.store
            .authorize_operator(&context(&claim("worker")), Capability::ViewStatus)
            .await
            .is_err()
    );
    db.finish().await;
}

#[tokio::test]
#[ignore = "requires WORLDWRIGHT_TEST_DATABASE_URL and all current migrations"]
async fn r6_restore_reconciles_current_control_authority_not_restored_grants() {
    let (control, _) = prepared().await;
    let current = admin(&control, "current_admin").await;
    let (target, old_lease) = prepared().await;
    let stale = admin(&target, "stale_admin").await;
    let old_actor = actor(&target.store, 1, 1, "Alice", "r6-restored-token").await;
    target
        .store
        .reconcile_restored_security(
            &control.store,
            &context(&current),
            RecoveryJobId::from_u128(980).unwrap(),
            OperationId::from_u128(981).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        target
            .store
            .authorize_operator(&context(&stale), Capability::ManageOperators)
            .await
            .is_err()
    );
    assert!(
        target
            .store
            .authorize_operator(&context(&current), Capability::ManageOperators)
            .await
            .is_err()
    );
    assert!(matches!(
        target
            .store
            .inventory_with_lease(old_lease, &old_actor)
            .await,
        Err(PersistenceError::StaleWriter)
    ));
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM worldwright_sessions), \
        (SELECT COUNT(*) FROM worldwright_operator_grants WHERE revoked_at IS NULL), \
        (SELECT COUNT(*) FROM worldwright_operators WHERE enabled=TRUE)",
    )
    .fetch_one(target.store.pool_for_test())
    .await
    .unwrap();
    assert_eq!(row, (0, 4, 1));
    let fenced: (i64, Option<uuid::Uuid>) = sqlx::query_as(
        "SELECT writer_epoch,active_writer_id FROM worldwright_worlds WHERE world_id=$1",
    )
    .bind(uuid::Uuid::from_u128(1))
    .fetch_one(target.store.pool_for_test())
    .await
    .unwrap();
    assert_eq!(fenced, (0, None));
    let proof: serde_json::Value = sqlx::query_scalar(
        "SELECT \"authorization\" FROM worldwright_operator_audit WHERE operation_id=$1",
    )
    .bind(uuid::Uuid::from_u128(981))
    .fetch_one(target.store.pool_for_test())
    .await
    .unwrap();
    assert_eq!(proof["required_capabilities"].as_array().unwrap().len(), 4);
    target.finish().await;
    control.finish().await;
}
