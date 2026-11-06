# Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `Address already in use` on start | Something else listens on port 4100 | Stop it, or run with `METERTRAIL_ADDR=127.0.0.1:4101` and point the demo at it with `METERTRAIL_URL=http://127.0.0.1:4101` |
| `./scripts/demo.sh` says `API not reachable` | The API is not running, or runs on another address | Start it with `cargo run --locked`, or use `START_SERVER=1 ./scripts/demo.sh` |
| `cargo test --locked` fails with a lock file error | `Cargo.lock` does not match `Cargo.toml` | Run `cargo update -p <crate>` only if you meant to change dependencies, and commit the new lock file |
| `415 Unsupported Media Type` | Missing `Content-Type: application/json` | Add the header |
| `422` with a message about an unknown field | The body has a field other than `event_id` and `operation` | Remove the extra field |
| `409 event_payload_conflict` | The id was already used with a different `operation` | Generate a new id for each distinct usage |
| Usage went back to 0 | The API restarted; this version keeps events in memory | Expected. Durable storage is the next milestone |
| `permission denied: ./scripts/demo.sh` | The file lost its executable bit | `chmod +x scripts/demo.sh` or run `bash scripts/demo.sh` |
