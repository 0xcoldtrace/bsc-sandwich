#!/usr/bin/env bash
# Cụm `truth-victim-ok-and-memleak` (mục 5) — cài unit systemd THẬT +
# logrotate cho bot trên VPS, thay `nohup`/`systemd-run --collect`.
#
# CHẠY TRÊN VPS (không phải WSL). `scripts/deploy_vps.sh` đã copy source lên
# trước; script này chỉ cài đặt tầng vận hành.
#
# Usage (tren VPS, trong /root/bsc-sandwich):
#   scripts/install_systemd_vps.sh [--start]
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

if [ "${1:-}" = "--start" ]; then
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
