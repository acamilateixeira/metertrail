# Architecture

## Today: local ingestion contract

```mermaid
flowchart LR
    Client[API client] -->|X-API-Key + event| API[Axum router]
    API --> Auth[Key → project]
    Auth --> Ledger[(In-memory ledger<br/>project → event_id → operation)]
    Ledger --> Usage[GET /v1/usage<br/>distinct ids per project]
```

Everything runs in one process (`src/main.rs`):

| Part | What it does |
| --- | --- |
| Router | `POST /v1/events`, `GET /v1/usage`, `GET /health` |
| Key lookup | Maps `demo-alpha` and `demo-beta` to the `alpha` and `beta` projects |
| Validation | Rejects unknown fields, blank values, and values over 100 characters |
| Ledger | A map of project → (event id → operation) behind one mutex. Check and insert happen under the same lock. |
| Usage | Counts the ids stored for the project |

This is enough to prove the contract and its concurrency behavior. It is not durable: a restart loses every event.

## Planned: cloud pipeline

```mermaid
flowchart LR
    Client[API client] --> API[Rust API on Cloud Run]
    API --> Source[(Firestore event ledger)]
    Source --> Job[Rust projection job]
    Job --> Data[(ClickHouse)]
    Data --> Query[Usage by project and period]
    API --> OTEL[OpenTelemetry]
    Job --> OTEL
```

| Component | Role | Failure it handles |
| --- | --- | --- |
| Firestore ledger | Durable identity of each event per project, written in a transaction | Client retries and concurrent duplicates |
| Projection job | Moves accepted events to ClickHouse in batches and records progress | Crash between the ClickHouse write and the progress record |
| ClickHouse | Time-window queries over usage | Duplicate physical rows after a replay: queries count distinct ids |
| OpenTelemetry | Traces for ingestion and projection | Finding where a slow or failed event went |
| Terraform | Reproducible cloud resources | Drift between environments |

### Freshness

The API confirms durable receipt; analytics catch up asynchronously. The hosted demo will show the age of the oldest pending event and query latency, so an operator can tell whether the total on screen is current.

### Why the job can replay safely

The job may write a batch to ClickHouse and crash before recording its progress. On restart it writes the batch again. The duplicate rows carry the same event ids, and usage counts distinct ids per project, so the total does not change. See [Design Decisions](Design-Decisions).
