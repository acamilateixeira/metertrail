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
| `200 OK` | `{"count":<number>,"project":"<name>"}` |
| `401 Unauthorized` | `{"error":"invalid_key"}` |

## `GET /health`

```bash
curl http://127.0.0.1:4100/health
# {"mode":"local-memory","status":"ok"}
```

`mode` tells you where events are stored. The local version always reports `local-memory`.
