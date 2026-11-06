# Operations

How to configure and run the current version.

## Configuration

| Setting | Default | Purpose |
| --- | --- | --- |
| `METERTRAIL_ADDR` | `127.0.0.1:4100` | Address and port the API binds to. Use `0.0.0.0:4100` inside a container. |

API keys are fixed in code for the demo (`demo-alpha`, `demo-beta`). Configurable keys arrive with durable storage.

## Running

```bash
cargo run --locked --release
METERTRAIL_ADDR=0.0.0.0:8080 cargo run --locked --release
```

On start the process prints `MeterTrail local demo at http://<address>`.

## Health check

`GET /health` returns `200 {"mode":"local-memory","status":"ok"}`. Use it for liveness. It does not check storage, because the local version has none.

## Data and restarts

Events live in process memory. A restart, a crash, or a second instance starts from zero, and two instances do not share counts. Do not run more than one instance of this version behind a load balancer.

## Limits

- `event_id` and `operation`: at most 100 characters each.
- Unknown fields are rejected.
- Memory grows with the number of distinct events; there is no eviction.

## Observability

The current version prints only the start-up line. Request tracing with OpenTelemetry is part of the hosted milestone. See [Roadmap](Roadmap).
