#!/usr/bin/env bash
# The humpyard demo: one model name, routed by what each request is, with the cost compared.
# Needs no API keys and no network: a local mock provider answers every call. Needs python3, jq and
# a humpyard binary (HUMPYARD=/path/to/humpyard, default: the one on PATH).
#
#   examples/demo/run.sh                # runs straight through
#   DEMO_PACE=1 examples/demo/run.sh    # types the commands and pauses (used for the recording)
#
# Tokens come from the mock provider; prices come from demo.toml. Swap in real providers and the
# same comparison comes from your real traffic (see docs/budgets.md, "Statistics").
set -euo pipefail
# A relative HUMPYARD path is relative to where the command was run, not to this directory.
case "${HUMPYARD:-}" in "" | /*) ;; */*) HUMPYARD="$PWD/$HUMPYARD" ;; esac
cd "$(dirname "$0")"

BIN=${HUMPYARD:-humpyard}
PACE=${DEMO_PACE:-0}
PORT=8090
MOCK=9100
URL="http://127.0.0.1:$PORT"
WORK=$(mktemp -d)
GW_PID=""
MOCK_PID=""

cleanup() {
  [ -n "$GW_PID" ] && { kill "$GW_PID" 2>/dev/null; wait "$GW_PID" 2>/dev/null; } || true
  [ -n "$MOCK_PID" ] && { kill "$MOCK_PID" 2>/dev/null; wait "$MOCK_PID" 2>/dev/null; } || true
  rm -rf "$WORK"
}
trap cleanup EXIT

bold=$'\033[1m'; dim=$'\033[2m'; cyan=$'\033[36m'; green=$'\033[32m'; amber=$'\033[33m'; off=$'\033[0m'

pause() { [ "$PACE" = 0 ] || sleep "$(awk "BEGIN{print $1*$PACE}")"; }

