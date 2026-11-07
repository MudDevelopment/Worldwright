use crate::{lifecycle::LifecycleController, metrics::Metrics, web::AppState};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use worldwright_core::ids::{CorrelationId, GrantId, OperationId, OperatorSessionId, WorldId};
use worldwright_operations::{
    Capability, Environment, OperatorClaims, OperatorKind, PublicationView,
    decode_release_submission, verify_token,
};
use worldwright_persistence::{OperatorContext, PostgresStore};

#[derive(Clone)]
pub(crate) struct OperatorService {
    pub(crate) store: PostgresStore,
    pub(crate) key: Arc<Vec<u8>>,
    pub(crate) issuer: String,
    pub(crate) audience: String,
    pub(crate) environment: Environment,
    pub(crate) world: WorldId,
    pub(crate) lifecycle: LifecycleController,
    pub(crate) metrics: Metrics,
    pub(crate) drain_gate: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionRevokeBody {
    session_id: String,
    subject: String,
    reason: String,
    correlation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorToggle {
    subject: String,
    enabled: bool,
    reason: String,
    correlation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorCreate {
    subject: String,
    display_name: String,
    kind: OperatorKind,
    reason: String,
    correlation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantBody {
    grant_id: String,
    subject: String,
    capability: Capability,
    environment: Environment,
    world_id: String,
    expires_seconds: Option<u64>,
    reason: String,
    correlation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRevokeBody {
    grant_id: String,
    reason: String,
    correlation_id: String,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/operator/status", get(status))
        .route("/api/v1/operator/metrics", get(metrics))
        .route("/api/v1/operator/moderation", post(moderation))
        .route("/api/v1/operator/sessions/revoke", post(revoke_session))
        .route("/api/v1/operator/drain", post(drain))
        .route("/api/v1/operator/operators/enabled", post(toggle))
        .route("/api/v1/operator/operators", post(create_operator))
        .route("/api/v1/operator/releases/{request}", get(view_release))
        .route("/api/v1/operator/grants", post(grant))
        .route("/api/v1/operator/grants/revoke", post(revoke_grant))
        .layer(DefaultBodyLimit::max(4096))
}

pub(crate) fn submission_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/operator/releases", post(submit_release))
        .layer(DefaultBodyLimit::max(
            worldwright_operations::MAX_RELEASE_SUBMISSION_BYTES,
        ))
}

async fn submit_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    if headers.get("content-type").and_then(|v| v.to_str().ok())
        != Some("application/vnd.worldwright.release-submission-v1")
    {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported media type\n",
        )
            .into_response();
    }
    let (claims, _) = match auth(&headers, &service) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let decoded = match decode_release_submission(&body) {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let correlation = match decoded.metadata.correlation_id.parse() {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let context = context(&service, &claims, correlation, &decoded.metadata.reason);
    let random = match random_u128() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let request = match worldwright_core::ids::PublicationId::from_u128(random) {
        Ok(value) => value,
        Err(_) => return unavailable(),
    };
    let operation = match operation_id() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let result = service
        .store
        .submit_publication_authorized_with_context(
            &context,
            request,
            decoded.artifact,
            decoded.metadata.kind,
            decoded.metadata.target,
            decoded.metadata.expected_current,
            decoded.metadata.review_context,
            operation,
        )
        .await;
    match result {
        Ok(record) => (StatusCode::CREATED, axum::Json(publication_view(&record))).into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn view_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(request): Path<String>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let request = match request.parse() {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let (claims, _) = match auth(&headers, &service) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let random = match random_u128() {
        Ok(value) => value,
        Err(response) => return response,
    };
    let correlation = match CorrelationId::from_u128(random) {
        Ok(value) => value,
        Err(_) => return unavailable(),
    };
    let context = context(&service, &claims, correlation, "view release status");
    match service
        .store
        .publication_authorized(&context, request)
        .await
    {
        Ok(record) => (StatusCode::OK, axum::Json(publication_view(&record))).into_response(),
        Err(error) => map_error(&service, error),
    }
}

fn publication_view(record: &worldwright_persistence::PublicationRecord) -> PublicationView {
    PublicationView {
        request_id: record.request().to_string(),
        world_id: record.world().to_string(),
        release_id: record.release_id().as_str().to_owned(),
        artifact_digest: record.artifact_digest().to_string(),
        kind: record.kind(),
        target: record.target(),
        expected_current: record.expected_current(),
        requested_by: record.requested_by().to_owned(),
        status: record.status(),
        reviewed_by: record.reviewed_by().map(str::to_owned),
        review_reason: record.review_reason().map(str::to_owned),
        activation_executor: record.activation_executor().map(str::to_owned),
        review_context: record.review_context().cloned(),
    }
}

fn service(state: &AppState) -> Result<Arc<OperatorService>, Response> {
    state
        .operator
        .clone()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "not found\n").into_response())
}

fn auth(
    headers: &HeaderMap,
    service: &OperatorService,
) -> Result<(OperatorClaims, CorrelationId), Response> {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            service
                .metrics
                .operator_denied
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            unauthorized()
        })?;
    let token = value
        .strip_prefix("Bearer ")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            service
                .metrics
                .operator_denied
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            unauthorized()
        })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| unavailable())?
        .as_secs();
    let claims = verify_token(token, &service.key, &service.issuer, &service.audience, now)
        .map_err(|_| {
            service
                .metrics
                .operator_denied
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            unauthorized()
        })?;
    let correlation = CorrelationId::from_u128(random_u128()?).map_err(|_| bad_request())?;
    Ok((claims, correlation))
}

