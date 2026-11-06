# FAQ

**Why does the client choose the event id instead of the server?**
Only the client knows that two requests are the same usage. A server-generated id would be different on every retry. See [Concepts](Concepts).

**Why `200` for a duplicate instead of an error?**
A retry that already succeeded should look like success, so the client stops retrying and no alert fires.

**Why `409` when the same id arrives with different content?**
One id cannot mean two different usages. Keeping the first record and refusing the second makes the client bug visible instead of corrupting the count.

**Can two projects use the same event id?**
Yes. Ids are unique within a project, and the project comes from the API key.

**Is it production ready?**
Not yet. Recovery after a worker crash is implemented and tested, but everything lives in memory and the keys are demo values. The next milestones add durable storage and a hosted deployment. See [Roadmap](Roadmap).

**Why Rust?**
Typed request handling, explicit validation, and predictable performance for a high-volume ingestion path, with a separate batch job planned in the same language.

**Does it bill customers?**
No. MeterTrail measures usage. Pricing and invoicing would sit on top of its counts.

**How is this different from OpenMeter or Lago?**
Those are complete metering and billing platforms. MeterTrail is a focused project about one problem: counting each event once through retries and worker failures, with tests and a demo that prove it.
