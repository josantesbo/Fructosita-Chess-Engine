#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)"
cd "$repo_root"

FASTCHESS_BIN="${FASTCHESS_BIN:-fastchess}"
ENGINE_A="${ENGINE_A:-./target/release/fructosita}"
ENGINE_B="${ENGINE_B:-./target/release/fructosita}"
NAME_A="${NAME_A:-Fructosita-A}"
NAME_B="${NAME_B:-Fructosita-B}"
GAMES="${GAMES:-20}"
TC="${TC:-5+0.05}"
THREADS="${THREADS:-1}"
HASH="${HASH:-64}"
CONCURRENCY="${CONCURRENCY:-1}"
OPENINGS="${OPENINGS:-}"
OPENINGS_FORMAT="${OPENINGS_FORMAT:-}"
PGN_OUT="${PGN_OUT:-target/fastchess-regression/games.pgn}"
LOG_OUT="${LOG_OUT:-target/fastchess-regression/fastchess.log}"
MODE="${MODE:-smoke-local}"
SPRT="${SPRT:-}"

out_dir="target/fastchess-regression"
mkdir -p "$out_dir"
mkdir -p "$(dirname "$PGN_OUT")" "$(dirname "$LOG_OUT")"

case "$MODE" in
  smoke-local|required) ;;
  *)
    echo "Unsupported MODE: $MODE (expected smoke-local or required)" >&2
    exit 1
    ;;
esac

fastchess_available=0
if command -v "$FASTCHESS_BIN" >/dev/null 2>&1; then
  fastchess_available=1
fi

if [[ "$MODE" == "required" && "$fastchess_available" -ne 1 ]]; then
  echo "fastchess is required but was not found: FASTCHESS_BIN=$FASTCHESS_BIN" >&2
  exit 1
fi

if [[ -n "$OPENINGS" && ! -f "$OPENINGS" ]]; then
  if [[ "$MODE" == "required" ]]; then
    echo "OPENINGS was set but file does not exist: $OPENINGS" >&2
    exit 1
  fi
fi

sprt_status="off"
if [[ -n "$SPRT" ]]; then
  sprt_status="on"
fi

cat <<CONFIG
Fastchess regression configuration:
  engine A: $NAME_A ($ENGINE_A)
  engine B: $NAME_B ($ENGINE_B)
  TC: $TC
  games: $GAMES
  concurrency: $CONCURRENCY
  hash: $HASH
  threads: $THREADS
  openings: ${OPENINGS:-none}
  openings format: ${OPENINGS_FORMAT:-none}
  SPRT: $sprt_status
  PGN output: $PGN_OUT
  log output: $LOG_OUT
  mode: $MODE
CONFIG

if [[ "$MODE" == "smoke-local" ]]; then
  if [[ "$fastchess_available" -ne 1 ]]; then
    echo "fastchess not found; smoke-local mode is Codex/CI-safe and exits without running matches."
    exit 0
  fi

  echo "smoke-local mode: fastchess is available, but no statistical match is run in Codex/CI."
  exit 0
fi

cmd=(
  "$FASTCHESS_BIN"
  -engine "cmd=$ENGINE_A" "name=$NAME_A" "option.Threads=$THREADS" "option.Hash=$HASH"
  -engine "cmd=$ENGINE_B" "name=$NAME_B" "option.Threads=$THREADS" "option.Hash=$HASH"
  -each "tc=$TC"
  -games "$GAMES"
  -concurrency "$CONCURRENCY"
  -pgnout "$PGN_OUT"
)

if [[ -n "$OPENINGS" ]]; then
  cmd+=( -openings "file=$OPENINGS" )
  if [[ -n "$OPENINGS_FORMAT" ]]; then
    cmd+=( "format=$OPENINGS_FORMAT" )
  fi
fi

if [[ -n "$SPRT" ]]; then
  cmd+=( -sprt "$SPRT" )
fi

printf 'Running fastchess command:' | tee "$LOG_OUT"
printf ' %q' "${cmd[@]}" | tee -a "$LOG_OUT"
printf '\n' | tee -a "$LOG_OUT"

"${cmd[@]}" 2>&1 | tee -a "$LOG_OUT"

if grep -Eiq 'illegal|crash|disconnect|timeout|forfeit|stalled|terminated|error' "$LOG_OUT"; then
  echo "FASTCHESS REGRESSION FAIL: failure pattern found in $LOG_OUT" >&2
  exit 1
fi

echo "FASTCHESS REGRESSION PASS"
