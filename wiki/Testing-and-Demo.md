# Testing and Demo

Two layers check the contract: Rust tests against the router, and a shell demo against the running server. CI runs both on every push and pull request.

## Rust tests

```bash
cargo test --locked
```

| Test | What it proves |
| --- | --- |
| `parallel_retries_count_once_and_projects_stay_isolated` | 16 concurrent requests with the same event: exactly one `201`, the rest `200`. The same id with different content returns `409`. Alpha counts 1, beta counts 0 until it sends the same id and then counts 1. An unknown key returns `401`. |
| `rejects_requests_that_break_the_contract` | Missing and unknown keys return `401`. Blank ids and ids over 100 characters return `400`. Unknown fields return `422`. A valid event is still accepted. |

The tests call the router in process (`tower::ServiceExt::oneshot`), so they need no network port.

## Demo script

```bash
cargo run --locked          # terminal 1
./scripts/demo.sh           # terminal 2
START_SERVER=1 ./scripts/demo.sh   # or build and start the API from the script
```

The script talks to the real HTTP server and checks:

1. The same event sent 8 times at once counts once (1 created, 7 duplicates, usage + 1).
2. Reusing the id with different content returns `409` and leaves usage unchanged.
3. The same id in the other project is new there, and each project keeps its own count.

It uses a fresh event id per run and compares counts before and after, so it works against a server that already has data. It exits non-zero at the first failed check.

## CI

`.github/workflows/ci.yml` runs, in order: `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and the demo with `START_SERVER=1`.

## Manual checks

`insomnia/metertrail.json` (Insomnia) and `docs/postman.collection.json` (Postman) walk through the same cases. The Postman collection includes assertions for each response.
