# Glossary

| Term | Meaning in MeterTrail |
| --- | --- |
| **Event** | One unit of usage reported by a client, such as one API call. |
| **Event id** | The client-chosen name of an event. Reused on every retry of the same event. |
| **Operation** | What was used, for example `GET /orders`. Part of the event content. |
| **Project** | The customer or workspace an API key belongs to. Usage is counted per project. |
| **Ledger** | The store of accepted events, keyed by project and event id, with a global sequence number. In memory today, Firestore in the plan. |
| **Duplicate** | A request with a known event id and the same content. Answered with `200`, not counted again. |
| **Conflict** | A request with a known event id and different content. Refused with `409`. |
| **Idempotency** | Sending the same request more than once has the same effect as sending it once. |
| **Projection** | Copying accepted events into an analytics store (ClickHouse) for time-window queries. |
| **Checkpoint** | The last ledger sequence number the projection finished. Advanced only after the analytics write succeeds. |
| **Replay** | Running a projection batch again after a crash, or the whole ledger from the start. Safe because usage counts distinct ids. |
| **Physical rows** | Rows stored in analytics, duplicates included. Higher than the count after a replay. |
| **Freshness** | How far behind the analytics are, shown as the age of the oldest pending event. |
