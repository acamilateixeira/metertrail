# Roadmap

## Ingestion contract — done

- Rust API with `POST /v1/events`, `GET /v1/usage`, `GET /health`
- Retry-safe counting, `409` on conflicting content, isolation by project
- Contract tests, including 16 concurrent retries
- Demo script against the running server, run in CI
- Insomnia and Postman collections, quickstart, MIT license

## Failure and recovery — done, in memory

- `EventLedger`, `Analytics`, and `Checkpoint` traits, with in-memory implementations
- A projection job that writes batches to analytics, then advances the checkpoint
- An injected crash between those two writes, and tests proving the total holds after the restart and after a full replay
- Usage by period, counting distinct ids, with freshness (pending events and the age of the oldest)
- Admin endpoints to run and replay the projection, used by the demo script and both API collections

## Durable storage — next

- Firestore implementation of the ledger and the checkpoint (transactions for check-and-insert; a read cursor that tolerates late commits)
- ClickHouse implementation of analytics (batch inserts, distinct counts by window)
- The same tests run against the in-memory implementations and against the Firestore and ClickHouse emulators in CI

## Hosted demo — planned

- API and job on GCP Cloud Run
- OpenTelemetry traces for ingestion and projection
- Infrastructure in Terraform, deployed from GitHub Actions with a smoke test
- A public walkthrough with synthetic traffic: pending event age, p95 ingestion and query latency, and the cost of exact distinct counts

## Not planned

Pricing, invoicing, and payments. MeterTrail measures usage; billing belongs to another system.
