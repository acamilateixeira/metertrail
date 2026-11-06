use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Default)]
struct Ledger(Arc<Mutex<HashMap<&'static str, HashMap<String, String>>>>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    event_id: String,
    operation: String,
}

fn project(headers: &HeaderMap) -> Option<&'static str> {
    match headers.get("x-api-key")?.to_str().ok()? {
        "demo-alpha" => Some("alpha"),
        "demo-beta" => Some("beta"),
        _ => None,
    }
}

async fn ingest(
    State(ledger): State<Ledger>,
    headers: HeaderMap,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    let Some(project) = project(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_key"})),
        );
    };
    if event.event_id.trim().is_empty()
        || event.event_id.len() > 100
        || event.operation.trim().is_empty()
        || event.operation.len() > 100
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_event"})),
        );
    }

    let mut projects = ledger.0.lock().expect("ledger lock");
    let entries = projects.entry(project).or_default();
    match entries.entry(event.event_id) {
        Entry::Vacant(slot) => {
            let body = json!({"event_id":slot.key(),"result":"accepted"});
            slot.insert(event.operation);
            (StatusCode::CREATED, Json(body))
        }
        Entry::Occupied(slot) if slot.get() == &event.operation => (
            StatusCode::OK,
            Json(json!({"event_id":slot.key(),"result":"duplicate"})),
        ),
        Entry::Occupied(_) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"event_payload_conflict"})),
        ),
    }
}

async fn usage(State(ledger): State<Ledger>, headers: HeaderMap) -> impl IntoResponse {
    let Some(project) = project(&headers) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_key"})),
        );
    };
    let projects = ledger.0.lock().expect("ledger lock");
    let count = projects.get(project).map_or(0, HashMap::len);
    (
        StatusCode::OK,
        Json(json!({"project":project,"count":count})),
    )
}

fn app() -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok","mode":"local-memory"})) }),
        )
        .route("/v1/events", post(ingest))
        .route("/v1/usage", get(usage))
        .with_state(Ledger::default())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = "127.0.0.1:4100";
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("MeterTrail local demo at http://{address}");
    axum::serve(listener, app()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    async fn call(
        app: Router,
        key: &str,
        operation: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let request = match operation {
            Some(value) => Request::post("/v1/events")
                .header("content-type", "application/json")
                .header("x-api-key", key)
                .body(Body::from(
                    json!({"event_id":"same-id","operation":value}).to_string(),
                )),
            None => Request::get("/v1/usage")
                .header("x-api-key", key)
                .body(Body::empty()),
        };
        let response = app.oneshot(request.unwrap()).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_retries_count_once_and_projects_stay_isolated() {
        let app = app();
        let calls: Vec<_> = (0..16)
            .map(|_| tokio::spawn(call(app.clone(), "demo-alpha", Some("GET /orders"))))
            .collect();
        let mut created = 0;
        for request in calls {
            let (status, _) = request.await.unwrap();
            assert!(matches!(status, StatusCode::CREATED | StatusCode::OK));
            created += usize::from(status == StatusCode::CREATED);
        }
        assert_eq!(created, 1);
        assert_eq!(
            call(app.clone(), "demo-alpha", Some("POST /orders"))
                .await
                .0,
            StatusCode::CONFLICT
        );
        assert_eq!(call(app.clone(), "demo-alpha", None).await.1["count"], 1);
        assert_eq!(call(app.clone(), "demo-beta", None).await.1["count"], 0);
        assert_eq!(
            call(app.clone(), "demo-beta", Some("GET /orders")).await.0,
            StatusCode::CREATED
        );
        assert_eq!(call(app.clone(), "demo-beta", None).await.1["count"], 1);
        assert_eq!(call(app, "unknown", None).await.0, StatusCode::UNAUTHORIZED);
    }
}
