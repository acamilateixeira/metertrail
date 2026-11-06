use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use metertrail::{
    ledger::{Append, EventLedger, MemoryLedger},
    projection::{
        Analytics, FailPoint, MemoryAnalytics, MemoryCheckpoint, ProjectionError, Projector, Window,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};

const ADMIN_KEY: &str = "demo-admin";

#[derive(Clone)]
struct AppState {
    ledger: Arc<dyn EventLedger>,
    analytics: Arc<dyn Analytics>,
    projector: Arc<Projector>,
}

impl AppState {
    fn in_memory(batch_size: usize) -> Self {
        let ledger: Arc<dyn EventLedger> = Arc::new(MemoryLedger::default());
        let analytics: Arc<dyn Analytics> = Arc::new(MemoryAnalytics::default());
        let projector = Arc::new(Projector::new(
            ledger.clone(),
            analytics.clone(),
            Arc::new(MemoryCheckpoint::default()),
            batch_size,
        ));
        Self {
            ledger,
            analytics,
            projector,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    event_id: String,
    operation: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowQuery {
    from: Option<u64>,
    to: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RunOptions {
    fail: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn error(status: StatusCode, code: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": code })))
}

fn project(headers: &HeaderMap) -> Option<&'static str> {
    match headers.get("x-api-key")?.to_str().ok()? {
        "demo-alpha" => Some("alpha"),
        "demo-beta" => Some("beta"),
        _ => None,
    }
}

fn is_admin(headers: &HeaderMap) -> bool {
    headers
        .get("x-admin-key")
        .and_then(|value| value.to_str().ok())
        == Some(ADMIN_KEY)
}

async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<Event>,
) -> (StatusCode, Json<Value>) {
    let Some(project) = project(&headers) else {
        return error(StatusCode::UNAUTHORIZED, "invalid_key");
    };
    if event.event_id.trim().is_empty()
        || event.event_id.len() > 100
        || event.operation.trim().is_empty()
        || event.operation.len() > 100
    {
        return error(StatusCode::BAD_REQUEST, "invalid_event");
    }

    match state
        .ledger
        .append(project, &event.event_id, &event.operation, now())
    {
        Append::Accepted(stored) => (
            StatusCode::CREATED,
            Json(json!({"event_id":stored.event_id,"result":"accepted"})),
        ),
        Append::Duplicate => (
            StatusCode::OK,
            Json(json!({"event_id":event.event_id,"result":"duplicate"})),
        ),
        Append::Conflict => error(StatusCode::CONFLICT, "event_payload_conflict"),
    }
}

/// Usage from the ledger: always current, no time window.
async fn usage(State(state): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    let Some(project) = project(&headers) else {
        return error(StatusCode::UNAUTHORIZED, "invalid_key");
    };
    (
        StatusCode::OK,
        Json(json!({"project":project,"count":state.ledger.count(project),"source":"ledger"})),
    )
}

/// Usage from analytics: by period, and says how far behind it is.
async fn projected_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WindowQuery>,
) -> (StatusCode, Json<Value>) {
    let Some(project) = project(&headers) else {
        return error(StatusCode::UNAUTHORIZED, "invalid_key");
    };
    if let (Some(from), Some(to)) = (query.from, query.to)
        && from >= to
    {
        return error(StatusCode::BAD_REQUEST, "invalid_window");
    }
    let window = Window {
        from: query.from,
        to: query.to,
    };
    let freshness = state.projector.freshness(now());
    (
        StatusCode::OK,
        Json(json!({
            "project": project,
            "count": state.analytics.distinct_events(project, window),
            "physical_rows": state.analytics.physical_rows(project),
            "source": "analytics",
            "window": {"from": query.from, "to": query.to},
            "freshness": {
                "ledger_head": freshness.ledger_head,
                "projected_through": freshness.projected_through,
                "pending_events": freshness.pending_events,
                "oldest_pending_age_secs": freshness.oldest_pending_age_secs,
            },
        })),
    )
}

fn projection_result(
    state: &AppState,
    result: Result<usize, ProjectionError>,
) -> (StatusCode, Json<Value>) {
    let projected_through = state.projector.freshness(now()).projected_through;
    match result {
        Ok(batches) => (
            StatusCode::OK,
            Json(json!({"batches":batches,"projected_through":projected_through})),
        ),
        Err(ProjectionError::InjectedCrash { batch_end }) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "injected_crash",
                "written_through": batch_end,
                "projected_through": projected_through,
            })),
        ),
        Err(ProjectionError::Store(store)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"store_error","detail":store.0})),
        ),
    }
}

