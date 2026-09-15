#!/usr/bin/env bash
# Cụm `evm-validate-fixed-then-wire` (D3) — chạy PAPER 30 phút (mặc định) với
# cấu hình ngưỡng-0 + universal + USDT + động cơ EVM, rồi in đủ số liệu funnel/
# skip/tax/validate/sim.evm. Script DÙNG CHUNG cho WSL (máy dev) và VPS (máy
# chạy thật) — không có bước SSH/deploy nào ở đây, chỉ chạy TẠI CHỖ trong thư
# mục repo đã có `.env` thật. Muốn đưa source lên VPS trước, dùng
# `scripts/deploy_vps.sh` (Linux/macOS) hoặc `scripts/deploy_vps.ps1`
# (Windows) — 2 script đó CÓ SSH, script này thì KHÔNG.
#
# Script KHÔNG bật live/armed, KHÔNG gửi tx, KHÔNG in secret.
#
# Usage (trong thu muc repo da co .env that va da/se `cargo build --release`):
#   scripts/paper_run.sh [--minutes 30] [--port 8799]
#
# Yeu cau: .env co BSC_HTTP/BSC_WS that.
# Script tao 1 config TAM (khong dung config.toml that cua Chu) voi nguong 0.
#
# CLAUDE.md luat "3 luat bo sung 2026-09-15" (#2): moi so lieu runtime phai
# ghi ro chay o dau (WSL/VPS) + hash binary — script nay tu in ca 2 thu do
# ngay dau output ket qua, dan nguyen vao BAOCAO.
set -euo pipefail

MINUTES=30
PORT=8799
while [ $# -gt 0 ]; do
  case "$1" in
    --minutes) MINUTES="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    *) echo "tham so la: $1"; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# ---- xac dinh may dang chay (WSL hay VPS/Linux thuong) — luat #2 ----
RUN_ENV="VPS"
if grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null; then
  RUN_ENV="WSL"
fi
echo "== may chay: $RUN_ENV (repo: $ROOT) =="

# ---- verify BSC_WS host = publicnode, KHONG in URL/token ----
if [ -f .env ]; then
  # shellcheck disable=SC1091
  set +u; . ./.env 2>/dev/null || true; set -u
fi
if [ -n "${BSC_WS:-}" ]; then
  WS_HOST="$(printf '%s' "$BSC_WS" | sed -E 's#^[a-z]+://##; s#[:/].*$##')"
  echo "BSC_WS host = $WS_HOST (KHONG in URL day du)"
  case "$WS_HOST" in
    *publicnode*) echo "  -> khop publicnode, OK" ;;
    *) echo "  -> CANH BAO: host khong phai publicnode (van chay, chi ghi nhan)" ;;
  esac
else
  echo "CANH BAO: BSC_WS rong trong .env — pending se chi qua inject/txpool"
fi

# ---- build release ----
echo "== cargo build --release =="
cargo build --release

# ---- hash binary vua build — luat #2 (moi so lieu runtime phai kem hash) ----
BIN_SHA="$(sha256sum target/release/bsc_sandwich | awk '{print $1}')"
GIT_HEAD="$(git rev-parse HEAD 2>/dev/null || echo MISSING)"
echo "== binary sha256 = $BIN_SHA =="
echo "== git HEAD = $GIT_HEAD =="

# ---- xoa halt.lock (neu con tu lan truoc) ----
mkdir -p state logs
rm -f state/halt.lock state/disarm.req state/reset.req
echo "da xoa state/halt.lock (neu co)"

# ---- config TAM: nguong 0, universal, USDT, sim_engine=evm ----
CFG="$(mktemp -t paperrun.XXXXXX.toml)"
trap 'rm -f "$CFG"' EXIT
# Lay config.toml that lam nen roi override cac field can thiet.
cp config.toml "$CFG"
# override bang sed (chi cac field can): nguong 0 + bat universal + USDT + evm.
apply() { # apply <key> <value>
  local k="$1" v="$2"
  if grep -qE "^$k[[:space:]]*=" "$CFG"; then
    sed -i -E "s#^($k[[:space:]]*=).*#\1 $v#" "$CFG"
  else
    printf '%s = %s\n' "$k" "$v" >> "$CFG"
  fi
}
apply min_profit_bnb 0
apply min_reserve_wbnb 0
apply max_roundtrip_tax 0
apply pairs_min_swap_bnb 0
apply min_profit_usdt 0
apply min_reserve_usdt 0
apply pair_scan_universal true
apply scan_quote_usdt true
apply sim_engine '"evm"'
apply web_port "$PORT"
echo "== config TAM (nguong 0 + universal + USDT + sim_engine=evm), port $PORT =="

# ---- chay bot NGAM voi config tam ----
# Binary nhan duong dan config lam THAM SO VI TRI THU 1 (src/main.rs:28), KHONG
# phai --config. Config tam co web_port rieng nen khong dung web_port that.
LOG="logs/paper_run_$(date +%s).log"
echo "== chay bot $MINUTES phut, log -> $LOG =="
./target/release/bsc_sandwich "$CFG" >"$LOG" 2>&1 &
echo $! > state/paper_run.pid
PID="$(cat state/paper_run.pid)"
echo "PID=$PID"

sleep $(( MINUTES * 60 ))

BASE="http://127.0.0.1:$PORT"
echo "======== KET QUA SAU $MINUTES PHUT (may: $RUN_ENV, binary sha256: $BIN_SHA, git HEAD: $GIT_HEAD) ========"
echo "---- 30 dong funnel.minute cuoi (logs/bot.jsonl) ----"
grep '"event":"funnel.minute"' logs/bot.jsonl 2>/dev/null | tail -30 || echo "(chua co funnel.minute)"
echo "---- /api/skips ----"; curl -s "$BASE/api/skips" || true; echo
echo "---- /api/funnel ----"; curl -s "$BASE/api/funnel" || true; echo
echo "---- /api/tax ----"; curl -s "$BASE/api/tax" || true; echo
echo "---- /api/validate ----"; curl -s "$BASE/api/validate" || true; echo
echo "---- 20 dong tx.skip cuoi (token+venue) ----"
grep '"event":"tx.skip"' logs/bot.jsonl 2>/dev/null | tail -20 || true
echo "---- 10 dong sim.evm cuoi ----"
grep '"event":"sim.evm"' logs/bot.jsonl 2>/dev/null | tail -10 || true
echo "---- dem Simulated (sim.evm decision=simulated) ----"
grep '"event":"sim.evm"' logs/bot.jsonl 2>/dev/null | grep -c '"decision":"simulated"' || echo 0

# ---- halt sach ----
echo "== halt bot (ghi state/halt.lock + kill PID) =="
: > state/halt.lock
kill "$PID" 2>/dev/null || true
echo "DONE. Log day du: $LOG (redact secret truoc khi dan cho Grok)."
echo "Nho dan lai: may=$RUN_ENV, binary sha256=$BIN_SHA, git HEAD=$GIT_HEAD (luat #2 CLAUDE.md)."
