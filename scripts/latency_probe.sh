#!/usr/bin/env bash
# Cụm `vps-latencyprobe-retest` (B) — đo round-trip TỪNG CHẶNG bằng lệnh
# đọc/vô hại (eth_blockNumber / eth_chainId), KHÔNG ký/gửi tx thật, KHÔNG
# sửa relay.rs/pipeline.rs/executor.rs (đo bằng script ngoài, không đụng
# code sản phẩm). Dùng curl thuần + awk/date, KHÔNG Python (đúng AGENTS.md
# "Cấm ... Python runtime" cho mọi phần thuộc repo bot, kể cả tooling).
#
# Chạy trên máy có `.env` của bot (thường là VPS đang host bot, vì đây là
# nơi "bot" thực sự gọi RPC/relay — round-trip đo từ dev machine không phản
# ánh đúng latency thật của bot).
#
# Usage: scripts/latency_probe.sh
set -uo pipefail
cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

TIMEOUT=5
SAMPLES=3

redact() {
  echo "$1" | sed -E 's#^(https?|wss?)://([^/]+).*#\1://\2#'
}

# In: url, json payload. Out (stdout): "OK <ms>" hoặc "ERR <ms> <curl_exit>"
rpc_call_ms() {
  local url="$1" payload="$2"
  local t0 t1 ms out rc
  t0=$(date +%s%N)
  out=$(curl -s -m "$TIMEOUT" -o /tmp/latprobe_body.$$ -w "%{http_code}" -X POST \
    -H "Content-Type: application/json" -d "$payload" "$url" 2>/tmp/latprobe_err.$$)
  rc=$?
  t1=$(date +%s%N)
  ms=$(( (t1 - t0) / 1000000 ))
  if [ $rc -ne 0 ]; then
    echo "ERR $ms curl_exit_$rc"
  else
    body=$(cat /tmp/latprobe_body.$$ 2>/dev/null)
    if echo "$body" | grep -q '"result"'; then
      echo "OK $ms"
    else
      short=$(echo "$body" | tr -d '\n' | cut -c1-80)
      echo "ERR $ms http_${out}_body_${short}"
    fi
  fi
  rm -f /tmp/latprobe_body.$$ /tmp/latprobe_err.$$
}

probe_endpoint() {
  local url="$1" label="$2" payload="$3"
  local sum=0 min=999999 max=0 ok=0 lasterr="-"
  for i in $(seq 1 $SAMPLES); do
    res=$(rpc_call_ms "$url" "$payload")
    status=$(echo "$res" | awk '{print $1}')
    ms=$(echo "$res" | awk '{print $2}')
    if [ "$status" = "OK" ]; then
      ok=$((ok+1))
      sum=$((sum+ms))
      [ "$ms" -lt "$min" ] && min=$ms
      [ "$ms" -gt "$max" ] && max=$ms
    else
      lasterr=$(echo "$res" | cut -d' ' -f3-)
    fi
  done
  local avg=0
  [ $ok -gt 0 ] && avg=$((sum/ok))
  [ $ok -eq 0 ] && min=0
  printf "%-8s %-45s ok=%d/%d avg=%dms min=%dms max=%dms err=%s\n" \
    "$label" "$(redact "$url")" "$ok" "$SAMPLES" "$avg" "$min" "$max" "$lasterr"
}

echo "=== (B) latency probe — $(date -u +%FT%TZ) ==="
echo "--- BSC_HTTP list (eth_blockNumber x${SAMPLES}) ---"
IFS=',' read -ra HTTP_URLS <<< "${BSC_HTTP:-}"
n=0
for u in "${HTTP_URLS[@]}"; do
  u_trim=$(echo "$u" | xargs)
  [ -z "$u_trim" ] && continue
  n=$((n+1))
  probe_endpoint "$u_trim" "http" '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
done
echo "(tổng $n URL trong BSC_HTTP)"

echo ""
echo "--- Relay pinned (src/relay.rs) — eth_chainId x${SAMPLES}, vô hại ---"
probe_endpoint "https://puissant-builder.48.club/" "relay" '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
probe_endpoint "https://bsc.blockrazor.xyz" "relay" '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'

echo ""
echo "=== hết (B) hop-latency — GHI RÕ: round-trip TỪNG CHẶNG riêng lẻ bằng lệnh đọc/vô hại, KHÔNG phải end-to-end 1 tx thật (dry_run=true xuyên suốt) ==="
