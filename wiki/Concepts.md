# Concepts

## Why retries break counting

A client calls the API, the response is lost to a timeout, and the client sends the request again. From the server's side, both requests are real. If each one adds a unit of usage, the customer is billed twice for one action. Usage metering has to survive that path, because it happens every day on real networks.

## Event identity

MeterTrail asks the client for an `event_id` that names the usage, not the request. Every retry of the same usage carries the same id. The server keeps, per project, the id and the content it first saw:

| What arrives | What it means | Answer |
| --- | --- | --- |
| A new id | New usage | Record it, `201` |
| A known id with the same content | A retry | Return the earlier result, `200`; count stays the same |
| A known id with different content | Two different things share one id | Refuse, `409`; neither version replaces the other |

Answering a retry with success (instead of an error) matters: a client that sees an error would keep retrying, and an operator would see false alarms.

## Why a conflict is an error

If the second payload silently replaced the first, the count would stay right but the record would change after the fact. If it were counted as new, the total would be wrong. Refusing with `409` keeps the first record intact and makes the client bug visible.

## Projects and isolation

The API key resolves to a project on the server. Ids are unique **within** a project, so two customers can both send `evt-1` without affecting each other. The client never sends a project id, so it cannot write into another project.

## Concurrency

Retries often arrive at the same time, for example when a client fires a request and its retry races it. The check for an existing id and the insert happen while holding one lock on the ledger, so exactly one of the concurrent requests creates the event. The test suite proves this with 16 concurrent requests.

## Counting

Usage is the number of distinct event ids in the project. Analytics rows may be written twice after a worker crash; the query counts distinct ids, so a duplicate row cannot inflate the total. The tests and step 4 of the demo show this with an injected crash. See [Architecture](Architecture).
