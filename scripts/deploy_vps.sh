#!/usr/bin/env bash
# Deploy source bsc-sandwich len VPS qua SSH (KHONG rsync bat buoc - dung
# tar + ssh pipe de chay duoc tren git-bash Windows lan Linux/macOS thuc).
# Dung khi da CO SSH access toi VPS (key hoac ssh-agent) - script nay
# KHONG BAO GIO nhan/luu password qua tham so hay stdin.
#
# Usage:
#   scripts/deploy_vps.sh --host <ip_hoac_dns> [--user root] [--port 22] \
#     [--identity ~/.ssh/id_khoa] [--path /root/bsc-sandwich] \
#     [--probe] [--build] [--run]
#
#   --probe   sau khi copy, chay scripts/run_rpc_probe.sh TREN VPS
#             (can VPS da co san .env voi BSC_HTTP/BSC_WS that)
#   --build   sau khi copy, chay `cargo build --release` TREN VPS
#   --run     sau build, chay bot NGAM TREN VPS qua `systemd-run --collect`
#             (fallback nohup neu VPS khong co systemd-run), dry_run theo
#             config.toml da copy - script nay khong tu bat live/armed
set -euo pipefail

HOST=""
SSH_USER="root"
PORT="22"
IDENTITY=""
REMOTE_PATH="/root/bsc-sandwich"
DO_PROBE=0
DO_BUILD=0
DO_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/deploy_vps.sh --host <ip> [--user root] [--port 22] \
       [--identity <key>] [--path /root/bsc-sandwich] [--probe] [--build] [--run]

KHONG BAO GIO truyen password qua dong lenh/script nay - dung SSH key
(--identity <file>) hoac ssh-agent da nap san key.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --host) HOST="$2"; shift 2 ;;
    --user) SSH_USER="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    --identity) IDENTITY="$2"; shift 2 ;;
    --path) REMOTE_PATH="$2"; shift 2 ;;
    --probe) DO_PROBE=1; shift ;;
    --build) DO_BUILD=1; shift ;;
    --run) DO_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Tham so khong biet: $1" >&2; usage; exit 1 ;;
  esac
done

if [ -z "$HOST" ]; then
  echo "MISSING: can --host <ip_hoac_dns_vps>" >&2
  usage
  exit 1
fi

SSH_OPTS=(-o BatchMode=yes -o ConnectTimeout=8 -p "$PORT")
if [ -n "$IDENTITY" ]; then
  SSH_OPTS+=(-i "$IDENTITY")
fi

cd "$(dirname "$0")/.."

echo "== 1/4: kiem tra SSH toi ${SSH_USER}@${HOST}:${PORT} (BatchMode - khong hoi password) =="
if ! ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" 'echo ok'; then
  echo "LOI: khong SSH duoc bang key hien co. Kiem tra --identity hoac ssh-agent." >&2
  exit 1
fi

echo "== 2/4: dam bao thu muc dich + rustup tren VPS =="
ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" "mkdir -p '${REMOTE_PATH}'"
ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" '
  if ! command -v cargo >/dev/null 2>&1; then
    echo "rustup chua co tren VPS, cai (khong hoi, stable, -y)..."
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  else
    echo "cargo da co tren VPS, bo qua cai rustup."
  fi
'

echo "== 3/4: copy source (khong target/state/logs/artifacts/.env) qua tar+ssh =="
# Cum `econ-truth-latency-vps` (muc 5) - GIU LAI .git (truoc day loai tru) -
# docs/RUN.md/AGENTS.md yeu cau xac nhan "VPS cung git commit voi WSL" bang
# `git rev-parse HEAD` CHAY TREN VPS - thieu .git thi lenh do bao loi "not a
# git repository", khong the nao verify duoc (phat hien that khi deploy phien
# nay). .git chi ~5MB, khong dang ke so voi thoi gian build release.
tar --exclude='target' --exclude='state' --exclude='logs' \
    --exclude='artifacts' --exclude='.env' -czf - . \
  | ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" "tar -xzf - -C '${REMOTE_PATH}'"
echo "da copy xong vao ${SSH_USER}@${HOST}:${REMOTE_PATH}"

echo "== 4/4: buoc tuy chon (--probe/--build/--run) =="

if [ "$DO_PROBE" = "1" ]; then
  echo "-- rpc_probe TREN VPS (can .env tren VPS co BSC_HTTP/BSC_WS that) --"
  ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" \
    "cd '${REMOTE_PATH}' && source \$HOME/.cargo/env && bash scripts/run_rpc_probe.sh"
fi

if [ "$DO_BUILD" = "1" ]; then
  echo "-- cargo build --release TREN VPS --"
  ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" \
    "cd '${REMOTE_PATH}' && source \$HOME/.cargo/env && cargo build --release"
fi

if [ "$DO_RUN" = "1" ]; then
  echo "-- chay bot NGAM TREN VPS (dry_run theo config.toml da copy) --"
  # Phat hien tu phien deploy-vps-live (xem docs/STATE.md): "nohup ... &
  # disown" qua exec_command SSH khong-tty (paramiko va co the ca ssh CLI
  # thuong khi goi qua kenh khong cap tty) TREO ca kenh SSH vi shell cha
  # khong bao gio thoat duoc du tien trinh con da chay nen. Doi sang
  # `systemd-run --collect` - systemd nhan tien trinh ngay, tra shell ve
  # NGAY LAP TUC, khong phu thuoc tty/kenh SSH con song hay khong.
  RUN_CMD="cd '${REMOTE_PATH}' && if command -v systemd-run >/dev/null 2>&1; then \
    systemd-run --unit=bsc-sandwich-paper --working-directory='${REMOTE_PATH}' --collect \
      --property=StandardOutput=append:/root/bsc-sandwich-run.log \
      --property=StandardError=append:/root/bsc-sandwich-run.log \
      /bin/bash -c 'set -a; [ -f .env ] && source .env; set +a; exec ./target/release/bsc_sandwich'; \
  else \
    echo 'CANH BAO: khong tim thay systemd-run tren VPS nay, fallback nohup (co the treo kenh SSH neu goi qua kenh non-tty).'; \
    nohup ./target/release/bsc_sandwich > /root/bsc-sandwich-run.log 2>&1 & disown; \
  fi; sleep 1; curl -s http://127.0.0.1:8787/api/status || true; echo"
  ssh "${SSH_OPTS[@]}" "${SSH_USER}@${HOST}" "$RUN_CMD"
fi

echo "Xong. Xem dashboard tu may ban qua tunnel (KHONG mo port 8787 public):"
echo "  ssh -N -L 8787:127.0.0.1:8787 -p ${PORT} ${SSH_USER}@${HOST}"
