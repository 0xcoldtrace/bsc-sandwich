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
# `set -a` de MOI bien duoc source tu .env tu dong EXPORT ra tien trinh con
# (`./target/release/bsc_sandwich ... &` ben duoi) - thieu buoc nay, .env chi
# dinh nghia BIEN SHELL cuc bo (vi .env khong co tu khoa `export` truoc moi
# dong), bot con chay khong thay BSC_HTTP/BSC_WS gi ca, roi VOI xuong
# vps.json (con placeholder "REPLACE_ME_..." chua thay) -> loi "relative URL
# without a base", khong ket noi RPC nao - da quan sat that phien nay truoc
# khi sua (loi nay co tu truoc, khong phai do cum lenh nay gay ra).
if [ -f .env ]; then
  # shellcheck disable=SC1091
  set +u -a; . ./.env 2>/dev/null || true; set -u +a
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

# ---- config TAM: CHI nguong ve 0 + doi port ----
# Cum `strategy-lock-mode2` (Chu chot 2026-09-15): KHONG con ep
# pair_scan_universal=true / scan_quote_usdt=true / sim_engine="evm" nua -
# 3 field do GIU NGUYEN gia tri ship trong config.toml that (mode 2 only,
# sim_engine="v2") de paper run phan anh DUNG hanh vi san pham that dang
# chay, khong phai 1 che do do rieng khac voi production. Chi override NGUONG
# KINH TE ve 0 (khong loc mat candidate nao o tang do) + doi web_port (khong
# dung chung cong voi bot that dang chay).
CFG="$(mktemp -t paperrun.XXXXXX.toml)"
trap 'rm -f "$CFG"' EXIT
# Lay config.toml that lam nen roi override cac field can thiet.
cp config.toml "$CFG"
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
apply web_port "$PORT"
echo "== config TAM (chi nguong ve 0 + web_port, GIU NGUYEN sim_engine/pair_scan_universal/scan_quote_usdt tu config.toml that), port $PORT =="

# ---- cum `strategy-lock-mode2`: dem nhanh pairs.txt truoc khi chay (tong /
# co "vetted YYYY-MM-DD" hop le / con lai chua vet) - grep tho, KHONG phai
# nguon su that (nguon su that la PairBook::reload luc bot chay that, dong
# nay chi de Chu/Grok nhin nhanh khong can doi bot boot) ----
PAIRS_TOTAL=$( { grep -cE '^0x' pairs.txt 2>/dev/null || true; } )
PAIRS_TOTAL="${PAIRS_TOTAL:-0}"
PAIRS_VETTED=$( { grep -E '^0x' pairs.txt 2>/dev/null | { grep -cE '\|[[:space:]]*vetted[[:space:]]+[0-9]{4}-[0-9]{2}-[0-9]{2}' || true; }; } )
PAIRS_VETTED="${PAIRS_VETTED:-0}"
echo "== pairs.txt (grep tho, xem sim.evm/pair.unvetted trong log de co so that): tong=$PAIRS_TOTAL vetted=$PAIRS_VETTED chua_vet=$((PAIRS_TOTAL - PAIRS_VETTED)) =="

# ---- chay bot NGAM voi config tam ----
# Binary nhan duong dan config lam THAM SO VI TRI THU 1 (src/main.rs:28), KHONG
# phai --config. Config tam co web_port rieng nen khong dung web_port that.
LOG="logs/paper_run_$(date +%s).log"

# ---- cum exec-path-traps (muc 13a): chi dem log CUA LAN CHAY NAY, khong
# lan voi log tich luy tu cac lan chay truoc trong CUNG file logs/bot.jsonl
# (file dung chung, khong bi xoa giua cac lan chay). Ghi lai dong cuoi cung
# TRUOC khi bot moi khoi dong, RUN_LOG() chi doc TU dong ke tiep tro di.
mkdir -p logs
touch logs/bot.jsonl
START_LINE=$(( $(wc -l < logs/bot.jsonl 2>/dev/null || echo 0) + 1 ))
RUN_LOG() { tail -n +"$START_LINE" logs/bot.jsonl 2>/dev/null; }

# `grep -c PATTERN` LUON in ra so dem (ke ca "0") NHUNG van thoat ma 1 neu
# khong co dong nao khop - "X=$(grep -c ... || echo 0)" vi vay bi GHI DOI 2
# LAN ("0" that cua grep -c CONG THEM "0" cua || echo 0) khi dem ra 0. Dung 2
# ham nay o MOI cho can dem dong bang grep -c de tranh loi do (chi dung
# `|| true` giu nguyen dung 1 gia tri grep -c da in).
count_matches() { # count_matches <pattern>
  RUN_LOG | { grep -c "$1" || true; }
}
count_matches_in() { # count_matches_in <pattern_loc> <pattern_dem>
  # QUAN TRONG: MOI stage trong pipe phai tu "|| true" rieng — voi
  # `set -o pipefail`, exit code cua CA PIPE la exit code cua stage THAT BAI
  # CUOI CUNG TINH TU PHAI SANG (khong phai chi stage cuoi cung): neu chi
  # bao ve stage `grep -c` cuoi (`|| true`) ma stage `grep "$1"` GIUA bi 0
  # dong khop (exit 1, dieu BINH THUONG khi dem ra 0), pipefail VAN bao loi
  # ca pipe (vi stage giua la "rightmost FAILING stage"), lam `set -e` giet
  # ca script — da tu tai hien bug nay that (script chet dung ngay o day
  # truoc khi sua, khong thay dong "== halt bot ..." nao ca).
  RUN_LOG | { grep "$1" || true; } | { grep -c "$2" || true; }
}

