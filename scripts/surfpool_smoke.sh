#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SURFPOOL_PORT="${SURFPOOL_PORT:-8899}"
SURFPOOL_WS_PORT="${SURFPOOL_WS_PORT:-8900}"
SURFPOOL_RPC_URL="${SURFPOOL_RPC_URL:-http://127.0.0.1:${SURFPOOL_PORT}}"
ANCHOR_WALLET_PATH="${ANCHOR_WALLET:-$HOME/.config/solana/id.json}"
SURFPOOL_LOG_DIR="${SURFPOOL_LOG_DIR:-.surfpool/logs}"
SURFPOOL_LOG_PATH="${SURFPOOL_LOG_PATH:-${SURFPOOL_LOG_DIR}/surfpool-smoke.log}"
SURFPOOL_PID=""
STARTED_SURFPOOL=0

mkdir -p "$SURFPOOL_LOG_DIR"

cleanup() {
  if [[ "$STARTED_SURFPOOL" == "1" && -n "$SURFPOOL_PID" ]]; then
    kill "$SURFPOOL_PID" >/dev/null 2>&1 || true
    for _ in $(seq 1 10); do
      if ! kill -0 "$SURFPOOL_PID" >/dev/null 2>&1; then
        wait "$SURFPOOL_PID" >/dev/null 2>&1 || true
        return
      fi
      sleep 1
    done
    kill -9 "$SURFPOOL_PID" >/dev/null 2>&1 || true
    wait "$SURFPOOL_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if solana -u "$SURFPOOL_RPC_URL" slot >/dev/null 2>&1; then
  echo "Using existing Surfpool-compatible RPC at ${SURFPOOL_RPC_URL}"
else
  echo "Starting Surfpool mainnet fork at ${SURFPOOL_RPC_URL}"
  surfpool start \
    --network mainnet \
    --no-tui \
    --no-studio \
    --no-deploy \
    --ci \
    --port "$SURFPOOL_PORT" \
    --ws-port "$SURFPOOL_WS_PORT" \
    --airdrop-keypair-path "$ANCHOR_WALLET_PATH" \
    --airdrop-amount 10000000000000 \
    --log-path "$SURFPOOL_LOG_DIR" \
    >"$SURFPOOL_LOG_PATH" 2>&1 &
  SURFPOOL_PID="$!"
  STARTED_SURFPOOL=1

  for _ in $(seq 1 90); do
    if solana -u "$SURFPOOL_RPC_URL" slot >/dev/null 2>&1; then
      break
    fi
    if ! kill -0 "$SURFPOOL_PID" >/dev/null 2>&1; then
      echo "Surfpool exited before RPC became ready. Log follows:" >&2
      sed -n '1,220p' "$SURFPOOL_LOG_PATH" >&2 || true
      exit 1
    fi
    sleep 1
  done

  if ! solana -u "$SURFPOOL_RPC_URL" slot >/dev/null 2>&1; then
    echo "Surfpool RPC did not become ready. Log follows:" >&2
    sed -n '1,220p' "$SURFPOOL_LOG_PATH" >&2 || true
    exit 1
  fi
fi

anchor build
anchor deploy \
  -p meta_amm \
  --program-keypair target/deploy/meta_amm-keypair.json \
  --provider.cluster "$SURFPOOL_RPC_URL" \
  --provider.wallet "$ANCHOR_WALLET_PATH" \
  --commitment confirmed \
  --no-idl

ANCHOR_PROVIDER_URL="$SURFPOOL_RPC_URL" \
ANCHOR_WALLET="$ANCHOR_WALLET_PATH" \
pnpm test:surfpool
