# MeterTrail wiki

**MeterTrail counts API usage once per event, even when clients retry.** This wiki explains how to run it, the API contract, the design behind it, and what comes next. The [README](https://github.com/acamilateixeira/metertrail#readme) is the short version.

[Português](Inicio)

## Where to start

| If you want to | Read |
| --- | --- |
| Run it and see the guarantees in 5 minutes | [Getting Started](Getting-Started) |
| Integrate a client | [API Reference](API-Reference) |
| Understand why retries do not double count | [Concepts](Concepts) |
| See how the pieces fit, now and in the cloud plan | [Architecture](Architecture) |
| Know why it was built this way | [Design Decisions](Design-Decisions) |
| Check what is tested and how | [Testing and Demo](Testing-and-Demo) |
| Configure and run it | [Operations](Operations) |
| Report a problem or contribute | [Contributing](Contributing) · [Security](Security) |
| Fix a local issue | [Troubleshooting](Troubleshooting) · [FAQ](FAQ) |
| Look up a term | [Glossary](Glossary) |
| Know what is planned | [Roadmap](Roadmap) |

## Project status

| Milestone | Status |
| --- | --- |
| Ingestion contract | **Done.** Rust API, contract tests, and a scripted demo that runs in CI. |
| Failure and recovery | **Done, in memory.** Batch projection with a checkpoint, an injected crash between the two writes, full replay, usage by period, and freshness. |
| Durable storage | Next. Firestore and ClickHouse behind the same interfaces, tested against emulators in CI. |
| Hosted demo | Planned. Cloud Run, OpenTelemetry, and Terraform. |

The current version keeps the ledger, analytics, and checkpoint in memory and uses fixed demo keys. It proves the ingestion contract and the recovery rules; it is not meant for production. See [Roadmap](Roadmap).

## At a glance

- **Language:** Rust (Axum, Tokio, Serde)
- **Endpoints:** `POST /v1/events`, `GET /v1/usage`, `GET /v1/usage/projected`, `POST /admin/projection/run`, `POST /admin/projection/replay`, `GET /health`
- **Guarantee:** an `event_id` counts at most once per project; reusing it with different content returns `409`
- **License:** [MIT](https://github.com/acamilateixeira/metertrail/blob/main/LICENSE)
