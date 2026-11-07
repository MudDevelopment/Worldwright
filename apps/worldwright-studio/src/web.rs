#![allow(clippy::items_after_test_module)]

use crate::{
    artifact::PackageRequest,
    cli::ServeOptions,
    preview::PreviewManager,
    publication::{
        PublicationClient, PublicationClientError, PublishRequest, validate_publication_view,
        validate_text,
    },
    workspace::{Workspace, WorkspaceError},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot};

const INDEX: &str = include_str!("../assets/index.html");
const CSS: &str = include_str!("../assets/app.css");
const JS: &str = include_str!("../assets/app.js");
#[derive(Clone)]
pub(crate) struct AppState {
    workspace: Workspace,
    preview: PreviewManager,
    publication: Option<PublicationClient>,
    host: String,
    origin: String,
}

pub async fn serve(options: ServeOptions, ws: Workspace) -> Result<(), String> {
    let listener = TcpListener::bind(options.bind)
        .await
        .map_err(|_| "Studio listener failed".to_owned())?;
    let addr = listener
        .local_addr()
        .map_err(|_| "Studio address unavailable".to_owned())?;
    let host = format!("127.0.0.1:{}", addr.port());
    let origin = format!("http://{host}");
    println!("Worldwright Studio: {origin}/");
    let command = crate::preview::resolve_command(options.preview_command.as_ref())?;
    let publication = match (
        options.release_endpoint,
        options.operator_token_env.as_deref(),
    ) {
        (Some(endpoint), Some(name)) => {
            let token = std::env::var(name)
                .map_err(|_| "configured operator token environment variable is unavailable")?;
            Some(
                PublicationClient::new(endpoint, token)
                    .map_err(|_| "publication client configuration is invalid")?,
            )
        }
        (None, None) => None,
        _ => return Err("publication endpoint and token environment must be paired".into()),
    };
    let preview = PreviewManager::new(ws.clone(), command);
    let shutdown_preview = preview.clone();
    let router = router(AppState {
        workspace: ws.clone(),
        preview,
        publication,
        host: host.clone(),
        origin: origin.clone(),
    });
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let result = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let result = tokio::signal::ctrl_c()
                .await
                .map_err(|_| "Studio shutdown signal failed".to_owned());
            let _ = shutdown_tx.send(result);
        })
        .await
        .map_err(|_| "Studio server failed".to_owned());
    let stop_result = tokio::task::spawn_blocking(move || shutdown_preview.stop())
        .await
        .map_err(|_| "preview shutdown failed".to_owned())?;
    if stop_result.is_err() {
        return Err("preview shutdown failed".into());
    }
    match result {
        Err(error) => Err(error),
        Ok(()) => match shutdown_rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("Studio shutdown signal failed".into()),
        },
    }
}