/// Runs the projection until it catches up. Optional body:
/// `{"fail":"after_analytics_write"}` stops once between the two writes.
async fn run_projection(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<RunOptions>>,
) -> (StatusCode, Json<Value>) {
    if !is_admin(&headers) {
        return error(StatusCode::UNAUTHORIZED, "invalid_admin_key");
    }
    let options = body.map(|Json(options)| options).unwrap_or_default();
    match options.fail.as_deref() {
        None => {}
        Some("after_analytics_write") => state.projector.inject(FailPoint::AfterAnalyticsWriteOnce),
        Some(_) => return error(StatusCode::BAD_REQUEST, "unknown_fail_point"),
    }
    let result = state.projector.catch_up();
    projection_result(&state, result)
}

async fn replay_projection(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if !is_admin(&headers) {
        return error(StatusCode::UNAUTHORIZED, "invalid_admin_key");
    }
    let result = state.projector.replay();
    projection_result(&state, result)
}

fn app(state: AppState) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok","mode":"local-memory"})) }),
        )
        .route("/v1/events", post(ingest))
        .route("/v1/usage", get(usage))
        .route("/v1/usage/projected", get(projected_usage))
        .route("/admin/projection/run", post(run_projection))
        .route("/admin/projection/replay", post(replay_projection))
        .with_state(state)
}

