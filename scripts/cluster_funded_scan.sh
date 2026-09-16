#!/usr/bin/env bash
# scripts/cluster_funded_scan.sh — cụm `decision-data-24h` (mục 1a)
#
# Dựng DANH SÁCH VÍ THUỘC CỤM ĐỐI THỦ cho một khoảng block, bằng `eth_getLogs`
# THẬT (chỉ ĐỌC, không ký, không gửi tx). Dùng cho log CŨ do binary trước cụm
# `bugfix-presign-and-contract-plan` ghi ra — log đó KHÔNG có cờ
# `victim_in_competitor_cluster`, nên phải dựng lại ngoại tuyến.
#
# Cơ chế bám đúng `src/competitor.rs`: ví nào nhận Transfer quote-asset
# (WBNB hoặc USDT) TỪ một trong 3 địa chỉ seed đã verify on-chain (BAOCAO41)
# thì thuộc cụm. Output TSV `wallet<TAB>block` (mỗi lần cấp vốn 1 dòng) để
# `analyze_econ.sh` vừa kiểm được kiểu "từng được cấp vốn", vừa kiểm được
# cửa sổ block ±N chặt như bot làm lúc chạy thật.
#
# Dùng: scripts/cluster_funded_scan.sh --from-block N --to-block M --out FILE
#       [--chunk 2000] [--rpc URL]
set -euo pipefail

SEEDS_TOPIC='["0x000000000000000000000000b406021e07b31e1f7850fcccd7076094f18d07ef","0x000000000000000000000000a739dfab40ef6585f1174fce90ec96330669758c","0x0000000000000000000000008180ad6a7c9f8f4864e9909480fba4123fce6c54"]'
TRANSFER_TOPIC='0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef'
# WBNB + USDT đã pin trong DEX_REGISTRY.md.
ADDRS='["0x55d398326f99059fF775485246999027B3197955","0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"]'

FROM=""; TO=""; OUT=""; CHUNK=2000; RPC=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-block) FROM="$2"; shift 2;;
    --to-block) TO="$2"; shift 2;;
    --out) OUT="$2"; shift 2;;
    --chunk) CHUNK="$2"; shift 2;;
    --rpc) RPC="$2"; shift 2;;
    *) echo "tham so la: $1" >&2; exit 2;;
  esac
done
[[ -n "$FROM" && -n "$TO" && -n "$OUT" ]] || { echo "thieu --from-block/--to-block/--out" >&2; exit 2; }

if [[ -z "$RPC" ]]; then
  # Lấy URL đầu tiên của BSC_HTTP trong .env (KHÔNG in URL ra ngoài).
  [[ -f .env ]] || { echo "khong co .env va khong co --rpc" >&2; exit 2; }
  RPC=$(awk -F= '/^BSC_HTTP=/{sub(/^BSC_HTTP=/,""); print}' .env | tr -d '"' | cut -d, -f1)
fi
[[ -n "$RPC" ]] || { echo "khong tim duoc RPC URL" >&2; exit 2; }
echo "rpc host = $(echo "$RPC" | sed 's|https\?://\([^/]*\).*|\1|')  (KHONG in URL day du)" >&2

CHAIN=$(curl -s -m 20 -X POST "$RPC" -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' | jq -r '.result')
[[ "$CHAIN" == "0x38" ]] || { echo "chain_id != 0x38 (duoc: $CHAIN) - dung" >&2; exit 1; }
echo "eth_chainId = $CHAIN" >&2

: > "$OUT"
b=$FROM
calls=0; errors=0
while [[ $b -le $TO ]]; do
  e=$(( b + CHUNK - 1 )); [[ $e -gt $TO ]] && e=$TO
  fh=$(printf '0x%x' "$b"); th=$(printf '0x%x' "$e")
  body=$(printf '{"jsonrpc":"2.0","id":1,"method":"eth_getLogs","params":[{"fromBlock":"%s","toBlock":"%s","address":%s,"topics":["%s",%s]}]}' \
    "$fh" "$th" "$ADDRS" "$TRANSFER_TOPIC" "$SEEDS_TOPIC")
  resp=$(curl -s -m 60 -X POST "$RPC" -H 'content-type: application/json' -d "$body" || echo '{}')
  if ! echo "$resp" | jq -e '.result' >/dev/null 2>&1; then
    errors=$(( errors + 1 ))
    echo "LOI getLogs [$b..$e]: $(echo "$resp" | jq -c '.error' 2>/dev/null | head -c 160)" >&2
  else
    echo "$resp" | jq -r '.result[] | [ (.topics[2] | "0x" + .[26:]), (.blockNumber | ltrimstr("0x") | "0x"+. ) ] | @tsv' \
      | awk -F'\t' '{ printf "%s\t%d\n", tolower($1), strtonum($2) }' >> "$OUT"
  fi
  calls=$(( calls + 1 ))
  b=$(( e + 1 ))
done
n=$(wc -l < "$OUT"); u=$(cut -f1 "$OUT" | sort -u | wc -l)
echo "xong: $calls call, $errors loi, $n lan cap von, $u vi rieng biet -> $OUT" >&2
