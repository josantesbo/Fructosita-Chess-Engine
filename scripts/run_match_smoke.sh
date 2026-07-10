#!/usr/bin/env bash
set -euo pipefail

ENGINE="./target/release/fructosita"
LOG_DIR="target/match-smoke"
mkdir -p "$LOG_DIR"

cargo build --release

if [[ ! -x "$ENGINE" ]]; then
  echo "engine binary not found or not executable: $ENGINE" >&2
  exit 1
fi

UCI_LOG="$LOG_DIR/uci-handshake.log"
printf 'uci\nisready\nucinewgame\nquit\n' | "$ENGINE" > "$UCI_LOG"

if ! grep -q 'uciok' "$UCI_LOG"; then
  echo "UCI handshake failed: missing uciok" >&2
  cat "$UCI_LOG" >&2
  exit 1
fi

if ! grep -q 'readyok' "$UCI_LOG"; then
  echo "UCI handshake failed: missing readyok" >&2
  cat "$UCI_LOG" >&2
  exit 1
fi

if command -v fastchess >/dev/null 2>&1; then
  echo "fastchess found; running external arbiter smoke"
  MATCH_LOG="$LOG_DIR/fastchess.log"
  PGN_LOG="$LOG_DIR/fastchess.pgn"

  fastchess \
    -engine cmd="$ENGINE" name=FructositaA \
    -engine cmd="$ENGINE" name=FructositaB \
    -each tc=10+0.1 \
    -rounds 2 \
    -games 2 \
    -repeat \
    -concurrency 1 \
    -pgnout "$PGN_LOG" \
    2>&1 | tee "$MATCH_LOG"

  if grep -Eiq 'illegal|stalled|disconnect|timeout|crash|forfeit|protocol' "$MATCH_LOG" "$PGN_LOG"; then
    echo "fastchess smoke detected a protocol/game failure" >&2
    exit 1
  fi

  echo "external arbiter smoke completed successfully"
else
  echo "fastchess not found; running internal selfplay fallback"
  SELFPLAY_LOG="$LOG_DIR/internal-selfplay.log"
  "$ENGINE" selfplay 80 30 1 2>&1 | tee "$SELFPLAY_LOG"

  if ! grep -q 'Auto-juego completado sin movimientos ilegales' "$SELFPLAY_LOG"; then
    echo "internal selfplay fallback failed" >&2
    exit 1
  fi

  echo "internal selfplay fallback completed successfully"
fi