pub(crate) fn router(state: AppState) -> Router {
    let small = Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route("/api/v1/workspace", get(workspace))
        .route("/api/v1/revisions/{revision}/validate", post(validate))
        .route("/api/v1/revisions/{revision}/export", get(export))
        .route(
            "/api/v1/package",
            post(package).layer(DefaultBodyLimit::max(4096)),
        )
        .route("/api/v1/artifacts", get(artifacts))
        .route("/api/v1/publications", post(publish).get(publications))
        .route("/api/v1/publications/{request}", post(refresh_publication))
        .route("/api/v1/preview", get(preview_status))
        .route("/api/v1/preview/start", post(preview_start))
        .route("/api/v1/preview/input", post(preview_input))
        .route("/api/v1/preview/stop", post(preview_stop))
        .layer(DefaultBodyLimit::max(4096));
    let drafts = Router::new()
        .route(
            "/api/v1/draft",
            put(save).layer(DefaultBodyLimit::max(worldwright_content::MAX_INPUT_BYTES)),
        )
        .route(
            "/api/v1/import",
            post(import).layer(DefaultBodyLimit::max(worldwright_content::MAX_INPUT_BYTES)),
        );
    small
        .merge(drafts)
        .layer(middleware::from_fn_with_state(state.clone(), request_gate))
        .layer(middleware::map_response(headers))
        .with_state(state)
}
async fn headers(mut response: Response) -> Response {
    let h = response.headers_mut();
    h.insert("content-security-policy",HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert("cache-control", HeaderValue::from_static("no-store"));
    response
}
async fn request_gate(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    if request.headers().get("host").and_then(|v| v.to_str().ok()) != Some(state.host.as_str()) {
        return error(StatusCode::MISDIRECTED_REQUEST, "invalid host");
    }
    let mutation = matches!(
        request.method(),
        &axum::http::Method::POST
            | &axum::http::Method::PUT
            | &axum::http::Method::PATCH
            | &axum::http::Method::DELETE
    );
    if mutation {
        if request
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            != Some(state.origin.as_str())
        {
            return error(StatusCode::FORBIDDEN, "invalid origin");
        }
        let expected = if matches!(request.uri().path(), "/api/v1/draft" | "/api/v1/import") {
            "application/vnd.worldwright.world+json"
        } else {
            "application/json"
        };
        if request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            != Some(expected)
        {
            return error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported content type",
            );
        }
    }
    next.run(request).await
}
async fn index(State(_state): State<AppState>) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], INDEX).into_response()
}
async fn css(State(_state): State<AppState>) -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS).into_response()
}
async fn js(State(_state): State<AppState>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        JS,
    )
        .into_response()
}
async fn workspace(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || state.workspace.latest()).await {
        Ok(Ok(v)) => json_response(StatusCode::OK, &v),
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "workspace unavailable"),
    }
}
async fn save(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if headers.get("content-type").and_then(|v| v.to_str().ok())
        != Some("application/vnd.worldwright.world+json")
    {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported content type",
        );
    }
    let base = headers
        .get("x-worldwright-draft-revision")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_revision(v).ok());
    let Some(base) = base else {
        return error(StatusCode::BAD_REQUEST, "invalid revision");
    };
    match tokio::task::spawn_blocking(move || state.workspace.save(base, &body)).await {
        Ok(Ok(v)) => json_response(StatusCode::OK, &v),
        Ok(Err(WorkspaceError::Conflict { current_revision })) => json_response(
            StatusCode::CONFLICT,
            &json!({"code":"revision_conflict","current_revision":current_revision}),
        ),
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "draft operation failed"),
    }
}
async fn import(State(_state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if headers.get("content-type").and_then(|v| v.to_str().ok())
        != Some("application/vnd.worldwright.world+json")
    {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported content type",
        );
    }
    match tokio::task::spawn_blocking(move || Workspace::inspect_import(&body)).await {
        Ok(Ok((draft, validation))) => json_response(
            StatusCode::OK,
            &json!({"draft":draft,"validation":validation}),
        ),
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "import operation failed"),
    }
}
async fn validate(State(state): State<AppState>, Path(revision): Path<String>) -> Response {
    let revision = match parse_revision(&revision) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid revision"),
    };
    match tokio::task::spawn_blocking(move || state.workspace.validate_revision(revision)).await {
        Ok(Ok(v)) => json_response(StatusCode::OK, &v),
        Ok(Err(WorkspaceError::RevisionNotFound)) => {
            error(StatusCode::NOT_FOUND, "revision not found")
        }
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "validation failed"),
    }
}
async fn export(State(state): State<AppState>, Path(revision): Path<String>) -> Response {
    let revision = match parse_revision(&revision) {
        Ok(value) => value,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid revision"),
    };
    match tokio::task::spawn_blocking(move || state.workspace.export(revision)).await {
        Ok(Ok(bytes)) => {
            let value = format!("attachment; filename=\"worldwright-draft-r{revision}.json\"");
            (
                [
                    (
                        header::CONTENT_TYPE,
                        "application/vnd.worldwright.world+json",
                    ),
                    (header::CONTENT_DISPOSITION, value.as_str()),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(Err(WorkspaceError::RevisionNotFound)) => {
            error(StatusCode::NOT_FOUND, "revision not found")
        }
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "export failed"),
    }
}

async fn package(State(state): State<AppState>, body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<PackageRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid package request");
    };
    match tokio::task::spawn_blocking(move || state.workspace.package(request)).await {
        Ok(Ok(record)) => json_response(StatusCode::OK, &record),
        Ok(Err(WorkspaceError::Conflict { .. })) => {
            error(StatusCode::CONFLICT, "artifact conflict")
        }
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "package failed"),
    }
}

