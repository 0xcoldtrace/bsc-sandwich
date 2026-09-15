#!/usr/bin/env bash
# Alias tương thích ngược: logic thật đã chuyển sang `scripts/paper_run.sh`
# (dùng chung cho WSL và VPS, phiên "wsl-env-rules-paperrun", 2026-09-15).
# Giữ file này để không phá tham chiếu cũ (docs/VPS_RUN.md, BAOCAO33) — chỉ
# forward nguyên tham số, không có logic riêng.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec "$ROOT/scripts/paper_run.sh" "$@"