fn context<'a>(
    service: &OperatorService,
    claims: &'a OperatorClaims,
    correlation: CorrelationId,
    reason: &'a str,
) -> OperatorContext<'a> {
    OperatorContext {
        claims,
        environment: service.environment,
        world: service.world,
        correlation,
        reason,
    }
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let context = context(&service, &claims, correlation, "view operational status");
    match service.store.operational_status(&context).await {
        Ok(status) => (StatusCode::OK, axum::Json(json!({"lifecycle": format!("{:?}", service.lifecycle.state()).to_lowercase(), "ready": state.world.is_ready(), "connected_clients": state.world.connected_clients(), "active_release": status.active_release.map(|v|v.to_string()), "writer_epoch": Some(status.writer_epoch), "authority_incarnation": Some(status.authority_incarnation.to_string()), "outbox_backlog": Some(status.outbox_backlog), "outbox_oldest_seconds": status.outbox_oldest_seconds}))).into_response(),
        Err(_) => unavailable(),
    }
}

async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let context = context(&service, &claims, correlation, "view operational metrics");
    match service.store.operational_status(&context).await {
        Ok(status) => (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            service.metrics.render(
                service.lifecycle.state() as u8,
                state.world.is_ready(),
                state.world.connected_clients(),
                state.world.admission_rejections(),
                status.outbox_backlog,
                status.outbox_oldest_seconds,
            ),
        )
            .into_response(),
        Err(_) => unavailable(),
    }
}