async fn artifacts(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || state.workspace.artifacts()).await {
        Ok(Ok(records)) => json_response(StatusCode::OK, &records),
        Ok(Err(error)) => map_workspace_error(error),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "artifact listing failed"),
    }
}
async fn publish(State(state): State<AppState>, body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<PublishRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid publication request");
    };
    match tokio::task::spawn_blocking(move || perform_publish(&state, request)).await {
        Ok(Ok(view)) => (StatusCode::CREATED, axum::Json(view)).into_response(),
        Ok(Err(failure)) => failure.into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "publication operation failed",
        ),
    }
}
async fn publications(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || state.workspace.publication_records()).await {
        Ok(Ok(records)) => json_response(StatusCode::OK, &records),
        Ok(Err(workspace_error)) => map_workspace_error(workspace_error),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "publication listing failed",
        ),
    }
}
async fn refresh_publication(
    State(state): State<AppState>,
    Path(request): Path<String>,
) -> Response {
    match tokio::task::spawn_blocking(move || perform_refresh(&state, &request)).await {
        Ok(Ok(view)) => json_response(StatusCode::OK, &view),
        Ok(Err(failure)) => failure.into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "publication refresh failed",
        ),
    }
}

fn perform_publish(
    state: &AppState,
    request: PublishRequest,
) -> Result<worldwright_operations::PublicationView, PublishFailure> {
    let client = state
        .publication
        .as_ref()
        .ok_or(PublishFailure::disabled())?;
    validate_text(&request.reason, 512).map_err(|_| PublishFailure::invalid())?;
    validate_text(&request.change_summary, 1024).map_err(|_| PublishFailure::invalid())?;
    let record = state
        .workspace
        .artifact(&request.artifact_digest)
        .map_err(|_| PublishFailure::not_found("artifact not found"))?;
    let artifact = state
        .workspace
        .artifact_bytes(&request.artifact_digest)
        .map_err(|_| PublishFailure::not_found("artifact not found"))?;
    let receipt = state
        .workspace
        .preview_receipt(&request.artifact_digest)
        .map_err(|_| PublishFailure::conflict("successful preview required"))?;
    let latest = state
        .workspace
        .latest()
        .map_err(|_| PublishFailure::internal())?;
    let stale =
        latest.revision != record.draft_revision || latest.snapshot_sha256 != record.payload_digest;
    if stale && !request.allow_stale {
        return Err(PublishFailure::conflict(
            "artifact is stale relative to the current draft",
        ));
    }
    match request.target {
        worldwright_release::DeploymentTarget::Production => {
            let expected = format!("production:{}", &record.artifact_digest[..12]);
            if request.production_confirmation.as_deref() != Some(expected.as_str()) {
                return Err(PublishFailure::invalid_confirmation());
            }
        }
        worldwright_release::DeploymentTarget::Staging => {
            if request.production_confirmation.is_some() {
                return Err(PublishFailure::invalid_confirmation());
            }
        }
    }
    let correlation = worldwright_core::ids::CorrelationId::from_u128(random_nonzero_id()?)
        .map_err(|_| PublishFailure::unavailable("random generation failed"))?;
    let review_context = worldwright_operations::StudioReviewContext {
        schema_version: 1,
        draft_revision: record.draft_revision,
        snapshot_sha256: record.payload_digest.clone(),
        validation_diagnostic_count: 0,
        preview_artifact_sha256: record.artifact_digest.clone(),
        preview_completed_at: receipt.completed_at,
        change_summary: request.change_summary,
    };
    let metadata = worldwright_operations::ReleaseSubmissionMetadata {
        schema_version: worldwright_operations::RELEASE_SUBMISSION_VERSION,
        kind: request.kind,
        target: request.target,
        expected_current: request.expected_current,
        reason: request.reason,
        correlation_id: correlation.to_string(),
        review_context: review_context.clone(),
    };
    let view = client
        .submit(&metadata, &artifact)
        .map_err(PublishFailure::from_client)?;
    validate_publication_view(&view).map_err(|_| PublishFailure::invalid_response())?;
    if view.release_id != record.release_id
        || view.artifact_digest != record.artifact_digest
        || view.kind != metadata.kind
        || view.target != metadata.target
        || view.expected_current != metadata.expected_current
        || view.status != worldwright_release::PublicationStatus::Requested
        || view.review_context.as_ref() != Some(&review_context)
    {
        return Err(PublishFailure::invalid_response());
    }
    state
        .workspace
        .insert_publication_record(&view)
        .map_err(|_| PublishFailure::internal())?;
    Ok(view)
}

