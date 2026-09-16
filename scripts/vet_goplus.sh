#!/usr/bin/env bash
# Cụm `strategy-lock-mode2` (2026-09-15) — bước LỌC THÔ cho quy trình vet tay
# `pairs.txt` (mode 2, pair-mode). Gọi GoPlus Security `token_security` API
# công khai (chain 56, KHÔNG cần API key) cho MỌI địa chỉ token trong
# `pairs.txt`, phân loại PASS/REVIEW/FAIL theo đúng tiêu chí AGENTS.md mục
# "Chiến lược đã chốt" (verified, không tax, không honeypot, không blacklist,
# không cooldown/anti-MEV, không pausable, không rebase).
#
# ĐÂY LÀ BƯỚC LỌC THÔ — KHÔNG phải quyết định cuối cùng. Chủ vẫn phải tự soát
# bằng mắt (đọc contract, kiểm BscScan, thử swap nhỏ...) trước khi điền
# `vetted YYYY-MM-DD` vào `pairs.txt`. Script KHÔNG tự sửa `pairs.txt`, KHÔNG
# gửi tx, KHÔNG cần `.env`/khoá riêng.
#
# Usage:
#   scripts/vet_goplus.sh [pairs.txt]
#
# Yêu cầu: curl, jq. Không có 2 lệnh này -> báo lỗi rõ, không âm thầm bỏ qua.
set -euo pipefail

FILE="${1:-pairs.txt}"

for bin in curl jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "LOI: can lenh '$bin' (chua cai) - cai qua 'apt-get install -y $bin' hoac tuong duong." >&2
    exit 2
  fi
done

if [ ! -f "$FILE" ]; then
  echo "LOI: khong tim thay file '$FILE'" >&2
  exit 2
fi

