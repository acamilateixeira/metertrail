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

**Context.** The first milestone proves the contract and its behavior under concurrency.
**Decision.** A `HashMap` per project behind a single `Mutex`; check and insert under the same lock.
**Trade-off.** Not durable, and one lock serializes writes. Both are replaced by the Firestore transaction in the next milestone; the contract and its tests stay the same.

## 7. Count distinct ids, not rows

**Context.** The projection job may write the same batch twice after a crash.
**Decision.** Usage is the number of distinct event ids per project.
**Trade-off.** Exact distinct counts cost more than summing rows. The benchmark in the hosted milestone will measure that cost.
