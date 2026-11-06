# API Reference

Base URL for the local demo: `http://127.0.0.1:4100`. All responses are JSON.

## Authentication

Send the project key in the `X-API-Key` header. The server maps the key to a project; clients never send a project id. A missing or unknown key returns `401 {"error":"invalid_key"}`.

## `POST /v1/events`

Records one usage event for the key's project.

**Body**

| Field | Type | Rules |
| --- | --- | --- |
| `event_id` | string | Required. Not blank. At most 100 characters. Chosen by the client and reused on every retry of the same event. |
| `operation` | string | Required. Not blank. At most 100 characters. What was used, for example `GET /orders`. |

Any other field is rejected (`422`), so a typo cannot be silently ignored.

**Responses**

| Status | Body | When | Usage |
| --- | --- | --- | --- |
| `201 Created` | `{"event_id":"…","result":"accepted"}` | First time this `event_id` is seen in the project | + 1 |
| `200 OK` | `{"event_id":"…","result":"duplicate"}` | Same `event_id` and same `operation` as before | unchanged |
| `409 Conflict` | `{"error":"event_payload_conflict"}` | Same `event_id`, different `operation` | unchanged |
| `400 Bad Request` | `{"error":"invalid_event"}` | A field is blank or longer than 100 characters | unchanged |
| `401 Unauthorized` | `{"error":"invalid_key"}` | Missing or unknown key | unchanged |
| `422 Unprocessable Entity` | Plain-text rejection from Axum | Valid JSON with a missing field, a wrong type, or an unknown field | unchanged |
| `400 Bad Request` | Plain-text rejection from Axum | Body is not valid JSON | unchanged |
| `415 Unsupported Media Type` | Plain-text rejection from Axum | `Content-Type: application/json` is missing | unchanged |

The body is parsed before the key is checked, so a malformed body returns `400`, `415`, or `422` even when the key is also wrong.

**Example**

```bash
curl -i http://127.0.0.1:4100/v1/events \
  -H 'X-API-Key: demo-alpha' -H 'Content-Type: application/json' \
  -d '{"event_id":"evt-1","operation":"GET /orders"}'
```

### Client guidance

- Generate the `event_id` once, when the usage happens, and keep it with the request. Retries must send the same id.
- Treat `201` and `200` as success. Both mean the event is recorded exactly once.
- Treat `409` as a bug in the client: the same id was used for two different things.
- Retry `5xx` and network errors with the same `event_id`.

## `GET /v1/usage`

Returns the number of distinct events recorded for the key's project.

```bash
curl http://127.0.0.1:4100/v1/usage -H 'X-API-Key: demo-alpha'
# {"count":1,"project":"alpha"}
```

| Status | Body |
| --- | --- |
| `200 OK` | `{"count":<number>,"project":"<name>","source":"ledger"}` |
| `401 Unauthorized` | `{"error":"invalid_key"}` |

This count comes from the ledger, so it is always current. It has no time window.

## `GET /v1/usage/projected`

Usage from analytics, optionally for a period, with how far analytics is behind the ledger.

| Query parameter | Meaning |
| --- | --- |
| `from` | Optional. Unix seconds, inclusive. Compared with the time the API accepted the event. |
| `to` | Optional. Unix seconds, exclusive. Must be greater than `from`. |

```bash
curl 'http://127.0.0.1:4100/v1/usage/projected?from=1793990000' -H 'X-API-Key: demo-alpha'
```

```json
{
  "project": "alpha",
  "count": 6,
  "physical_rows": 10,
  "source": "analytics",
  "window": {"from": 1793990000, "to": null},
  "freshness": {
    "ledger_head": 7,
    "projected_through": 7,
    "pending_events": 0,
    "oldest_pending_age_secs": null
  }
}
```

| Field | Meaning |
| --- | --- |
| `count` | Distinct event ids for the project in the window |
| `physical_rows` | Rows stored for the project, duplicates included. Higher than `count` after a crash or a replay. |
| `freshness.ledger_head` | Highest sequence number in the ledger, all projects |
| `freshness.projected_through` | Checkpoint: the last sequence number the projection finished |
| `freshness.pending_events` | Events accepted but not yet projected |
| `freshness.oldest_pending_age_secs` | Age of the oldest pending event, or `null` when nothing is pending |

| Status | Body |
| --- | --- |
| `200 OK` | As above |
| `400 Bad Request` | `{"error":"invalid_window"}` when `from` is not lower than `to`; plain text for a non-numeric or unknown parameter |
| `401 Unauthorized` | `{"error":"invalid_key"}` |

## `POST /admin/projection/run`

Runs the projection until it catches up with the ledger. Requires `X-Admin-Key: demo-admin`.

Optional body: `{"fail":"after_analytics_write"}` stops the next batch once, after it is written to analytics and before the checkpoint moves.

| Status | Body | When |
| --- | --- | --- |
| `200 OK` | `{"batches":<n>,"projected_through":<seq>}` | Caught up |
| `500 Internal Server Error` | `{"error":"injected_crash","written_through":<seq>,"projected_through":<seq>}` | The injected crash: analytics has rows up to `written_through`, the checkpoint stayed at `projected_through` |
| `400 Bad Request` | `{"error":"unknown_fail_point"}` | Any other `fail` value |
| `401 Unauthorized` | `{"error":"invalid_admin_key"}` | Missing or wrong admin key |

## `POST /admin/projection/replay`

Resets the checkpoint to zero and projects the whole ledger again. Same responses as `run`, without a body. The projected `count` does not change; `physical_rows` grows.

## `GET /health`

```bash
curl http://127.0.0.1:4100/health
# {"mode":"local-memory","status":"ok"}
```

`mode` tells you where events are stored. The local version always reports `local-memory`.