# ---- rut dia chi TOKEN tu moi dong du lieu (dong "0xAddr ..." hoac
# "0xAddr,0xWBNB ..."), bo qua comment/blank. Dinh dang comment sau '#'
# (vetted/tax/owner/note) KHONG anh huong buoc nay - chi can cot dia chi. ----
mapfile -t TOKENS < <(
  grep -E '^0x' "$FILE" \
    | sed -E 's/^([^,#[:space:]]+).*$/\1/' \
    | tr 'A-F' 'a-f'
)
TOTAL=${#TOKENS[@]}
if [ "$TOTAL" -eq 0 ]; then
  echo "LOI: khong tim thay dong 0x... nao trong '$FILE'" >&2
  exit 2
fi
echo "== vet_goplus.sh: $TOTAL token tu '$FILE', goi GoPlus token_security chain 56 =="

# ---- 1 request/token, retry-with-backoff tren rate-limit (PHAT HIEN THAT
# phien nay, 2 loi hạ tầng thật của GoPlus, verify bằng probe thủ công):
# (1) endpoint token_security CHI tra ve DUNG 1 ket qua du truyen nhieu
#     `contract_addresses` phay-cach trong URL - .result luon chi co DUNG 1
#     khoa [dia chi DAU TIEN], cac dia chi sau bi AM THAM BO QUA (khong loi,
#     khong canh bao tu API) -> KHONG dung batching nhu tai lieu goi y, goi
#     TUAN TU tung token.
# (2) rate-limit theo BURST rat ngan: ~7-8 request lien tiep trong <5s da bi
#     tra ve HTTP 200 nhung body {"code":4029,...} (RONG, khong phai loi
#     mang) - PHAI kiem tra field `.code` trong JSON body, KHONG chi HTTP
#     status. Da verify: cho ~10s la `.code` tro lai 1 (OK). Vi vay: sleep co
#     ban 1.2s/token (an toan hon nhieu so voi 400ms ban dau) + retry-backoff
#     (5s/10s/20s, toi da 3 lan) khi gap code!=1 - KHONG bao gio coi
#     code!=1/rate-limit la "khong co co do" roi tinh PASS am tham (se an FAIL
#     that duoi lop du lieu rong).
TMP_JSON="$(mktemp -t vetgoplus.XXXXXX.json)"
ROWS="$(mktemp -t vetgoplus.XXXXXX.rows)"
trap 'rm -f "$TMP_JSON" "$ROWS"' EXIT
: > "$ROWS"

fetch_token() { # fetch_token <addr> -> ghi $TMP_JSON, return 0 = co du lieu that (code=1)
  local addr="$1"
  local attempt backoff=5 gp_code
  for attempt in 1 2 3; do
    curl -sS -o "$TMP_JSON" \
      "https://api.gopluslabs.io/api/v1/token_security/56?contract_addresses=${addr}" || return 1
    gp_code=$(jq -r '.code // "?"' "$TMP_JSON" 2>/dev/null || echo "?")
    if [ "$gp_code" = "1" ]; then
      return 0
    fi
    echo "  $addr: GoPlus .code=$gp_code (lan $attempt/3, cho ${backoff}s roi thu lai)" >&2
    sleep "$backoff"
    backoff=$((backoff * 2))
  done
  return 1
}

n=0
for addr in "${TOKENS[@]}"; do
  n=$((n + 1))
  if ! fetch_token "$addr"; then
    echo "  [$n/$TOTAL] $addr: khong lay duoc du lieu THAT sau 3 lan thu (rate-limit/loi mang) - ghi ERROR, KHONG doan PASS" >&2
    echo -e "${addr}\tERROR\trate_limit_or_network" >> "$ROWS"
    sleep 1.2
    continue
  fi
  jq -r --arg a "$addr" '
    .result as $r |
    ($r[$a] // {}) as $t |
    [
      $a,
      ($t.token_symbol // "?"),
      ($t.is_honeypot // "?"),
      ($t.cannot_sell_all // "?"),
      ($t.buy_tax // "?"),
      ($t.sell_tax // "?"),
      ($t.is_blacklisted // "?"),
      ($t.trading_cooldown // "?"),
      ($t.transfer_pausable // "?"),
      ($t.is_anti_whale // "?"),
      ($t.is_proxy // "?"),
      ($t.hidden_owner // "?"),
      ($t.can_take_back_ownership // "?"),
      ($t.is_mintable // "?"),
      ($t.is_open_source // "?"),
      ($t.owner_address // "?"),
      ($t.lp_holder_count // "?")
    ] | @tsv
  ' "$TMP_JSON" >> "$ROWS" 2>/dev/null || {
    echo "  [$n/$TOTAL] $addr: jq parse loi" >&2
    echo -e "${addr}\tERROR\tjq_parse_error" >> "$ROWS"
  }
  sleep 1.2
done

# ---- phan loai PASS/REVIEW/FAIL tung dong, in bang tom tat ----
PASS=0
REVIEW=0
FAIL=0
ERR=0

echo ""
printf '%-44s %-10s %-7s %s\n' "TOKEN" "VERDICT" "SYMBOL" "LY DO"
printf '%-44s %-10s %-7s %s\n' "--------------------------------------------" "----------" "-------" "-------------------------"

while IFS=$'\t' read -r addr a b c d e f g h i2 j k l m n o p; do
  if [ "${a:-}" = "ERROR" ]; then
    ERR=$((ERR + 1))
    printf '%-44s %-10s %-7s %s\n' "$addr" "REVIEW" "?" "khong goi duoc GoPlus (${b:-loi})"
    continue
  fi
  symbol="$a"; honeypot="$b"; cannot_sell="$c"; buy_tax="$d"; sell_tax="$e"
  blacklist="$f"; cooldown="$g"; pausable="$h"; antiwhale="$i2"; proxy="$j"
  hidden_owner="$k"; take_back="$l"; mintable="$m"; open_source="$n"
  owner="$o"; lp_holders="$p"

  reasons=()
  verdict="PASS"

  # FAIL - loai thang, dung tieu chi AGENTS.md (khong tax/honeypot/blacklist/
  # cooldown-anti-MEV/pausable).
  [ "$honeypot" = "1" ] && { verdict="FAIL"; reasons+=("honeypot"); }
  [ "$cannot_sell" = "1" ] && { verdict="FAIL"; reasons+=("cannot_sell_all"); }
  [ "$blacklist" = "1" ] && { verdict="FAIL"; reasons+=("blacklist"); }
  [ "$cooldown" = "1" ] && { verdict="FAIL"; reasons+=("trading_cooldown"); }
  [ "$pausable" = "1" ] && { verdict="FAIL"; reasons+=("transfer_pausable"); }
  [ "$take_back" = "1" ] && { verdict="FAIL"; reasons+=("can_take_back_ownership"); }
  [ "$hidden_owner" = "1" ] && { verdict="FAIL"; reasons+=("hidden_owner"); }
  # tax > 5% coi la FAIL ro rang (AGENTS.md: "khong tax" - nguong nay chi de
  # loc THO, Chu tu quyet dinh nguong chat hon khi soat tay).
  if [[ "$buy_tax" =~ ^[0-9.]+$ ]] && awk "BEGIN{exit !($buy_tax > 0.05)}"; then
    verdict="FAIL"; reasons+=("buy_tax=${buy_tax}")
  fi
  if [[ "$sell_tax" =~ ^[0-9.]+$ ]] && awk "BEGIN{exit !($sell_tax > 0.05)}"; then
    verdict="FAIL"; reasons+=("sell_tax=${sell_tax}")
  fi

  # REVIEW - can Chu tu soat (khong tu dong loai, nhung khong tu dong PASS).
  if [ "$verdict" = "PASS" ]; then
    if [ "$antiwhale" = "1" ]; then verdict="REVIEW"; reasons+=("anti_whale"); fi
    if [ "$proxy" = "1" ]; then verdict="REVIEW"; reasons+=("proxy_contract"); fi
    if [ "$mintable" = "1" ]; then verdict="REVIEW"; reasons+=("mintable"); fi
    if [ "$open_source" = "0" ]; then verdict="REVIEW"; reasons+=("not_open_source"); fi
    if [[ "$buy_tax" =~ ^[0-9.]+$ ]] && awk "BEGIN{exit !($buy_tax > 0)}"; then verdict="REVIEW"; reasons+=("buy_tax=${buy_tax}"); fi
    if [[ "$sell_tax" =~ ^[0-9.]+$ ]] && awk "BEGIN{exit !($sell_tax > 0)}"; then verdict="REVIEW"; reasons+=("sell_tax=${sell_tax}"); fi
    if [ "$owner" != "?" ] && [ "$owner" != "0x0000000000000000000000000000000000000000" ] && [ -n "$owner" ]; then
      verdict="REVIEW"; reasons+=("owner_not_renounced")
    fi
    if [[ "$lp_holders" =~ ^[0-9]+$ ]] && [ "$lp_holders" -lt 2 ]; then
      verdict="REVIEW"; reasons+=("lp_holder_count=${lp_holders}")
    fi
  fi

  reason_str="ok"
  [ "${#reasons[@]}" -gt 0 ] && reason_str="$(IFS=,; echo "${reasons[*]}")"

  case "$verdict" in
    PASS) PASS=$((PASS + 1)) ;;
    REVIEW) REVIEW=$((REVIEW + 1)) ;;
    FAIL) FAIL=$((FAIL + 1)) ;;
  esac
  printf '%-44s %-10s %-7s %s\n' "$addr" "$verdict" "${symbol:-?}" "$reason_str"
done < "$ROWS"


echo ""
echo "== TOM TAT: tong=$TOTAL PASS=$PASS REVIEW=$REVIEW FAIL=$FAIL (loi_goi_API=$ERR) =="
echo "PASS = khong co co do nao trong bang tren -> ung vien tot de Chu tu soat tiep + dien 'vetted YYYY-MM-DD'."
echo "REVIEW = co it nhat 1 co vang (owner chua renounce/mintable/proxy/tax nho/anti-whale/it LP holder) -> Chu PHAI tu doc contract truoc khi dien vetted."
echo "FAIL = co co do (honeypot/blacklist/cooldown/pausable/tax cao/can_take_back_ownership/hidden_owner) -> KHONG dien vetted, can xoa khoi pairs.txt neu Chu dong y."
echo "Day la LOC THO (GoPlus), KHONG phai ket luan cuoi cung - Chu van phai tu doi chieu bang mat truoc khi dien 'vetted'."
