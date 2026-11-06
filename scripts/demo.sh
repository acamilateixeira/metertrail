#!/usr/bin/env bash
# Demo of the ingestion contract against a running local API.
#   cargo run --locked            # terminal 1
#   ./scripts/demo.sh             # terminal 2
# Set START_SERVER=1 to build and start the API from this script (used in CI).
set -euo pipefail

BASE="${METERTRAIL_URL:-http://127.0.0.1:4100}"
EVENT="demo-$(date +%s)-$RANDOM"
pass=0

if [[ "${START_SERVER:-0}" == 1 ]]; then
  cargo build --locked --quiet
  ./target/debug/metertrail >/tmp/metertrail-demo.log 2>&1 &
  server=$!
  trap 'kill "$server" 2>/dev/null || true' EXIT
fi
for _ in $(seq 1 50); do curl -fsS "$BASE/health" >/dev/null 2>&1 && break; sleep 0.2; done
curl -fsS "$BASE/health" >/dev/null || { echo "API not reachable at $BASE" >&2; exit 1; }

post() { # key operation -> prints one line: "STATUS BODY"
  local body status
  body=$(mktemp)
  status=$(curl -sS -o "$body" -w '%{http_code}' -X POST "$BASE/v1/events" \
    -H "X-API-Key: $1" -H 'Content-Type: application/json' \
    -d "{\"event_id\":\"$EVENT\",\"operation\":\"$2\"}")
  printf '%s %s\n' "$status" "$(cat "$body")"
  rm -f "$body"
}
count() { # key -> usage count for that project
  local value
  value=$(curl -sS "$BASE/v1/usage" -H "X-API-Key: $1" | sed -nE 's/.*"count": *([0-9]+).*/\1/p')
  [[ "$value" =~ ^[0-9]+$ ]] || { echo "Could not read usage for $1" >&2; exit 1; }
  echo "$value"
}
check() { # description expected actual
  if [[ "$2" == "$3" ]]; then printf '  ✓ %s\n' "$1"; pass=$((pass + 1));
  else printf '  ✗ %s (expected %s, got %s)\n' "$1" "$2" "$3"; exit 1; fi
}

alpha_before=$(count demo-alpha)
beta_before=$(count demo-beta)

echo "1. The same event sent 8 times at once counts once"
statuses=$(for _ in $(seq 1 8); do post demo-alpha 'GET /orders' & done; wait)
created=$(grep -c '^201 ' <<<"$statuses" || true)
duplicates=$(grep -c '^200 ' <<<"$statuses" || true)
check "one request created the event" 1 "$created"
check "seven requests were reported as duplicates" 7 "$duplicates"
check "usage for alpha went up by exactly 1" $((alpha_before + 1)) "$(count demo-alpha)"

echo "2. Reusing the id with different content is a conflict"
response=$(post demo-alpha 'POST /orders')
check "HTTP 409" 409 "${response%% *}"
check "usage for alpha is unchanged" $((alpha_before + 1)) "$(count demo-alpha)"

echo "3. Projects are isolated"
response=$(post demo-beta 'GET /orders')
check "the same id is new for beta (HTTP 201)" 201 "${response%% *}"
check "usage for beta went up by exactly 1" $((beta_before + 1)) "$(count demo-beta)"
check "usage for alpha is unchanged" $((alpha_before + 1)) "$(count demo-alpha)"

echo "All $pass checks passed."
