# Design Decisions

Short records of the choices that shape MeterTrail: the context, the decision, and what it costs.

## 1. The client names the event

**Context.** The server cannot tell a retry from a new call by looking at the request alone.
**Decision.** Every event carries an `event_id` chosen by the client and reused on retries.
**Trade-off.** Clients must generate and keep the id. In return, deduplication is exact instead of guessed from timestamps or payload hashes.

## 2. A retry gets a success, not an error

**Context.** A client that receives an error on a retry keeps retrying and raises alerts.
**Decision.** A known id with the same content returns `200` with `"result":"duplicate"`.
**Trade-off.** The client cannot tell from the status alone whether this call or an earlier one recorded the event. It does not need to: both mean "recorded once".

## 3. Same id, different content is a conflict

**Context.** Two different usages under one id are a client bug.
**Decision.** Return `409` and keep the first record.
**Trade-off.** The second usage is not recorded until the client fixes its ids. Silently overwriting or double counting would hide the bug and corrupt billing.

## 4. The key decides the project

**Context.** A project id in the body could be forged.
**Decision.** The server maps the API key to a project; ids are unique per project.
**Trade-off.** One key per project. Key management (rotation, many keys per project) arrives with durable storage.

## 5. Strict input

**Context.** A misspelled field would otherwise be ignored and an event recorded with missing data.
**Decision.** Unknown fields are rejected, and both fields are limited to 100 characters.
**Trade-off.** Adding a field is a contract change that clients must follow. Limits keep memory bounded and ids readable.

## 6. In-memory ledger behind one lock, for now

**Context.** The first milestones prove the contract and the recovery rules.
**Decision.** The ledger is the `EventLedger` trait. The in-memory implementation keeps the events and an id index behind a single `Mutex`; check and insert happen under the same lock.
**Trade-off.** Not durable, and one lock serializes writes. A Firestore implementation replaces it in the next milestone and must pass the same tests.

## 7. Count distinct ids, not rows

**Context.** The projection job may write the same batch twice after a crash.
**Decision.** Usage is the number of distinct event ids per project.
**Trade-off.** Exact distinct counts cost more than summing rows. The benchmark in the hosted milestone will measure that cost.

## 8. Write analytics first, then the checkpoint

**Context.** Analytics and the checkpoint live in different systems, so one atomic write across both is not available.
**Decision.** The job inserts the batch into analytics and only then advances the checkpoint. A crash in between means the batch is written again on restart.
**Trade-off.** At-least-once delivery into analytics: physical rows can be duplicated. Decision 7 makes that harmless. The opposite order (checkpoint first) would lose events on a crash, and a lost event cannot be detected from the count.

## 9. A global sequence number in the ledger

**Context.** The job needs to know where it stopped, across all projects.
**Decision.** Each accepted event gets the next sequence number. The checkpoint is one number, and `pending_events` is the ledger head minus the checkpoint.
**Trade-off.** One counter is a contention point. In memory it is free; in Firestore a single counter document limits write throughput. The Firestore implementation will need either sharded counters or a read cursor by commit time with a safety lag for late commits. That choice is recorded with the durable storage milestone.

## 10. The failure is part of the product, behind an admin key

**Context.** "It recovers after a crash" is only believable if anyone can cause the crash and watch the recovery.
**Decision.** `POST /admin/projection/run` accepts `{"fail":"after_analytics_write"}`, which stops the next batch once in the exact place that matters. Tests, the demo script, and both API collections use it.
**Trade-off.** A fault switch in the API. It requires the admin key and fires once, and the hosted version will keep it off unless a demo flag enables it.

## 11. Usage by receipt time

**Context.** A usage period needs a timestamp, and the client clock cannot be trusted for billing-adjacent counts.
**Decision.** The ledger stores the second the API accepted the event. Windows are half-open, `[from, to)`, in Unix seconds.
**Trade-off.** An event delayed by retries is counted in the period it arrived, not when it happened on the client. A client timestamp can be added later as a separate field without changing the count rule.
