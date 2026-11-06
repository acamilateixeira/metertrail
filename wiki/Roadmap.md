# Roadmap

## Ingestion contract — done

- Rust API with `POST /v1/events`, `GET /v1/usage`, `GET /health`
- Retry-safe counting, `409` on conflicting content, isolation by project
- Contract tests, including 16 concurrent retries
- Demo script against the running server, run in CI
- Insomnia and Postman collections, quickstart, MIT license

## Failure and recovery — next

- A storage interface with the in-memory ledger and a Firestore implementation (transactions for check-and-insert)
- A projection job that writes batches to ClickHouse and records its progress
- An injected failure between the ClickHouse write and the progress record, and a test proving the total holds after replay
- Usage by period, counting distinct ids
- The same contract tests run against both storage implementations

## Hosted demo — planned

- API and job on GCP Cloud Run
- OpenTelemetry traces for ingestion and projection
- Infrastructure in Terraform, deployed from GitHub Actions with a smoke test
- A public walkthrough with synthetic traffic: pending event age, p95 ingestion and query latency, and the cost of exact distinct counts

## Not planned

Pricing, invoicing, and payments. MeterTrail measures usage; billing belongs to another system.