fn env_number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = std::env::var("METERTRAIL_ADDR").unwrap_or_else(|_| "127.0.0.1:4100".to_owned());
    let batch_size = usize::try_from(env_number("METERTRAIL_BATCH_SIZE", 100)).unwrap_or(100);
    let state = AppState::in_memory(batch_size);

    // Optional background projection. Off by default so the demo controls
    // each step; set METERTRAIL_PROJECT_EVERY_MS to run it on a timer.
    let every = env_number("METERTRAIL_PROJECT_EVERY_MS", 0);
    if every > 0 {
        let projector = state.projector.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(every));
            loop {
                tick.tick().await;
                let projector = projector.clone();
                match tokio::task::spawn_blocking(move || projector.catch_up()).await {
                    Ok(Err(failure)) => eprintln!("projection: {failure}"),
                    Err(panic) => eprintln!("projection task failed: {panic}"),
                    Ok(Ok(_)) => {}
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("MeterTrail local demo at http://{address}");
    axum::serve(listener, app(state)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    fn test_app() -> Router {
        app(AppState::in_memory(4))
    }

    async fn send(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn post_event(app: Router, key: &str, id: &str, operation: &str) -> StatusCode {
        let request = Request::post("/v1/events")
            .header("content-type", "application/json")
            .header("x-api-key", key)
            .body(Body::from(
                json!({"event_id":id,"operation":operation}).to_string(),
            ))
            .unwrap();
        send(app, request).await.0
    }

    async fn get_json(app: Router, uri: &str, key: &str) -> (StatusCode, Value) {
        let request = Request::get(uri)
            .header("x-api-key", key)
            .body(Body::empty())
            .unwrap();
        send(app, request).await
    }

    async fn admin(app: Router, path: &str, key: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut request = Request::post(path).header("x-admin-key", key);
        let body = match body {
            Some(value) => {
                request = request.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        send(app, request.body(body).unwrap()).await
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_retries_count_once_and_projects_stay_isolated() {
        let app = test_app();
        let calls: Vec<_> = (0..16)
            .map(|_| {
                tokio::spawn(post_event(
                    app.clone(),
                    "demo-alpha",
                    "same-id",
                    "GET /orders",
                ))
            })
            .collect();
        let mut created = 0;
        for request in calls {
            let status = request.await.unwrap();
            assert!(matches!(status, StatusCode::CREATED | StatusCode::OK));
            created += usize::from(status == StatusCode::CREATED);
        }
        assert_eq!(created, 1);
        assert_eq!(
            post_event(app.clone(), "demo-alpha", "same-id", "POST /orders").await,
            StatusCode::CONFLICT
        );
        let usage = |key| get_json(app.clone(), "/v1/usage", key);
        assert_eq!(usage("demo-alpha").await.1["count"], 1);
        assert_eq!(usage("demo-beta").await.1["count"], 0);
        assert_eq!(
            post_event(app.clone(), "demo-beta", "same-id", "GET /orders").await,
            StatusCode::CREATED
        );
        assert_eq!(usage("demo-beta").await.1["count"], 1);
        assert_eq!(usage("unknown").await.0, StatusCode::UNAUTHORIZED);
    }

    async fn post_raw(key: Option<&str>, body: &str) -> StatusCode {
        let mut request = Request::post("/v1/events").header("content-type", "application/json");
        if let Some(key) = key {
            request = request.header("x-api-key", key);
        }
        test_app()
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn rejects_requests_that_break_the_contract() {
        let valid = r#"{"event_id":"e-1","operation":"GET /orders"}"#;
        assert_eq!(post_raw(None, valid).await, StatusCode::UNAUTHORIZED);
        assert_eq!(
            post_raw(Some("unknown"), valid).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            post_raw(
                Some("demo-alpha"),
                r#"{"event_id":"  ","operation":"GET /orders"}"#
            )
            .await,
            StatusCode::BAD_REQUEST
        );
        let long_id = format!(
            r#"{{"event_id":"{}","operation":"GET /orders"}}"#,
            "x".repeat(101)
        );
        assert_eq!(
            post_raw(Some("demo-alpha"), &long_id).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            post_raw(
                Some("demo-alpha"),
                r#"{"event_id":"e-1","operation":"GET /orders","units":5}"#
            )
            .await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            post_raw(Some("demo-alpha"), valid).await,
            StatusCode::CREATED
        );
    }

    #[tokio::test]
    async fn the_projected_total_survives_a_crash_and_a_replay() {
        let app = test_app();
        for i in 0..6 {
            post_event(app.clone(), "demo-alpha", &format!("e-{i}"), "GET /orders").await;
        }
        post_event(app.clone(), "demo-beta", "e-0", "GET /orders").await;
        let projected = || get_json(app.clone(), "/v1/usage/projected", "demo-alpha");

        let (_, before) = projected().await;
        assert_eq!(before["count"], 0);
        assert_eq!(before["freshness"]["pending_events"], 7);

        // Batch size 4: the crash happens after writing seq 1..=4.
        let (status, crash) = admin(
            app.clone(),
            "/admin/projection/run",
            ADMIN_KEY,
            Some(json!({"fail":"after_analytics_write"})),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(crash["error"], "injected_crash");
        assert_eq!(crash["written_through"], 4);
        assert_eq!(crash["projected_through"], 0);

        let (status, run) = admin(app.clone(), "/admin/projection/run", ADMIN_KEY, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(run["projected_through"], 7);

        let (_, after) = projected().await;
        assert_eq!(after["count"], 6);
        assert_eq!(after["physical_rows"], 10);
        assert_eq!(after["freshness"]["pending_events"], 0);
        assert_eq!(
            get_json(app.clone(), "/v1/usage", "demo-alpha").await.1["count"],
            6
        );

        let (status, _) = admin(app.clone(), "/admin/projection/replay", ADMIN_KEY, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(projected().await.1["count"], 6);
        assert_eq!(
            get_json(app, "/v1/usage/projected", "demo-beta").await.1["count"],
            1
        );
    }

    #[tokio::test]
    async fn projection_endpoints_check_keys_and_inputs() {
        let app = test_app();
        assert_eq!(
            admin(app.clone(), "/admin/projection/run", "demo-alpha", None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            admin(app.clone(), "/admin/projection/replay", "wrong", None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            admin(
                app.clone(),
                "/admin/projection/run",
                ADMIN_KEY,
                Some(json!({"fail":"somewhere"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get_json(
                app.clone(),
                "/v1/usage/projected?from=200&to=100",
                "demo-alpha"
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let (status, body) = get_json(
            app.clone(),
            "/v1/usage/projected?from=0&to=100",
            "demo-alpha",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["window"], json!({"from":0,"to":100}));
        assert_eq!(
            get_json(app, "/v1/usage/projected", "unknown").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}
