#!/usr/bin/env bash
# scripts/analyze_econ.sh — cụm `decision-data-24h` (mục 1)
#
# Phân tích KINH TẾ ngoại tuyến từ `logs/bot.jsonl` — chạy được TRÊN VPS mà
# KHÔNG cần toolchain Rust (chỉ bash + jq + awk + sort, đều có sẵn Ubuntu).
# CHỈ ĐỌC log, không sửa file của bot đang chạy, không gọi RPC.
#
# Dùng:
#   scripts/analyze_econ.sh --log logs/bot.jsonl --from-line 43138 \
#       --out /root/analysis [--cluster-file cluster_funded.tsv] [--window 2]
#
# Output (thư mục --out):
#   rows.tsv                 dòng thô đã trích (sim.result + tx.skip)
#   sims.tsv                 chỉ sim.result (ít dòng, dùng cho p80/vốn-80%)
#   1a_pool_quote_hour.tsv   pool × quote × giờ UTC
#   1b_by_quote.tsv          tổng theo quote + 1b_top10_pools.tsv
#   1c_by_hour.tsv           phân bố theo giờ (UTC + giờ VN = UTC+7)
#   1d_summary.txt           kết luận bằng SỐ
#
# Ghi chú phương pháp (đọc kỹ trước khi dùng số):
#  - `net_pos` = dòng `sim.result` có `profit_net_wei > 0` (bot đã trừ gas
#    thật, F-03). Cộng theo ĐƠN VỊ QUOTE GỐC (BNB hoặc USDT) — KHÔNG quy đổi
#    chéo, đúng bài học bug A1 (`ReserveCache` đảo chiều tỉ giá).
#  - Cột `_bnb` chỉ tính khi tỉ giá quote→BNB của CHÍNH dòng đó hợp lệ; tỉ
#    giá > 1.0 với quote khác WBNB là dấu vết bug A1 → LOẠI, đếm riêng
#    (`rate_inverted_rejected`), không tự đảo ngược lại.
#  - `net_pos_non_cluster` = loại các victim thuộc cụm đối thủ MEV. Nguồn cờ,
#    theo thứ tự ưu tiên: (1) field `victim_in_competitor_cluster` nếu log có
#    (binary từ cụm `bugfix-presign-and-contract-plan` trở đi); (2) `from` là
#    1 trong 3 seed; (3) `from` có trong --cluster-file (ví nhận Transfer
#    quote từ seed, dựng bằng `cluster_funded_scan.sh`).
set -euo pipefail

LOG=""; FROM_LINE=1; OUT=""; CLUSTER_FILE=""; WINDOW=2
while [[ $# -gt 0 ]]; do
  case "$1" in
    --log) LOG="$2"; shift 2;;
    --from-line) FROM_LINE="$2"; shift 2;;
    --out) OUT="$2"; shift 2;;
    --cluster-file) CLUSTER_FILE="$2"; shift 2;;
    --window) WINDOW="$2"; shift 2;;
    *) echo "tham so la: $1" >&2; exit 2;;
  esac
done
[[ -n "$LOG" && -n "$OUT" ]] || { echo "thieu --log/--out" >&2; exit 2; }
[[ -f "$LOG" ]] || { echo "khong thay $LOG" >&2; exit 2; }
command -v jq >/dev/null || { echo "thieu jq" >&2; exit 2; }
mkdir -p "$OUT"

SEED1=0xb406021e07b31e1f7850fcccd7076094f18d07ef
SEED2=0xa739dfab40ef6585f1174fce90ec96330669758c
SEED3=0x8180ad6a7c9f8f4864e9909480fba4123fce6c54

echo "== 1/4 trich dong tho (tu dong $FROM_LINE cua $LOG) ==" >&2
tail -n +"$FROM_LINE" "$LOG" \
  | grep -E '"event":"(sim\.result|tx\.skip|latency\.decision_vs_mined|compete\.result)"' \
  > "$OUT/raw.jsonl"

