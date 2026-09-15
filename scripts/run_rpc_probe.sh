#!/usr/bin/env bash
# Cụm `rpc-probe`. Nạp .env (nếu có) vào biến môi trường của TIẾN TRÌNH CON
# này rồi chạy `cargo run --bin rpc_probe --release` — KHÔNG in nội dung
# .env ra màn hình, KHÔNG sửa .env. Dùng trên VPS Linux (NJ) hoặc máy dev
# WSL/Linux/macOS — dev đã chuyển hẳn sang WSL, không còn bản Windows
# PowerShell riêng (đã xoá ở cụm `docs-cleanup-mode2`).
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
else
  echo "MISSING: khong thay file .env o $(pwd) (chi anh huong bien moi truong, khong tu tao .env)"
fi

cargo run --bin rpc_probe --release
