# Operations

How to configure and run the current version.

## Configuration

| Setting | Default | Purpose |
| --- | --- | --- |
| `METERTRAIL_ADDR` | `127.0.0.1:4100` | Address and port the API binds to. Use `0.0.0.0:4100` inside a container. |
| `METERTRAIL_BATCH_SIZE` | `100` | Events per projection batch. |
| `METERTRAIL_PROJECT_EVERY_MS` | `0` (off) | Runs the projection on a timer. Off by default so the demo controls each run through `POST /admin/projection/run`. |

API keys are fixed in code for the demo: `demo-alpha` and `demo-beta` for projects, `demo-admin` (header `X-Admin-Key`) for the projection endpoints. Configurable keys arrive with durable storage.

## Running

```bash
cargo run --locked --release
METERTRAIL_ADDR=0.0.0.0:8080 cargo run --locked --release
```

On start the process prints `MeterTrail local demo at http://<address>`.

## Health check

`GET /health` returns `200 {"mode":"local-memory","status":"ok"}`. Use it for liveness. It does not check storage, because the local version has none.

## Data and restarts

The ledger, analytics rows, and checkpoint live in process memory. A restart, a crash, or a second instance starts from zero, and two instances do not share counts. Do not run more than one instance of this version behind a load balancer.

## Projection

- `POST /admin/projection/run` reads the ledger after the checkpoint, writes each batch to analytics, then advances the checkpoint, until nothing is left.
- `{"fail":"after_analytics_write"}` in the body stops the next batch once after the analytics write. The response is `500 injected_crash` and the checkpoint stays where it was. Running again writes that batch a second time.
- `POST /admin/projection/replay` resets the checkpoint to zero and projects the whole ledger again.
- `GET /v1/usage/projected` shows `freshness`: the ledger head, the checkpoint, the number of pending events, and the age of the oldest one.

Only one batch runs at a time inside a process. Two processes running the job against shared storage would need a lease; that comes with durable storage.

## Limits

- `event_id` and `operation`: at most 100 characters each.
- Unknown fields are rejected.
- Memory grows with the number of distinct events and, in analytics, with every replay; there is no eviction.

## Observability

The current version prints only the start-up line. Request tracing with OpenTelemetry is part of the hosted milestone. See [Roadmap](Roadmap).