echo "== chay bot $MINUTES phut, log -> $LOG (bot.jsonl tu dong $START_LINE) =="
./target/release/bsc_sandwich "$CFG" >"$LOG" 2>&1 &
echo $! > state/paper_run.pid
PID="$(cat state/paper_run.pid)"
echo "PID=$PID"

# ---- cum exec-path-traps (muc 13c): phat hien bot chet giua chung thay vi
# ngu mu roi bao ket qua rong nhu thanh cong ----
ELAPSED_MIN=0
while [ "$ELAPSED_MIN" -lt "$MINUTES" ]; do
  sleep 60
  ELAPSED_MIN=$((ELAPSED_MIN + 1))
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "BOT DA CHET sau $ELAPSED_MIN phut"
    echo "---- tail -30 $LOG ----"
    tail -30 "$LOG" || true
    exit 1
  fi
done

BASE="http://127.0.0.1:$PORT"
echo "======== KET QUA SAU $MINUTES PHUT (may: $RUN_ENV, binary sha256: $BIN_SHA, git HEAD: $GIT_HEAD) ========"
echo "---- 30 dong funnel.minute cuoi (lan chay nay) ----"
RUN_LOG | grep '"event":"funnel.minute"' | tail -30 || echo "(chua co funnel.minute)"
echo "---- /api/skips ----"; curl -s "$BASE/api/skips" || true; echo
echo "---- /api/funnel ----"; curl -s "$BASE/api/funnel" || true; echo
echo "---- /api/tax ----"; curl -s "$BASE/api/tax" || true; echo
echo "---- /api/validate ----"; curl -s "$BASE/api/validate" || true; echo
echo "---- 20 dong tx.skip cuoi (lan chay nay) ----"
RUN_LOG | grep '"event":"tx.skip"' | tail -20 || true
echo "---- 10 dong sim.evm cuoi (lan chay nay) ----"
RUN_LOG | grep '"event":"sim.evm"' | tail -10 || true
echo "---- dem Simulated (sim.evm decision=simulated, lan chay nay) ----"
count_matches_in '"event":"sim.evm"' '"decision":"simulated"'

# ---- halt sach: cho halt.triggered THAT truoc khi kill (muc 13b) ----
echo "== halt bot (ghi state/halt.lock, cho halt.triggered toi da 10s, roi kill PID) =="
: > state/halt.lock
HALT_WAITED=0
while [ "$HALT_WAITED" -lt 20 ]; do
  if RUN_LOG | grep -q '"event":"halt.triggered"'; then
    break
  fi
  sleep 0.5
  HALT_WAITED=$((HALT_WAITED + 1))
done
HALT_TRIGGERED_COUNT="$(count_matches '"event":"halt.triggered"')"
echo "halt.triggered count (lan chay nay) = $HALT_TRIGGERED_COUNT"
HALT_LINE_NO="$(RUN_LOG | grep -n '"event":"halt.triggered"' | head -1 | cut -d: -f1 || true)"
if [ -n "${HALT_LINE_NO:-}" ]; then
  TX_SEEN_AFTER_HALT="$(RUN_LOG | tail -n +"$((HALT_LINE_NO + 1))" | { grep -c '"event":"tx.seen"' || true; })"
else
  TX_SEEN_AFTER_HALT="N/A (chua thay halt.triggered trong ${HALT_WAITED}00ms cho)"
fi
echo "tx.seen sau halt.triggered (lan chay nay) = $TX_SEEN_AFTER_HALT (ky vong 0)"
kill "$PID" 2>/dev/null || true

# ---- dong tong ket DoD (muc 13d) ----
TX_BUILD_COUNT="$(count_matches '"event":"tx.build"')"
SIMULATED_COUNT="$(count_matches_in '"event":"sim.evm"' '"decision":"simulated"')"
BUILD_REFUSED_COUNT="$(count_matches '"event":"build.refused"')"
echo "tx.build=$TX_BUILD_COUNT simulated=$SIMULATED_COUNT build.refused=$BUILD_REFUSED_COUNT halt.triggered=$HALT_TRIGGERED_COUNT"

echo "DONE. Log day du: $LOG (redact secret truoc khi dan cho Grok)."
echo "Nho dan lai: may=$RUN_ENV, binary sha256=$BIN_SHA, git HEAD=$GIT_HEAD (luat #2 CLAUDE.md)."
