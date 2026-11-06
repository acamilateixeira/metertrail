# Testing and Demo

Three layers check MeterTrail: unit tests for the ledger and the projection, Rust tests against the router, and a shell demo against the running server. CI runs all of them on every push and pull request.

## Rust tests

```bash
cargo test --locked
```

### Ledger and projection (`src/ledger.rs`, `src/projection.rs`)

| Test | What it proves |
| --- | --- |
| `retries_count_once_and_conflicts_keep_the_first_record` | A retry is a duplicate, other content is a conflict, and the first record wins. |
| `the_same_id_is_independent_per_project` | The same id in two projects is two events. |
| `sequence_numbers_are_dense_and_reads_resume_after_a_position` | Sequence numbers start at 1 with no gaps, and reads resume after a given position. |
| `concurrent_retries_accept_exactly_one` | 16 threads append the same event: one is accepted. |
| `projects_in_batches_and_advances_the_checkpoint` | Batches respect the size limit and the checkpoint follows them. |
| `a_crash_between_the_analytics_write_and_the_checkpoint_does_not_change_the_total` | After the injected crash, analytics has the batch and the checkpoint does not. After the restart, physical rows are higher and the distinct count equals the ledger. |
| `a_full_replay_rebuilds_the_same_totals` | Replaying from zero gives the same count for every project. |
| `usage_by_period_uses_the_receipt_time_and_stays_per_project` | Half-open windows by receipt time, per project. |
| `freshness_reports_pending_events_and_the_age_of_the_oldest` | Pending events and the age of the oldest one. |
| `retries_into_the_ledger_never_reach_analytics_twice_without_a_crash` | Without a crash, a retried event produces one analytics row. |
| `the_checkpoint_never_moves_back` | The checkpoint refuses to go backwards. |

These modules use only the standard library, so they can also be compiled and run with `rustc --edition 2024 --test src/lib.rs`.

### HTTP (`src/main.rs`)

| Test | What it proves |
| --- | --- |
| `parallel_retries_count_once_and_projects_stay_isolated` | 16 concurrent requests with the same event: exactly one `201`, the rest `200`. The same id with different content returns `409`. Alpha counts 1, beta counts 0 until it sends the same id and then counts 1. An unknown key returns `401`. |
| `rejects_requests_that_break_the_contract` | Missing and unknown keys return `401`. Blank ids and ids over 100 characters return `400`. Unknown fields return `422`. A valid event is still accepted. |
| `the_projected_total_survives_a_crash_and_a_replay` | Through HTTP: injected crash returns `500` with the checkpoint unchanged, the next run catches up, the projected count equals the ledger with more physical rows, and a replay keeps the count. |
| `projection_endpoints_check_keys_and_inputs` | Admin endpoints refuse project keys, unknown fail points return `400`, and an inverted window returns `400`. |

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
4. With three new events, the projection crashes after the analytics write (`500`), runs again (`200`), and the projected count equals the ledger with nothing pending. A full replay keeps the count.

It uses a fresh event id per run and compares counts before and after, so it works against a server that already has data. It exits non-zero at the first failed check.

## CI

`.github/workflows/ci.yml` runs, in order: `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and the demo with `START_SERVER=1`.

## Manual checks

`insomnia/metertrail.json` (Insomnia) and `docs/postman.collection.json` (Postman) walk through the same cases. The Postman collection includes assertions for each response, including the crash, the restart, and the replay.
