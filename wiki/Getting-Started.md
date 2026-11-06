# Getting Started

Run the API locally and watch the three guarantees hold. You need stable Rust and Cargo, `curl`, and a POSIX shell for the demo script.

## 1. Clone, test, and run

```bash
git clone https://github.com/acamilateixeira/metertrail.git
cd metertrail
cargo test --locked   # contract tests, including 16 concurrent retries
cargo run --locked    # serves http://127.0.0.1:4100
```

`--locked` uses the exact dependency versions in `Cargo.lock`, the same ones CI uses.

## 2. Send an event twice

```bash
curl -i http://127.0.0.1:4100/v1/events \
  -H 'X-API-Key: demo-alpha' -H 'Content-Type: application/json' \
  -d '{"event_id":"evt-1","operation":"GET /orders"}'
```

The first call returns `201` with `"result":"accepted"`. Run it again: `200` with `"result":"duplicate"`.

```bash
curl http://127.0.0.1:4100/v1/usage -H 'X-API-Key: demo-alpha'
# {"count":1,"project":"alpha"}
```

## 3. Run the demo

In a second terminal, with the API still running:

```bash
./scripts/demo.sh
```

It sends the same event 8 times at once, reuses the id with different content, and repeats the id in the second project. It prints a check per guarantee and stops at the first failure. To build and start the API from the script itself, as CI does, use `START_SERVER=1 ./scripts/demo.sh`.

## 4. Explore by hand

Import `insomnia/metertrail.json` into Insomnia or `docs/postman.collection.json` into Postman. Both follow the same steps as the demo.

## Demo keys

| Key | Project |
| --- | --- |
| `demo-alpha` | `alpha` |
| `demo-beta` | `beta` |

Events live in memory: restarting the API resets both counts.

To see the recovery, run `./scripts/demo.sh`. Step 4 injects a crash in the projection, runs it again, replays everything, and checks that the projected total still equals the ledger.

Next: [API Reference](API-Reference) · [Concepts](Concepts)
