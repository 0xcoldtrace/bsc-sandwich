#!/usr/bin/env bash
# Cụm `strategy-lock-mode2` — chạy PAPER 30 phút (mặc định) với cấu hình
# ngưỡng-0 (chỉ hạ min_profit/min_reserve/... về 0 để dễ quan sát), GIỮ
# NGUYÊN sim_engine/pair_scan_universal/scan_quote_usdt từ config.toml thật
# (mode 2 only, sim_engine="v2"), rồi in đủ số liệu funnel/skip/tax/validate.
# Script DÙNG CHUNG cho WSL (máy dev) và VPS (máy chạy thật) — không có bước
# SSH/deploy nào ở đây, chỉ chạy TẠI CHỖ trong thư mục repo đã có `.env`
# thật. Muốn đưa source lên VPS trước, dùng `scripts/deploy_vps.sh` (có SSH,
# script này thì KHÔNG — không có bản `.ps1`, chạy trên WSL/Linux/VPS).
#
# Script KHÔNG bật live/armed, KHÔNG gửi tx, KHÔNG in secret.
#
# Usage (trong thu muc repo da co .env that va da/se `cargo build --release`):
#   scripts/paper_run.sh [--minutes 30] [--port 8799]
#
# Yeu cau: .env co BSC_HTTP/BSC_WS that.
# Script tao 1 config TAM (khong dung config.toml that cua Chu) voi nguong 0.
#
# AGENTS.md luat "3 luat bo sung 2026-09-15" (#2): moi so lieu runtime phai
# ghi ro chay o dau (WSL/VPS) + hash binary — script nay tu in ca 2 thu do
# ngay dau output ket qua, dan nguyen vao BAOCAO.
set -euo pipefail

MINUTES=30
PORT=8799
# Cụm `competitor-recon-and-strategy` (mục 4) — `--live-mode shadow` override
# CHỈ field `live_mode` trong config TẠM (mặc định "off", giữ NGUYÊN hành vi
# mọi phiên trước cụm này) — dùng để verify shadow mode (ký thật, KHÔNG gửi)
# bằng `.env` đã có `PRIVATE_KEY` thật (ví Chủ tự điền). KHÔNG override field
# nào khác liên quan live (`allow_live`/`bot_armed`/`dry_run` GIỮ NGUYÊN từ
# `config.toml` thật, luôn `false`/`false`/`true` theo ship — script này vẫn
# KHÔNG có cách nào bật live).
LIVE_MODE="off"
# Cum `verify-cluster-as-victim` (muc 2) — `--allow-competitor-victims`
# override CHI field `allow_competitor_victims` trong config TAM. Can thiet vi
# o `live_mode="shadow"` ship-value `false` lam candidate cua CUM DOI THU bi
# skip `competitor_victim` TRUOC khi toi buoc ky/`shadow.sim`, tuc dung cai
# nhom ta dang can do. KHONG bat live/armed/dry_run gi ca.
ALLOW_COMPETITOR="keep"
while [ $# -gt 0 ]; do
  case "$1" in
    --minutes) MINUTES="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    --live-mode) LIVE_MODE="$2"; shift 2 ;;
    --allow-competitor-victims) ALLOW_COMPETITOR="true"; shift 1 ;;
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

# ---- cum `econ-truth-latency-vps` (muc 4, no nho) — rotate logs/bot.jsonl
# khi vuot 200MB, ho tro --minutes dai (vd 1440 = 24h) ma khong day dia. Giu
# TOI DA 5 ban cu (.1 .. .5), xoa ban cu nhat khi vuot. Rotate TRUOC khi bot
# moi ghi dong nao — khong lam mat du lieu dang ghi giua chung.
ROTATE_MAX_BYTES=$((200 * 1024 * 1024))
if [ -f logs/bot.jsonl ]; then
  CUR_SIZE=$(stat -c%s logs/bot.jsonl 2>/dev/null || stat -f%z logs/bot.jsonl 2>/dev/null || echo 0)
  if [ "$CUR_SIZE" -ge "$ROTATE_MAX_BYTES" ]; then
    echo "== logs/bot.jsonl >= 200MB ($CUR_SIZE bytes) - rotate truoc khi chay =="
    [ -f logs/bot.jsonl.5 ] && rm -f logs/bot.jsonl.5
    for i in 4 3 2 1; do
      [ -f "logs/bot.jsonl.$i" ] && mv "logs/bot.jsonl.$i" "logs/bot.jsonl.$((i + 1))"
    done
    mv logs/bot.jsonl logs/bot.jsonl.1
    touch logs/bot.jsonl
  fi
