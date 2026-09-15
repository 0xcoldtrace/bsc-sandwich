#!/usr/bin/env bash
# Cụm `vps-latencyprobe-retest` (B) — "validator/block" latency = lệch giữa
# giờ hệ thống lúc bot NHẬN `rpc.block` (đã có trong logs/bot.jsonl, field
# `ts`) và `timestamp` thật trong block header on-chain (`eth_getBlockByNumber`
# read-only, KHÔNG ký/gửi gì). KHÔNG phải latency end-to-end của 1 tx thật.
#
# Usage: scripts/block_latency.sh <bot.jsonl> <since_iso8601_utc> <until_iso8601_utc>
# Cần biến môi trường BSC_HTTP (source .env trước, hoặc script tự source
# .env ở thư mục gốc repo nếu có).
set -uo pipefail
cd "$(dirname "$0")/.."

LOGFILE="${1:?usage: block_latency.sh <bot.jsonl> <since_iso> <until_iso>}"
SINCE="${2:?usage: block_latency.sh <bot.jsonl> <since_iso> <until_iso>}"
UNTIL="${3:?usage: block_latency.sh <bot.jsonl> <since_iso> <until_iso>}"

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

PRIMARY_HTTP=$(echo "${BSC_HTTP:-}" | cut -d',' -f1 | xargs)
if [ -z "$PRIMARY_HTTP" ]; then
  echo "MISSING: khong co BSC_HTTP trong env de goi eth_getBlockByNumber"
  exit 1
fi

SINCE_EPOCH=$(date -u -d "$SINCE" +%s)
UNTIL_EPOCH=$(date -u -d "$UNTIL" +%s)

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

# Cột: block  ts_iso8601 (lấy từ dòng ĐẦU TIÊN thấy block đó). 1 pass awk
# (không fork grep/sed/date per-line — log có thể có hàng chục nghìn dòng
# rpc.block tích luỹ từ trước, per-line subshell fork từng bị đo CHẬM tới
# mức timeout >120s, xem BAOCAO28). Lọc theo prefix chuỗi ISO8601 UTC của
# `since`/`until` (an toàn vì `ts` luôn dạng `YYYY-MM-DDTHH:MM:SS...+00:00`,
# so sánh chuỗi = so sánh thời gian trong cùng ngày UTC).
SINCE_PREFIX=$(date -u -d "$SINCE" +%Y-%m-%dT%H:%M:%S)
UNTIL_PREFIX=$(date -u -d "$UNTIL" +%Y-%m-%dT%H:%M:%S)
awk -F'"' -v since="$SINCE_PREFIX" -v until="$UNTIL_PREFIX" '
  /"event":"rpc.block"/ {
    block=""; ts="";
    for (i=1; i<=NF; i++) {
      if ($i == "block") { v=$(i+1); gsub(/^:/, "", v); gsub(/[,}]/, "", v); block=v; }
      if ($i == "ts") { ts=$(i+2); }
    }
    if (block == "" || ts == "") next;
    tscmp = substr(ts, 1, 19);
    if (tscmp < since || tscmp > until) next;
    if (!(block in seen)) { seen[block]=1; print block, ts; }
  }
' "$LOGFILE" > "$WORKDIR/blocks_raw.txt"

# ts (nanosecond, co dau +00:00) -> epoch_ms qua `date -d` (van can fork,
# nhung chi cho ~so block trong 5 phut, khong phai toan bo log).
: > "$WORKDIR/blocks_in_window.txt"
while read -r block ts; do
  epoch_ms=$(date -u -d "$ts" +%s%3N 2>/dev/null) || continue
  echo "$block $epoch_ms" >> "$WORKDIR/blocks_in_window.txt"
done < "$WORKDIR/blocks_raw.txt"

TOTAL=$(wc -l < "$WORKDIR/blocks_in_window.txt" | xargs)
echo "=== (B) block/validator latency — cua so [$SINCE, $UNTIL) ==="
echo "block rpc.block distinct trong cua so: $TOTAL"

if [ "$TOTAL" -eq 0 ]; then
  echo "MISSING: khong co block nao trong cua so nay trong $LOGFILE"
  exit 0
fi

probe_one() {
  local block="$1" recv_ms="$2"
  local hexblock
  hexblock=$(printf '0x%x' "$block")
  local body
  body=$(curl -s -m 5 -X POST -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getBlockByNumber\",\"params\":[\"$hexblock\",false]}" \
    "$PRIMARY_HTTP" 2>/dev/null)
  local ts_hex
  ts_hex=$(echo "$body" | grep -oE '"timestamp":"0x[0-9a-fA-F]+"' | head -1 | grep -oE '0x[0-9a-fA-F]+')
  if [ -z "$ts_hex" ]; then
    echo "$block MISS"
    return
  fi
  local block_ts_s=$((ts_hex))
  local block_ts_ms=$((block_ts_s * 1000))
  local delta_ms=$((recv_ms - block_ts_ms))
  local abs_delta_ms=$delta_ms
  [ "$abs_delta_ms" -lt 0 ] && abs_delta_ms=$((-abs_delta_ms))
  echo "$block OK $abs_delta_ms"
}
export -f probe_one
export PRIMARY_HTTP

xargs -a "$WORKDIR/blocks_in_window.txt" -P 16 -L 1 bash -c 'probe_one "$0" "$1"' > "$WORKDIR/results.txt"

OK_COUNT=$(awk '$2=="OK"' "$WORKDIR/results.txt" | wc -l | xargs)
MISS_COUNT=$(awk '$2=="MISS"' "$WORKDIR/results.txt" | wc -l | xargs)
echo "eth_getBlockByNumber OK: $OK_COUNT / MISS: $MISS_COUNT (tong $TOTAL)"

awk '$2=="OK" {sum+=$3; n++; if(min==""||$3<min) min=$3; if(max==""||$3>max) max=$3} END {
  if (n>0) printf "avg_delta_ms=%.1f min_delta_ms=%d max_delta_ms=%d n=%d\n", sum/n, min, max, n;
  else print "n=0 (khong co ket qua OK nao)";
}' "$WORKDIR/results.txt"

echo "--- 10 dong mau (block, status, abs_delta_ms) ---"
head -10 "$WORKDIR/results.txt"
echo "=== hết (B) block-latency — |giờ hệ thống lúc nhận rpc.block - block.header.timestamp on-chain|, KHÔNG phải end-to-end tx thật ==="
