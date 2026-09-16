#!/usr/bin/env bash
# Cụm `truth-victim-ok-and-memleak` (mục 5) — cài unit systemd THẬT +
# logrotate cho bot trên VPS, thay `nohup`/`systemd-run --collect`.
#
# CHẠY TRÊN VPS (không phải WSL). `scripts/deploy_vps.sh` đã copy source lên
# trước; script này chỉ cài đặt tầng vận hành.
#
# Usage (tren VPS, trong /root/bsc-sandwich):
#   scripts/install_systemd_vps.sh [--start] [--paper-thresholds]
#
# `--paper-thresholds`: dung `config.runtime.toml` voi 6 nguong kinh te ha ve
# 0 — GIONG HET `scripts/paper_run.sh` — de so lieu chay dai tren VPS so sanh
# duoc truc tiep voi cac lan paper run tren WSL. KHONG co co nay thi
# `config.runtime.toml` la ban sao y het `config.toml` ship.
#
# KHONG bat live: bot doc `config.toml` that cua Chu (dry_run=true,
# allow_live=false, bot_armed=false theo ship). Script nay KHONG sua config.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

UNIT_SRC="$ROOT/scripts/bsc-sandwich-paper.service"
UNIT_DST="/etc/systemd/system/bsc-sandwich-paper.service"
LR_SRC="$ROOT/scripts/bsc-sandwich.logrotate"
LR_DST="/etc/logrotate.d/bsc-sandwich"

if grep -qiE 'microsoft|wsl' /proc/version 2>/dev/null; then
  echo "TU CHOI: dang chay tren WSL. Script nay chi chay TREN VPS."
  exit 2
fi
if [ "$(id -u)" != "0" ]; then
  echo "TU CHOI: can quyen root de ghi $UNIT_DST"
  exit 2
fi
if [ ! -x "$ROOT/target/release/bsc_sandwich" ]; then
  echo "TU CHOI: chua co $ROOT/target/release/bsc_sandwich (chay cargo build --release truoc)"
  exit 2
fi
PAPER_THRESHOLDS=0
START=0
# Cum `verify-cluster-as-victim` (muc 1+2) — VPS phai chay shadow mode voi
# allow_competitor_victims=true de sinh `shadow.sim` (revm 3 chan) cho candidate
# CUA CUM DOI THU. O `live_mode="off"` (truoc cum nay) KHONG co dong shadow.sim
# nao trong ca 10,92 h — do la ly do lai xac nhan duoc cua cua so do = 0.
# 2 co nay CHI dung config.runtime.toml; dry_run/allow_live/bot_armed KHONG doi.
LIVE_MODE=""
ALLOW_COMPETITOR=0
for a in "$@"; do
  case "$a" in
    --start) START=1 ;;
    --paper-thresholds) PAPER_THRESHOLDS=1 ;;
    --shadow) LIVE_MODE="shadow" ;;
    --allow-competitor-victims) ALLOW_COMPETITOR=1 ;;
    *) echo "tham so la: $a"; exit 2 ;;
  esac
done

if [ ! -f "$ROOT/.env" ]; then
  echo "TU CHOI: thieu $ROOT/.env (EnvironmentFile cua unit tro toi day)"
  exit 2
fi

# Unit tro cung duong dan /root/bsc-sandwich - tu choi neu repo nam cho khac,
# de khong cai mot unit tro sai cho roi tuong da chay.
if [ "$ROOT" != "/root/bsc-sandwich" ]; then
  echo "TU CHOI: unit tro cung /root/bsc-sandwich nhung repo dang o $ROOT"
  exit 2
fi

# ---- config.runtime.toml (unit KHONG BAO GIO doc thang config.toml) ----
RT="$ROOT/config.runtime.toml"
cp "$ROOT/config.toml" "$RT"
apply() {
  local k="$1" v="$2"
  if grep -qE "^$k[[:space:]]*=" "$RT"; then
    sed -i -E "s#^($k[[:space:]]*=).*#\1 $v#" "$RT"
  else
    printf '%s = %s\n' "$k" "$v" >> "$RT"
  fi
}
if [ "$PAPER_THRESHOLDS" = "1" ]; then
  # DUNG 6 field nhu scripts/paper_run.sh - khong them bot field nao, de 2 may
  # so sanh duoc. KHONG dung toi dry_run/allow_live/bot_armed/sim_engine.
  apply min_profit_bnb 0
  apply min_reserve_wbnb 0
  apply max_roundtrip_tax 0
  apply pairs_min_swap_bnb 0
  apply min_profit_usdt 0
  apply min_reserve_usdt 0
  echo "== config.runtime.toml: 6 nguong kinh te = 0 (giong paper_run.sh) =="
else
  echo "== config.runtime.toml: ban sao y het config.toml ship =="
fi
if [ -n "$LIVE_MODE" ]; then
  apply live_mode "\"$LIVE_MODE\""
  echo "== config.runtime.toml: live_mode = \"$LIVE_MODE\" (KY that bang PRIVATE_KEY, KHONG BAO GIO gui) =="
fi
if [ "$ALLOW_COMPETITOR" = "1" ]; then
  apply allow_competitor_victims true
  echo "== config.runtime.toml: allow_competitor_victims = true (CHI de DO cum doi thu, ship la false) =="
fi
echo "   dry_run/allow_live/bot_armed trong config.runtime.toml:"
grep -E "^(dry_run|allow_live|bot_armed|live_mode|sim_engine|allow_competitor_victims)[[:space:]]*=" "$RT" | sed "s/^/     /"

echo "== dung cac cach chay CU (nohup / systemd-run) truoc khi cai unit that =="
systemctl stop bsc-sandwich-paper.service 2>/dev/null || true
systemctl reset-failed bsc-sandwich-paper.service 2>/dev/null || true
pkill -f 'target/release/bsc_sandwich' 2>/dev/null || true

echo "== cai unit: $UNIT_DST =="
install -m 0644 "$UNIT_SRC" "$UNIT_DST"
echo "== cai logrotate: $LR_DST =="
install -m 0644 "$LR_SRC" "$LR_DST"
# `logrotate --debug` KHONG ghi gi, chi kiem cu phap - chay de loi cu phap lo
# ra NGAY bay gio chu khong phai luc no am tham bo qua file log.
logrotate --debug "$LR_DST" >/dev/null && echo "  cu phap logrotate: OK"

systemctl daemon-reload
systemctl enable bsc-sandwich-paper.service
echo "== da enable (tu chay lai sau reboot) =="

if [ "$START" = "1" ]; then
  systemctl start bsc-sandwich-paper.service
  sleep 3
  systemctl --no-pager --full status bsc-sandwich-paper.service | head -20
fi

echo
echo "Lenh thuong dung:"
echo "  systemctl status bsc-sandwich-paper"
echo "  systemctl restart bsc-sandwich-paper"
echo "  journalctl -u bsc-sandwich-paper -f"
echo "  systemctl show bsc-sandwich-paper -p MemoryMax -p MemoryHigh -p MemoryCurrent"