jq -rc 'select(.event=="sim.result" or .event=="tx.skip")
        | [ .event, .ts, (.pair // "-"), (.quote // "-"), (.from // "-"),
            ((.amount_in // "0")|tostring), ((.amount_in_bnb_equiv // "")|tostring),
            ((.front_in_wei // "")|tostring), ((.profit_net_wei // "")|tostring),
            ((.gas_cost_wei // "")|tostring), (.reason // "-"), (.hash // "-"),
            (if .victim_in_competitor_cluster == null then "-" else (.victim_in_competitor_cluster|tostring) end) ]
        | @tsv' "$OUT/raw.jsonl" > "$OUT/rows.tsv"

jq -rc 'select(.event=="latency.decision_vs_mined") | [ .hash, (.mined_block|tostring) ] | @tsv' \
  "$OUT/raw.jsonl" > "$OUT/mined_block.tsv"
jq -rc 'select(.event=="compete.result") | [ (.pair // "-"), (if .competitor == null then "0" else "1" end) ] | @tsv' \
  "$OUT/raw.jsonl" > "$OUT/compete.tsv"

awk -F'\t' '$1=="sim.result"' "$OUT/rows.tsv" > "$OUT/sims.tsv"
echo "   rows=$(wc -l < "$OUT/rows.tsv")  sims=$(wc -l < "$OUT/sims.tsv")  mined=$(wc -l < "$OUT/mined_block.tsv")" >&2

CL="${CLUSTER_FILE:-/dev/null}"
MB="$OUT/mined_block.tsv"

echo "== 2/4 gan co cum doi thu + tinh ti gia tung dong ==" >&2
# enriched.tsv: rows.tsv + 3 cột: in_cluster, in_cluster_strict, rate (0 = khong dung duoc)
awk -F'\t' -v clf="$CL" -v mbf="$MB" -v s1="$SEED1" -v s2="$SEED2" -v s3="$SEED3" -v win="$WINDOW" '
BEGIN{ OFS="\t" }
FILENAME==clf { w=tolower($1); ever[w]=1; if (!(w in fmin) || $2+0 < fmin[w]) fmin[w]=$2+0; if ($2+0 > fmax[w]) fmax[w]=$2+0; next }
FILENAME==mbf { mined[$1]=$2+0; next }
{
  from=tolower($5); flag=$13
  cl=0; strict=0
  if (flag=="true") { cl=1; strict=1 }
  if (from==s1 || from==s2 || from==s3) { cl=1; strict=1 }
  if (from in ever) {
    cl=1
    mb = ($12 in mined) ? mined[$12] : 0
    if (mb>0 && fmin[from] <= mb && fmax[from] >= mb-win) strict=1
    else if (mb==0) strict=strict   # khong biet block -> khong khang dinh
  }
  # ti gia quote->BNB cua CHINH dong nay (xem web.rs::compute_econ_from_rows)
  rate=0
  if ($4=="wbnb") rate=1.0
  else if ($6+0 > 0 && $7!="") { r = ($7+0)/($6+0); if (r>0 && r<=1.0) rate=r }
  print $0, cl, strict, rate
}' "$CL" "$MB" "$OUT/rows.tsv" > "$OUT/enriched.tsv"

echo "== 3/4 bang 1a / 1b / 1c ==" >&2
# cot: 1 event 2 ts 3 pair 4 quote 5 from 6 amount_in 7 bnb_equiv 8 front_in
#      9 profit_net 10 gas_cost 11 reason 12 hash 13 flag 14 cl 15 strict 16 rate
awk -F'\t' '
BEGIN{ OFS="\t" }
{
  hour=substr($2,12,2)+0
  key=$3 "\t" $4 "\t" hour
  cand[key]++
  if ($1=="sim.result" && $9!="" && $9+0>0) {
    net=$9/1e18
    np[key]++; sum[key]+=net
    if (net>best[key]) best[key]=net
    if ($14==0) { npnc[key]++; sumnc[key]+=net }
    if ($16>0) { sumbnb[key]+=net*$16; if ($14==0) sumbnbnc[key]+=net*$16 }
  }
}
END{
  print "pool","quote","hour_utc","candidate","net_pos","net_pos_non_cluster","sum_net_native","sum_net_nc_native","sum_net_nc_bnb","best_native"
  for (k in cand) {
    split(k,a,"\t")
    printf "%s\t%s\t%02d\t%d\t%d\t%d\t%.6f\t%.6f\t%.6f\t%.6f\n", a[1],a[2],a[3],cand[k],np[k]+0,npnc[k]+0,sum[k]+0,sumnc[k]+0,sumbnbnc[k]+0,best[k]+0
  }
}' "$OUT/enriched.tsv" | { read -r h; echo "$h"; sort -k1,1 -k2,2 -k3,3n; } > "$OUT/1a_pool_quote_hour.tsv"

# p80 front_in theo pool x quote (chi tren dong CO LAI), tinh bang sort that
awk -F'\t' '$1=="sim.result" && $9!="" && $9+0>0 && $8!="" { printf "%s\t%s\t%.6f\n", $3,$4,$8/1e18 }' "$OUT/enriched.tsv" \
  | sort -k1,1 -k2,2 -k3,3g \
  | awk -F'\t' 'BEGIN{OFS="\t"} { k=$1"\t"$2; n[k]++; v[k,n[k]]=$3 }
      END{ print "pool","quote","n_profit","front_in_p80_native","front_in_max_native";
           for (k in n){ i=int((n[k]-1)*0.8)+1; if(i<1)i=1; split(k,a,"\t"); print a[1],a[2],n[k],v[k,i],v[k,n[k]] } }' \
  > "$OUT/1a_front_p80.tsv"

# 1b — tong theo quote
awk -F'\t' '
BEGIN{ OFS="\t" }
{
  q=$4
  cand[q]++
  if ($16==0 && $4!="wbnb") { rr[q]++ }
  if ($1=="sim.result") { sim[q]++
    if ($9!="" && $9+0>0) { net=$9/1e18; np[q]++; sum[q]+=net
      if ($14==0) { npnc[q]++; sumnc[q]+=net }
      if ($16>0) { sumbnb[q]+=net*$16; if ($14==0) sumbnbnc[q]+=net*$16 }
      if (net>best[q]) best[q]=net
    }
  }
  if ($14==1) clcand[q]++
}
END{
  print "quote","candidate","candidate_cluster","sim_result","net_pos","net_pos_non_cluster","sum_net_native","sum_net_nc_native","sum_net_nc_bnb","best_native","rate_rejected"
  for (q in cand) printf "%s\t%d\t%d\t%d\t%d\t%d\t%.6f\t%.6f\t%.6f\t%.6f\t%d\n", q,cand[q],clcand[q]+0,sim[q]+0,np[q]+0,npnc[q]+0,sum[q]+0,sumnc[q]+0,sumbnbnc[q]+0,best[q]+0,rr[q]+0
}' "$OUT/enriched.tsv" | { read -r h; echo "$h"; sort; } > "$OUT/1b_by_quote.tsv"

# 1b — top 10 pool theo net_pos_non_cluster
awk -F'\t' '
BEGIN{ OFS="\t" }
FILENAME ~ /compete.tsv$/ { if ($2=="1") touched[$1]=1; next }
{
  p=$3; q=$4
  if (p=="-") next
  key=p"\t"q
  cand[key]++
  if ($14==1) clc[key]++
  if ($1=="sim.result" && $9!="" && $9+0>0) {
    net=$9/1e18; np[key]++; sum[key]+=net
    if ($14==0) { npnc[key]++; sumnc[key]+=net } else { clnp[key]++ }
  }
}
END{
  print "pool","quote","candidate","net_pos","net_pos_non_cluster","sum_net_nc_native","pct_netpos_la_vi_cum","competitor_touched"
  for (k in cand) {
    if (npnc[k]+0==0 && np[k]+0==0) continue
    split(k,a,"\t")
    pct = (np[k]>0) ? (clnp[k]*100.0/np[k]) : 0
    printf "%s\t%s\t%d\t%d\t%d\t%.6f\t%.1f\t%d\n", a[1],a[2],cand[k],np[k]+0,npnc[k]+0,sumnc[k]+0,pct,(a[1] in touched)?1:0
  }
}' "$OUT/compete.tsv" "$OUT/enriched.tsv" \
  | { read -r h; echo "$h"; sort -t$'\t' -k5,5nr -k6,6gr; } > "$OUT/1b_top_pools.tsv"

# 1c — theo gio
awk -F'\t' '
BEGIN{ OFS="\t" }
{
  h=substr($2,12,2)+0
  cand[h]++
  if ($1=="sim.result" && $9!="" && $9+0>0) {
    net=$9/1e18; np[h]++
    if ($14==0) { npnc[h]++; if ($4=="usdt") ncu[h]+=net; else ncw[h]+=net }
  }
}
END{
  print "hour_utc","hour_vn","candidate","net_pos","net_pos_non_cluster","sum_nc_usdt","sum_nc_wbnb"
  for (h=0;h<24;h++) if (h in cand) printf "%02d\t%02d\t%d\t%d\t%d\t%.4f\t%.6f\n", h,(h+7)%24,cand[h],np[h]+0,npnc[h]+0,ncu[h]+0,ncw[h]+0
}' "$OUT/enriched.tsv" > "$OUT/1c_by_hour.tsv"

echo "== 4/4 ket luan 1d ==" >&2
TS_FIRST=$(head -1 "$OUT/rows.tsv" | cut -f2)
TS_LAST=$(tail -1 "$OUT/rows.tsv" | cut -f2)
HOURS=$(awk -v a="$TS_FIRST" -v b="$TS_LAST" 'BEGIN{
  gsub(/[-T:]/," ",a); gsub(/\..*/,"",b); gsub(/[-T:]/," ",b);
  print (mktime(b)-mktime(a))/3600 }')
{
  echo "==== 1d KET LUAN BANG SO ===="
  echo "cua so du lieu: $TS_FIRST  ->  $TS_LAST"
  printf "so gio THAT quan sat duoc: %.2f h  (KHONG phai 24h neu bot chet som)\n" "$HOURS"
  echo "vi cum doi thu nap tu: ${CLUSTER_FILE:-(khong co - chi 3 seed tinh)}"
  awk -F'\t' 'NR>1 { c+=$2; s+=$4; np+=$5; npnc+=$6; rr+=$11 }
      END{ printf "tong: candidate=%d sim.result=%d net_pos=%d net_pos_non_cluster=%d (cum doi thu chiem %.1f%% so net_pos) rate_rejected=%d\n", c,s,np,npnc, (np>0?(np-npnc)*100.0/np:0), rr }' "$OUT/1b_by_quote.tsv"
  echo
  echo "-- (a) pool dat >= 5 net_pos_non_cluster/NGAY (quy doi tu so gio that) --"
  awk -F'\t' -v hours="$HOURS" 'NR>1 && $5>0 { rate=$5*24.0/hours; if (rate>=5) printf "%.4f\t%s quote=%s net_pos_nc=%d -> %.1f/ngay  lai_nc=%.4f %s  vi_cum chiem %.1f%% net_pos\n", rate,$1,$2,$5,rate,$6,$2,$7 }' "$OUT/1b_top_pools.tsv" | sort -t$'\t' -k1,1gr | cut -f2- | sed 's/^/   /'
  echo "   (khong dong nao o tren = KHONG pool nao dat nguong 5/ngay)"
  echo
  echo "-- (b) quote WBNB co pool nao co lai khong --"
  awk -F'\t' 'NR>1 && $2=="wbnb" && $5>0 {n++; printf "   %s net_pos_nc=%d lai=%.6f BNB\n",$1,$5,$6} END{ if(!n) print "   KHONG - 0 pool quote WBNB co net_pos_non_cluster > 0" }' "$OUT/1b_top_pools.tsv"
  echo
  echo "-- (c) gio tap trung (top 5 gio theo net_pos_non_cluster) --"
  awk -F'\t' 'NR>1' "$OUT/1c_by_hour.tsv" | sort -t$'\t' -k5,5nr | head -5 \
    | awk -F'\t' '{printf "   UTC %s (VN %s): net_pos_nc=%d  candidate=%d  usdt=%.2f  wbnb=%.6f\n",$1,$2,$5,$3,$6,$7}'
  echo
  echo "-- (d) von can de lay 80% lai (theo quote, chi dong KHONG thuoc cum) --"
  awk -F'\t' '$1=="sim.result" && $9!="" && $9+0>0 && $14==0 && $8!="" { printf "%s\t%.6f\t%.6f\n", $4,$8/1e18,$9/1e18 }' "$OUT/enriched.tsv" \
    | sort -k1,1 -k2,2g \
    | awk -F'\t' '{ q=$1; n[q]++; f[q,n[q]]=$2; p[q,n[q]]=$3; tot[q]+=$3 }
        END{ for (q in n) { target=tot[q]*0.8; acc=0; cap=0; taken=0;
               for (i=1;i<=n[q];i++){ acc+=p[q,i]; taken=i; cap=f[q,i]; if (acc>=target) break }
               printf "   quote=%s: %d co hoi, tong lai %.4f -> can von %.4f %s de lay %d co hoi (%.4f = %.1f%%)\n", q,n[q],tot[q],cap,q,taken,acc,acc*100/tot[q] } }'
} > "$OUT/1d_summary.txt"

cat "$OUT/1d_summary.txt"
echo "== xong. output tai $OUT ==" >&2