async fn moderation(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<worldwright_operations::ModerationRequest>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    if body.validate().is_err() {
        return bad_request();
    }
    let (claims, correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let correlation = body
        .correlation_id()
        .map_err(|_| bad_request())
        .unwrap_or(correlation);
    let context = context(&service, &claims, correlation, &body.reason);
    let account = match body.account_id() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let operation = match operation_id() {
        Ok(v) => v,
        Err(v) => return v,
    };
    match service
        .store
        .set_account_disabled(&context, account, body.disabled, operation)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<SessionRevokeBody>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, _auth_correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let session: OperatorSessionId = match body.session_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let correlation: CorrelationId = match body.correlation_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    if body.subject.is_empty() {
        return bad_request();
    }
    let operation = match operation_id() {
        Ok(value) => value,
        Err(value) => return value,
    };
    let context = context(&service, &claims, correlation, &body.reason);
    match service
        .store
        .revoke_operator_session(&context, &body.subject, session, operation)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn drain(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<worldwright_operations::DrainRequest>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    if body.validate().is_err() {
        return bad_request();
    }
    let (claims, _auth_correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let _gate = service.drain_gate.lock().await;
    if service.lifecycle.state() != crate::lifecycle::LifecycleState::Running {
        return (StatusCode::CONFLICT, "already draining\n").into_response();
    }
    let correlation = match body.correlation_id() {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let context = context(&service, &claims, correlation, &body.reason);
    let operation = match operation_id() {
        Ok(value) => value,
        Err(value) => return value,
    };
    if let Err(error) = service
        .store
        .record_drain_started(&context, operation)
        .await
    {
        return map_error(&service, error);
    }
    state.world.begin_drain();
    service.lifecycle.drain();
    service
        .metrics
        .drain
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if tokio::time::timeout(std::time::Duration::from_secs(30), state.world.drain())
        .await
        .is_err()
    {
        return unavailable();
    }
    let completion_operation = match operation_id() {
        Ok(value) => value,
        Err(value) => return value,
    };
    if let Err(error) = service
        .store
        .record_drain_completed(&context, completion_operation)
        .await
    {
        return map_error(&service, error);
    }
    service
        .lifecycle
        .transition(crate::lifecycle::LifecycleState::Quiescent);
    StatusCode::NO_CONTENT.into_response()
}

async fn toggle(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OperatorToggle>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, _auth_correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let correlation: CorrelationId = match body.correlation_id.parse() {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let context = context(&service, &claims, correlation, &body.reason);
    let operation = match operation_id() {
        Ok(v) => v,
        Err(v) => return v,
    };
    match service
        .store
        .set_operator_enabled(&context, &body.subject, body.enabled, operation)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn create_operator(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<OperatorCreate>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, _) = match auth(&headers, &service) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let correlation = match body.correlation_id.parse() {
        Ok(value) => value,
        Err(_) => return bad_request(),
    };
    let context = context(&service, &claims, correlation, &body.reason);
    let operation = match operation_id() {
        Ok(value) => value,
        Err(response) => return response,
    };
    match service
        .store
        .create_operator(
            &context,
            &body.subject,
            &body.display_name,
            body.kind,
            operation,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<GrantBody>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, _auth_correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let grant: GrantId = match body.grant_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let world: WorldId = match body.world_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let correlation: CorrelationId = match body.correlation_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let operation = match operation_id() {
        Ok(v) => v,
        Err(v) => return v,
    };
    let context = context(&service, &claims, correlation, &body.reason);
    match service
        .store
        .grant(
            &context,
            grant,
            &body.subject,
            body.capability,
            body.environment,
            world,
            body.expires_seconds,
            operation,
        )
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

async fn revoke_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<GrantRevokeBody>,
) -> Response {
    let Ok(service) = service(&state) else {
        return (StatusCode::NOT_FOUND, "not found\n").into_response();
    };
    let (claims, _auth_correlation) = match auth(&headers, &service) {
        Ok(v) => v,
        Err(v) => return v,
    };
    let grant: GrantId = match body.grant_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let correlation: CorrelationId = match body.correlation_id.parse() {
        Ok(v) => v,
        Err(_) => return bad_request(),
    };
    let operation = match operation_id() {
        Ok(v) => v,
        Err(v) => return v,
    };
    let context = context(&service, &claims, correlation, &body.reason);
    match service.store.revoke_grant(&context, grant, operation).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => map_error(&service, error),
    }
}

fn random_u128() -> Result<u128, Response> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        if getrandom::fill(&mut bytes).is_ok() {
            let value = u128::from_be_bytes(bytes);
            if value != 0 {
                return Ok(value);
            }
        }
    }
    Err(unavailable())
}
fn operation_id() -> Result<OperationId, Response> {
    OperationId::from_u128(random_u128()?).map_err(|_| unavailable())
}
fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthorized\n").into_response()
}
fn bad_request() -> Response {
    (StatusCode::BAD_REQUEST, "bad request\n").into_response()
}
fn unavailable() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "temporarily unavailable\n").into_response()
}
fn map_error(
    service: &OperatorService,
    error: worldwright_persistence::PersistenceError,
) -> Response {
    if matches!(
        error,
        worldwright_persistence::PersistenceError::OperatorDenied
    ) {
        service
            .metrics
            .operator_denied
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    match error {
        worldwright_persistence::PersistenceError::OperatorDenied => {
            (StatusCode::FORBIDDEN, "forbidden\n").into_response()
        }
        worldwright_persistence::PersistenceError::ReleaseConflict
        | worldwright_persistence::PersistenceError::RecoveryConflict => {
            (StatusCode::CONFLICT, "conflict\n").into_response()
        }
        worldwright_persistence::PersistenceError::InvalidTransition
        | worldwright_persistence::PersistenceError::ExpectedReleaseConflict => {
            (StatusCode::CONFLICT, "conflict\n").into_response()
        }
        worldwright_persistence::PersistenceError::NotFound => {
            (StatusCode::NOT_FOUND, "not found\n").into_response()
        }
        worldwright_persistence::PersistenceError::InvalidData => bad_request(),
        _ => unavailable(),
    }
}