# Prints "$ <command>" as if typed, so a viewer sees what is being run.
typed() {
  printf '%s$ %s' "$dim" "$off"
  if [ "$PACE" = 0 ]; then printf '%s\n' "$1"; return; fi
  local i
  for ((i = 0; i < ${#1}; i++)); do printf '%s' "${1:i:1}"; sleep 0.018; done
  printf '\n'; sleep 0.4
}

scene() { printf '\n%s%s%s\n' "$bold$cyan" "$1" "$off"; pause 0.8; }

# ---- setup (quiet) ----
mk_key() { # prints "<key> <hash>" for a fresh key
  local out; out=$("$BIN" keygen "$1")
  printf '%s %s\n' "$(sed -n 's/^Key[^:]*: //p' <<<"$out")" "$(sed -n 's/^sha256 = "\(.*\)"/\1/p' <<<"$out")"
}
read -r KEY_TOUR HASH_TOUR < <(mk_key tour)
read -r KEY_BEFORE HASH_BEFORE < <(mk_key before)
read -r KEY_AFTER HASH_AFTER < <(mk_key after)
sed -e "s#__LEDGER__#$WORK/ledger.db#" -e "s#__BEFORE_HASH__#$HASH_BEFORE#" \
    -e "s#__AFTER_HASH__#$HASH_AFTER#" -e "s#__TOUR_HASH__#$HASH_TOUR#" demo.toml > "$WORK/demo.toml"

python3 mock_provider.py "$MOCK" & MOCK_PID=$!
DEMO_KEY=demo "$BIN" serve --config "$WORK/demo.toml" > "$WORK/gateway.log" 2>&1 & GW_PID=$!
for _ in $(seq 1 100); do curl -sf "$URL/healthz" >/dev/null 2>&1 && break; sleep 0.1; done
curl -sf "$URL/healthz" >/dev/null || { echo "the gateway did not start; see $WORK/gateway.log" >&2; cat "$WORK/gateway.log" >&2; exit 1; }

TOOLS='[{"type":"function","function":{"name":"read_file","description":"Read a file","parameters":{"type":"object","properties":{"path":{"type":"string"}}}}}]'
LONG=$(python3 -c "print('The quick brown fox jumps over the lazy dog. ' * 3600)")

# send <key> <model> <prompt> [tools] [profile]: one request; sets ROUTE, TARGET, PROVIDER, RULE.
send() {
  local key=$1 model=$2 prompt=$3 tools=${4:-} profile=${5:-} body
  if [ -n "$tools" ]; then
    body=$(jq -nc --arg m "$model" --arg p "$prompt" --argjson t "$TOOLS" '{model:$m,messages:[{role:"user",content:$p}],tools:$t,max_tokens:64}')
  else
    body=$(jq -nc --arg m "$model" --arg p "$prompt" '{model:$m,messages:[{role:"user",content:$p}],max_tokens:64}')
  fi
  local extra=(); [ -n "$profile" ] && extra=(-H "x-humpyard-profile: $profile")
  curl -s -D "$WORK/headers" -o /dev/null "$URL/v1/chat/completions" \
    -H "Authorization: Bearer $key" -H 'content-type: application/json' ${extra[@]+"${extra[@]}"} -d "$body"
  ROUTE=$(tr -d '\r' < "$WORK/headers" | awk -F': ' 'tolower($1)=="x-humpyard-route"{print $2}')
  TARGET=$(tr -d '\r' < "$WORK/headers" | awk -F': ' 'tolower($1)=="x-humpyard-target"{print $2}')
  RULE=$(tr -d '\r' < "$WORK/headers" | awk -F': ' 'tolower($1)=="x-humpyard-rule"{print $2}')
}

show() { # show <label> : prints the result of the last send
  printf '  %-38s %s%-13s%s -> %s%-14s%s %s(rule: %s)%s\n' "$1" "$green" "$ROUTE" "$off" "$amber" "$TARGET" "$off" "$dim" "$RULE" "$off"
}

# ---- scene 1: one model name, different requests ----
scene "1. Clients ask for one model: \"auto\". humpyard picks the pool by what the request is."
typed 'curl $URL/v1/chat/completions -H "Authorization: Bearer $KEY" -d '"'"'{"model":"auto", ...}'"'"
pause 0.6
send "$KEY_TOUR" auto "Explain what a hump yard is in one sentence.";                 show "a plain question"
pause 0.7
send "$KEY_TOUR" auto "Fix the failing test in src/lib.rs" tools;                      show "a turn that uses tools"
pause 0.7
send "$KEY_TOUR" auto "Summarize my private notes." "" private;                         show "a private request (profile: private)"
pause 0.7
send "$KEY_TOUR" auto "$LONG";                                                          show "a 40,000-token prompt"
pause 1.2

# ---- scene 2: why ----
scene "2. Not sure why? Ask before you send. Nothing is called and nothing is spent."
typed "curl \$URL/v1/route/explain -H \"Authorization: Bearer \$KEY\" -d '{\"model\":\"auto\",\"tools\":true}'"
curl -s "$URL/v1/route/explain" -H "Authorization: Bearer $KEY_TOUR" -H 'content-type: application/json' \
  -d '{"model":"auto","tools":true,"prompt_tokens":1200}' > "$WORK/explain.json"
jq -r '.selected.rule as $sel | .rules[] |
  "    \((.rule + "                ")[0:16]) " + (
    if .rule == $sel then "\u001b[32mmatched\u001b[0m -> route \(.route)"
    elif .matched then "also matches, but the first match wins"
    else "\u001b[2mskipped: \(.mismatches[0])\u001b[0m" end)' "$WORK/explain.json"
pause 1.5

# ---- scene 3: the cost ----
scene "3. The same 11 requests, two ways: everything on one big model, or routed by humpyard."
mix() { # mix <key> <model>: the same traffic; the model name is the only difference
  local key=$1 model=$2 q
  for q in "What is a hump yard?" "Summarize this error message." "Rename a variable for me." \
           "Is this regex correct?" "Give me a commit message."; do send "$key" "$model" "$q" >/dev/null; printf '.'; done
  for q in "Fix the failing test." "Add a unit test." "Refactor this function."; do send "$key" "$model" "$q" tools >/dev/null; printf '.'; done
  send "$key" "$model" "$LONG" >/dev/null; printf '.'
  for q in "Summarize my private notes." "Draft a reply to my private email."; do send "$key" "$model" "$q" "" private >/dev/null; printf '.'; done
  printf '\n'
}
typed "# before: all 11 requests ask for the one big model (\"capable\")"
printf '  '; mix "$KEY_BEFORE" capable
typed "# after: the same 11 ask for \"auto\""
printf '  '; mix "$KEY_AFTER" auto
sleep 0.5
pause 0.8

typed 'curl "$URL/v1/stats?group_by=route" -H "Authorization: Bearer $KEY"'
stats() { curl -s "$URL/v1/stats?window=1h&group_by=route" -H "Authorization: Bearer $1"; }
stats "$KEY_BEFORE" > "$WORK/before.json"
stats "$KEY_AFTER" > "$WORK/after.json"
total() { jq '[.data[] | (.requests * (.avg_cost_usd // 0))] | add // 0' "$1"; }
BEFORE=$(total "$WORK/before.json"); AFTER=$(total "$WORK/after.json")

printf '\n  %s%-22s %-9s %-10s %s%s\n' "$bold" "after: pool (route)" "requests" "avg cost" "p50 latency" "$off"
jq -r '.data | sort_by(-.requests)[] | "  \(.group | (. + "                      ")[0:22]) \(.requests | tostring | (. + "         ")[0:9]) $\(((.avg_cost_usd // 0) * 10000 | round / 10000) | tostring | (. + "          ")[0:9]) \(.latency_ms.p50) ms"' "$WORK/after.json"
pause 1.0
usd() { awk -v v="$1" 'BEGIN{printf "%.4f", v}'; }
saved() { awk -v b="$1" -v a="$2" 'BEGIN{printf "%.0f", (b-a)/b*100}'; }
printf '\n  %sone big model:%s       $%s\n' "$bold" "$off" "$(usd "$BEFORE")"
printf '  %srouted by humpyard:%s  $%s   %ssaved %s%%%s\n' "$bold" "$off" "$(usd "$AFTER")" "$green$bold" "$(saved "$BEFORE" "$AFTER")" "$off"
printf '\n  %sTokens come from a local mock provider; prices are in examples/demo/demo.toml.%s\n' "$dim" "$off"
printf '  %sPoint it at your real providers to get this table from your own traffic.%s\n\n' "$dim" "$off"
pause 2