fi

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
if [ "$ALLOW_COMPETITOR" = "true" ]; then
  apply allow_competitor_victims true
  echo "== CANH BAO: allow_competitor_victims override thanh true (ship false) - CHI de DO candidate cua cum doi thu o shadow, KHONG gui tx =="
fi
if [ "$LIVE_MODE" != "off" ]; then
  apply live_mode "\"$LIVE_MODE\""
  echo "== CANH BAO: live_mode override thanh \"$LIVE_MODE\" (mac dinh \"off\") - dry_run/allow_live/bot_armed VAN giu nguyen tu config.toml that (khong doi) =="
fi
echo "== config TAM (chi nguong ve 0 + web_port + live_mode neu co --live-mode, GIU NGUYEN sim_engine/pair_scan_universal/scan_quote_usdt tu config.toml that), port $PORT =="

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
echo "---- /api/pairs ----"; curl -s "$BASE/api/pairs" || true; echo
ECON_JSON="$(curl -s "$BASE/api/econ" || true)"
echo "---- /api/econ (cum real-economics-mode2, muc 3) ----"; printf '%s\n' "$ECON_JSON"
echo "---- /api/validate ----"; curl -s "$BASE/api/validate" || true; echo
echo "---- 20 dong tx.skip cuoi (lan chay nay) ----"
RUN_LOG | grep '"event":"tx.skip"' | tail -20 || true
echo "---- 10 dong sim.evm cuoi (lan chay nay) ----"
RUN_LOG | grep '"event":"sim.evm"' | tail -10 || true
echo "---- dem Simulated (sim.evm decision=simulated, lan chay nay) ----"
count_matches_in '"event":"sim.evm"' '"decision":"simulated"'
if [ "$LIVE_MODE" = "shadow" ]; then
  echo "---- cum competitor-recon-and-strategy (muc 4): shadow.signer_loaded / shadow.signer_load_failed ----"
  RUN_LOG | grep -E '"event":"shadow\.(signer_loaded|signer_load_failed)"' || echo "(khong thay dong nao)"
  echo "---- dem bundle.shadow (da ky THAT, KHONG gui) ----"
  count_matches '"event":"bundle.shadow"'
  echo "---- dem tx.abort theo tung ly do (cum bugfix-presign-and-contract-plan, A4) ----"
  for R in vet_stale reserve_stale mined_index_cold victim_already_mined nonce_not_prefetched sign_failed_front sign_failed_back pre_sign_revet_failed; do
    printf '  %-24s %s\n' "$R" "$(count_matches_in '"event":"tx.abort"' "\"reason\":\"$R\"")"
  done
  echo "---- 5 dong bundle.shadow cuoi (rut gon: bo raw_hex) ----"
  RUN_LOG | grep '"event":"bundle.shadow"' | tail -5 | jq -c 'del(.front.raw_hex, .back.raw_hex)' 2>/dev/null || RUN_LOG | grep '"event":"bundle.shadow"' | tail -2
  echo "---- presign.ms p50/p95 (A4, muc tieu p95 < 20 ms) ----"
  RUN_LOG | grep -E '"event":"(bundle\.shadow|tx\.abort)"' | jq -r '.presign_ms.total_before_sign // empty' 2>/dev/null \
    | sort -n | awk '{v[NR]=$1} END{
        if (NR==0) { print "(khong co mau presign_ms)" ; exit }
        i50=int((NR+1)/2); if(i50<1)i50=1
        i95=int(NR*0.95); if(i95<1)i95=1
        printf "n=%d p50=%.3f ms p95=%.3f ms max=%.3f ms\n", NR, v[i50], v[i95], v[NR]
      }'
  echo "---- bundle.shadow_econ (A7: bribe / net sau bribe / co doi thu) ----"
  RUN_LOG | grep '"event":"bundle.shadow_econ"' | tail -10 || true
  echo "---- shadow.sim (A7: mo phong bundle 3 chan bang revm NEN) ----"
  RUN_LOG | grep '"event":"shadow.sim"' | tail -10 || true
  echo "---- dem shadow.sim victim_ok=true / co loi ----"
  count_matches_in '"event":"shadow.sim"' '"victim_ok":true'
  count_matches_in '"event":"shadow.sim"' '"error"'
  # Cum `decision-data-24h` (muc 4) - doi chieu profit_sim (revm 3 chan) voi
  # profit_net (duong nong V2-math) cho TUNG bundle da ky. Truoc cum nay nhanh
  # quote USDT khong co profit_sim nao (9/9 bundle bi skip), nen bang nay rong.
  # Dung `jq` (co san WSL + VPS); khong co jq thi in dong tho, KHONG bia so.
  echo "---- doi chieu profit_sim (revm) vs profit_net (V2-math) theo tung victim ----"
  if command -v jq >/dev/null 2>&1; then
    RUN_LOG | grep '"event":"shadow.sim"' | jq -r '
      "  " + (.victim_hash[0:18]) + " quote=" + (.quote // "?") +
      (if .error then "  LOI: " + (.error[0:90])
       elif .skipped then "  SKIP: " + .skipped
       else "  profit_sim=" + ((.profit_sim_native // 0) | tostring) +
            "  victim_ok=" + ((.victim_ok // false) | tostring) +
            "  tax_buy/sell=" + ((.buy_tax_bps // "-") | tostring) + "/" + ((.sell_tax_bps // "-") | tostring)
       end)' 2>/dev/null || echo "  (khong co dong shadow.sim)"
    echo "  profit_net tuong ung (bundle.shadow_econ):"
    RUN_LOG | grep '"event":"bundle.shadow_econ"' | jq -r '
      "  " + (.victim_hash[0:18]) + "  profit_net=" + ((.profit_net_native // .profit_net_bnb // .profit_net_wei // "-") | tostring) +
      "  bribe=" + ((.bribe_native // .bribe_bnb // "-") | tostring) +
      "  unit=" + ((.bribe_unit // .quote // "?") | tostring)' 2>/dev/null || echo "  (khong co dong bundle.shadow_econ)"
  else
    echo "  (khong co jq - in dong tho)"
    RUN_LOG | grep '"event":"shadow.sim"' | tail -10 || true
  fi
  echo "---- latency.decision_vs_mined 10 dong cuoi ----"
  RUN_LOG | grep '"event":"latency.decision_vs_mined"' | tail -10 || true
fi
echo "---- cum A5: rpc.bg_pool (tach RPC nen khoi duong nong) ----"
RUN_LOG | grep '"event":"rpc.bg_pool"' | tail -2 || echo "(khong thay)"
echo "---- cum A3: competitor.subscribed / competitor.funded (dem) ----"
RUN_LOG | grep '"event":"competitor.subscribed"' | tail -2 || echo "(khong thay)"
echo "competitor.funded = $(count_matches '"event":"competitor.funded"')"
echo "victim_in_competitor_cluster=true = $(count_matches '"victim_in_competitor_cluster":true')"
echo "---- cum A2: dem skip sanity_reject / competitor_victim ----"
echo "sanity_reject = $(count_matches_in '"event":"tx.skip"' '"reason":"sanity_reject"')"
echo "competitor_victim = $(count_matches_in '"event":"tx.skip"' '"reason":"competitor_victim"')"
echo "---- cum 'decision-data-24h' muc 3: pair.vet_restore (nap lai snapshot vet luc boot) ----"
RUN_LOG | grep '"event":"pair.vet_restore"' | tail -2 || echo "(khong thay - chua co state/pairs_vetted.json)"
echo "---- cum 'decision-data-24h' muc 5: pair.vet_cycle_done (loi vet theo LOAI + so pool chua vet duoc) ----"
RUN_LOG | grep '"event":"pair.vet_cycle_done"' | tail -3 || echo "(khong thay)"
echo "vet_error missing_trie_node = $(count_matches_in '"event":"pair.vet_error"' '"class":"missing_trie_node"')"
echo "vet_error rate_limited      = $(count_matches_in '"event":"pair.vet_error"' '"class":"rate_limited"')"
echo "vet_error unsupported_method= $(count_matches_in '"event":"pair.vet_error"' '"class":"unsupported_method"')"
echo "vet_error other             = $(count_matches_in '"event":"pair.vet_error"' '"class":"other"')"
echo "---- cum A4: mined_index.ready / pair.vet_cycle ----"
RUN_LOG | grep -E '"event":"(mined_index.ready|pair.vet_cycle)"' | tail -5 || echo "(khong thay)"

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
echo "---- /api/econ summary_line (cum real-economics-mode2, muc 3.e, doc tu ECON_JSON lay TRUOC khi kill) ----"
printf '%s' "$ECON_JSON" | jq -r '.summary_line // "MISSING"' 2>/dev/null || echo "MISSING"

echo "DONE. Log day du: $LOG (redact secret truoc khi dan cho Grok)."
echo "Nho dan lai: may=$RUN_ENV, binary sha256=$BIN_SHA, git HEAD=$GIT_HEAD (luat #2 AGENTS.md)."