fn perform_refresh(
    state: &AppState,
    request: &str,
) -> Result<worldwright_operations::PublicationView, PublishFailure> {
    let client = state
        .publication
        .as_ref()
        .ok_or(PublishFailure::disabled())?;
    let previous = state
        .workspace
        .publication_record(request)
        .map_err(|_| PublishFailure::not_found("publication request not found"))?;
    let next = client
        .status(request)
        .map_err(PublishFailure::from_client)?;
    validate_publication_view(&next).map_err(|_| PublishFailure::invalid_response())?;
    if previous.request_id != next.request_id {
        return Err(PublishFailure::invalid_response());
    }
    state
        .workspace
        .update_publication_record(&next)
        .map_err(|error| match error {
            WorkspaceError::Conflict { .. } => {
                PublishFailure::conflict("publication status conflicts with local history")
            }
            _ => PublishFailure::internal(),
        })?;
    Ok(next)
}

fn random_nonzero_id() -> Result<u128, PublishFailure> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)
            .map_err(|_| PublishFailure::unavailable("random generation failed"))?;
        let value = u128::from_be_bytes(bytes);
        if value != 0 {
            return Ok(value);
        }
    }
    Err(PublishFailure::unavailable("random generation failed"))
}

#[derive(Debug)]
struct PublishFailure {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl PublishFailure {
    fn disabled() -> Self {
        Self::unavailable("publication is disabled")
    }
    fn invalid() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "invalid_publication",
            message: "publication request is invalid",
        }
    }
    fn invalid_confirmation() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "confirmation_required",
            message: "publication confirmation is invalid",
        }
    }
    fn not_found(message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message,
        }
    }
    fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "publication_conflict",
            message,
        }
    }
    fn unavailable(message: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "publication_unavailable",
            message,
        }
    }
    fn invalid_response() -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "invalid_publication_response",
            message: "publication service returned an invalid response",
        }
    }
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "publication_record_error",
            message: "publication record operation failed",
        }
    }
    fn from_client(error: PublicationClientError) -> Self {
        match error {
            PublicationClientError::Unauthorized => Self {
                status: StatusCode::UNAUTHORIZED,
                code: "publication_unauthorized",
                message: "publication authentication failed",
            },
            PublicationClientError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: "publication_forbidden",
                message: "publication permission denied",
            },
            PublicationClientError::NotFound => Self::not_found("publication request not found"),
            PublicationClientError::Conflict => {
                Self::conflict("publication service reported a conflict")
            }
            PublicationClientError::Unavailable => {
                Self::unavailable("publication service is unavailable")
            }
            PublicationClientError::InvalidRequest => Self::invalid(),
            PublicationClientError::InvalidResponse => Self::invalid_response(),
        }
    }
    fn into_response(self) -> Response {
        (
            self.status,
            axum::Json(json!({"code":self.code,"message":self.message})),
        )
            .into_response()
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewStart {
    artifact_digest: String,
    max_seconds: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewInput {
    text: String,
}
async fn preview_status(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || state.preview.status()).await {
        Ok(status) => json_response(StatusCode::OK, &status),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "preview status failed"),
    }
}
async fn preview_start(State(state): State<AppState>, body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<PreviewStart>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid preview request");
    };
    let result = tokio::task::spawn_blocking(move || {
        state
            .preview
            .start(&request.artifact_digest, request.max_seconds)
    })
    .await;
    match result {
        Ok(Ok(status)) => json_response(StatusCode::OK, &status),
        Ok(Err(message)) if message == "preview is disabled" => {
            error(StatusCode::SERVICE_UNAVAILABLE, &message)
        }
        Ok(Err(message)) if message.contains("already") => error(StatusCode::CONFLICT, &message),
        Ok(Err(message)) => error(StatusCode::UNPROCESSABLE_ENTITY, &message),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "preview start failed"),
    }
}
async fn preview_input(State(state): State<AppState>, body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<PreviewInput>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid preview request");
    };
    match tokio::task::spawn_blocking(move || state.preview.input(&request.text)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(message)) => error(StatusCode::CONFLICT, &message),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "preview input failed"),
    }
}
async fn preview_stop(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || state.preview.stop()).await {
        Ok(Ok(status)) => json_response(StatusCode::OK, &status),
        Ok(Err(message)) => error(StatusCode::CONFLICT, &message),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "preview stop failed"),
    }
}
fn json_response<T: serde::Serialize>(status: StatusCode, value: &T) -> Response {
    (status, axum::Json(value)).into_response()
}
fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(json!({"code":"error","message":message})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use std::{
        io::{Read, Write},
        net::TcpListener as StdTcpListener,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tower::ServiceExt;

    fn state() -> (AppState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "worldwright-studio-web-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = Workspace::open_or_create(&root, None).unwrap();
        (
            AppState {
                preview: PreviewManager::new(workspace.clone(), None),
                publication: None,
                workspace,
                host: "127.0.0.1:3010".into(),
                origin: "http://127.0.0.1:3010".into(),
            },
            root,
        )
    }

    #[tokio::test]
    async fn host_and_origin_are_global_security_gates() {
        let (state, root) = state();
        let router = router(state);
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/workspace")
                    .header("host", "wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);
        assert!(response.headers().contains_key("content-security-policy"));
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/revisions/1/validate")
                    .header("host", "127.0.0.1:3010")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn save_conflict_import_and_export_work() {
        let (state, root) = state();
        let router = router(state);
        let body = br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[{"id":"draft:rooms.start","title":"Start","description":"Place","exits":{}}]}"#;
        let request = axum::http::Request::builder()
            .method("PUT")
            .uri("/api/v1/draft")
            .header("host", "127.0.0.1:3010")
            .header("origin", "http://127.0.0.1:3010")
            .header("content-type", "application/vnd.worldwright.world+json")
            .header("x-worldwright-draft-revision", "1")
            .body(Body::from(body.to_vec()))
            .unwrap();
        assert_eq!(
            router.clone().oneshot(request).await.unwrap().status(),
            StatusCode::OK
        );
        let stale = axum::http::Request::builder()
            .method("PUT")
            .uri("/api/v1/draft")
            .header("host", "127.0.0.1:3010")
            .header("origin", "http://127.0.0.1:3010")
            .header("content-type", "application/vnd.worldwright.world+json")
            .header("x-worldwright-draft-revision", "1")
            .body(Body::from(body.to_vec()))
            .unwrap();
        assert_eq!(
            router.clone().oneshot(stale).await.unwrap().status(),
            StatusCode::CONFLICT
        );
        let export = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/revisions/2/export")
                    .header("host", "127.0.0.1:3010")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(export.status(), StatusCode::OK);
        let bytes = export.into_body().collect().await.unwrap().to_bytes();
        assert!(!bytes.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn host_origin_content_type_and_revision_errors_are_bounded() {
        let (state, root) = state();
        let router = router(state);
        for (host, expected) in [
            ("wrong", StatusCode::MISDIRECTED_REQUEST),
            ("127.0.0.1:3010", StatusCode::NOT_FOUND),
        ] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/unknown")
                        .header("host", host)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            assert!(response.headers().contains_key("content-security-policy"));
        }
        for origin in [None, Some("http://wrong")] {
            let mut request = axum::http::Request::builder()
                .method("PUT")
                .uri("/api/v1/draft")
                .header("host", "127.0.0.1:3010");
            if let Some(value) = origin {
                request = request.header("origin", value);
            }
            let response = router
                .clone()
                .oneshot(
                    request
                        .header("content-type", "application/vnd.worldwright.world+json")
                        .header("x-worldwright-draft-revision", "1")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri("/api/v1/draft")
                    .header("host", "127.0.0.1:3010")
                    .header("origin", "http://127.0.0.1:3010")
                    .header("content-type", "text/plain")
                    .header("x-worldwright-draft-revision", "1")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        for revision in ["0", "01", "%2B1", "%201"] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri(format!("/api/v1/revisions/{revision}/validate"))
                        .header("host", "127.0.0.1:3010")
                        .header("origin", "http://127.0.0.1:3010")
                        .header("content-type", "application/json")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn import_semantic_invalid_is_read_only_and_structural_is_422() {
        let (state, root) = state();
        let router = router(state.clone());
        let semantic=br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[{"id":"draft:rooms.start","title":"","description":"","exits":{}}]}"#;
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/import")
                    .header("host", "127.0.0.1:3010")
                    .header("origin", "http://127.0.0.1:3010")
                    .header("content-type", "application/vnd.worldwright.world+json")
                    .body(Body::from(semantic.to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(state.workspace.latest().unwrap().revision, 1);
        let structural=br#"{"schema_version":1,"namespace":"draft","entry_room":"draft:rooms.start","rooms":[],"unknown":true}"#;
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/import")
                    .header("host", "127.0.0.1:3010")
                    .header("origin", "http://127.0.0.1:3010")
                    .header("content-type", "application/vnd.worldwright.world+json")
                    .body(Body::from(structural.to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn embedded_assets_expose_the_bounded_accessible_workflow() {
        for forbidden in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "eval(",
            "http://",
            "https://",
        ] {
            assert!(
                !INDEX.contains(forbidden) && !CSS.contains(forbidden) && !JS.contains(forbidden),
                "forbidden asset token: {forbidden}"
            );
        }
        for required in [
            "aria-live",
            "skip-link",
            "delete-room-dialog",
            "stale-artifact-dialog",
            "production-dialog",
            "publication-list",
            "preview-output",
            "diagnostic-list",
        ] {
            assert!(INDEX.contains(required), "missing asset marker: {required}");
        }
        assert!(CSS.contains(":focus-visible"));
        assert!(CSS.contains("prefers-reduced-motion"));
        assert!(CSS.contains("@media"));
        for endpoint in [
            "/api/v1/draft",
            "/api/v1/import",
            "/api/v1/package",
            "/api/v1/preview/start",
            "/api/v1/publications",
        ] {
            assert!(JS.contains(endpoint), "missing API wiring: {endpoint}");
        }
    }

    #[test]
    fn exact_previewed_artifact_submits_and_persists_review_context() {
        let (mut state, root) = state();
        let snapshot = state.workspace.latest().unwrap();
        let artifact = state
            .workspace
            .package(PackageRequest {
                revision: snapshot.revision,
                snapshot_sha256: snapshot.snapshot_sha256.clone(),
                release_id: "studio-acceptance".into(),
                builder_identity: "studio-test".into(),
            })
            .unwrap();
        state
            .workspace
            .write_preview_receipt(
                &artifact.artifact_digest,
                &crate::preview::PreviewReceipt {
                    schema_version: 1,
                    artifact_digest: artifact.artifact_digest.clone(),
                    payload_digest: artifact.payload_digest.clone(),
                    draft_revision: artifact.draft_revision,
                    completed_at: 1_700_000_000,
                    result: "succeeded".into(),
                },
            )
            .unwrap();

        let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        state.publication = Some(PublicationClient::new(endpoint, "test-token".into()).unwrap());
        let expected_artifact = artifact.clone();
        let expected_bytes = state
            .workspace
            .artifact_bytes(&artifact.artifact_digest)
            .unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            let header_end = loop {
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
                if let Some(index) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            assert!(headers.starts_with("POST /api/v1/operator/releases HTTP/1.1\r\n"));
            assert!(headers.contains("\r\nAuthorization: Bearer test-token\r\n"));
            let content_length = headers
                .split("\r\n")
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap();
            while request.len() < header_end + content_length {
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
            }
            let decoded = worldwright_operations::decode_release_submission(
                &request[header_end..header_end + content_length],
            )
            .unwrap();
            assert_eq!(decoded.artifact, expected_bytes);
            let view = worldwright_operations::PublicationView {
                request_id: "00000000000000000000000000000022".into(),
                world_id: "00000000000000000000000000000001".into(),
                release_id: expected_artifact.release_id.clone(),
                artifact_digest: expected_artifact.artifact_digest.clone(),
                kind: decoded.metadata.kind,
                target: decoded.metadata.target,
                expected_current: decoded.metadata.expected_current,
                requested_by: "studio-builder".into(),
                status: worldwright_release::PublicationStatus::Requested,
                reviewed_by: None,
                review_reason: None,
                activation_executor: None,
                review_context: Some(decoded.metadata.review_context),
            };
            let body = serde_json::to_vec(&view).unwrap();
            write!(
                stream,
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });

        let view = perform_publish(
            &state,
            PublishRequest {
                artifact_digest: artifact.artifact_digest.clone(),
                kind: worldwright_release::PublicationKind::Activate,
                target: worldwright_release::DeploymentTarget::Staging,
                expected_current: None,
                reason: "acceptance submission".into(),
                change_summary: "Create the initial Studio room".into(),
                production_confirmation: None,
                allow_stale: false,
            },
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(
            view.status,
            worldwright_release::PublicationStatus::Requested
        );
        assert_eq!(
            state.workspace.publication_records().unwrap(),
            vec![view.clone()]
        );
        assert_eq!(
            view.review_context.unwrap().snapshot_sha256,
            snapshot.snapshot_sha256
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
fn parse_revision(value: &str) -> Result<u64, ()> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || value.starts_with('+')
        || value.trim() != value
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(());
    }
    let revision = value.parse::<u64>().map_err(|_| ())?;
    if revision == 0 { Err(()) } else { Ok(revision) }
}
fn map_workspace_error(workspace_error: WorkspaceError) -> Response {
    match workspace_error {
        WorkspaceError::InvalidDraft(report) => json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &json!({
                "code": "invalid_draft",
                "message": "draft is structurally invalid",
                "diagnostics": report.diagnostics(),
                "truncated": report.truncated()
            }),
        ),
        WorkspaceError::Conflict { current_revision } => json_response(
            StatusCode::CONFLICT,
            &json!({"code":"revision_conflict","current_revision":current_revision}),
        ),
        WorkspaceError::RevisionNotFound => error(StatusCode::NOT_FOUND, "revision not found"),
        WorkspaceError::Capacity => error(StatusCode::CONFLICT, "revision capacity reached"),
        WorkspaceError::InvalidWorkspace(_) | WorkspaceError::Io(_) => {
            error(StatusCode::INTERNAL_SERVER_ERROR, "workspace failure")
        }
    }
}
