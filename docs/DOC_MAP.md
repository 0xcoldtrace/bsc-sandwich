# docs/DOC_MAP.md — Đọc gì trước khi làm cụm mới

Thứ tự đọc bắt buộc đầu mỗi phiên (theo CLAUDE.md):

1. `CLAUDE.md` — luật, không đổi.
2. `docs/STATE.md` — quyết định kỹ thuật đã chốt (RPC crate, kiểu số, v.v).
3. `docs/TASKS.md` — cụm nào xong, cụm nào nợ.
4. `DEX_REGISTRY.md` — venue nào đã pin (getCode thật), venue nào DISABLED.
5. `config.toml` — field runtime hiện tại.
6. `baocao/BAOCAO{số lớn nhất}.md` — phiên gần nhất đã làm gì, còn nợ gì.

## File khác

- `vps.json` — chain_id + RPC placeholder cho VPS, không chứa secret thật.
- `.env.example` — tên biến môi trường cần có trong `.env` (không commit `.env`).
- `victims.example.txt` / `victims.txt` — định dạng nạn nhân theo dõi.
- `src/` — mã Rust (`src/lib.rs` khai `pub mod` để `src/bin/` dùng lại được).
  `web/` — static dashboard (HTML/CSS/JS) do bot serve qua axum.
- `src/bin/rpc_probe.rs` — công cụ đo RTT/chain_ok từng URL `BSC_HTTP*`/
  `BSC_WS*` (không gửi tx). Chạy qua `scripts/run_rpc_probe.sh` (Linux/VPS)
  hoặc `scripts/run_rpc_probe.ps1` (Windows) — PHẢI chạy trên VPS vận hành
  thật để số RTT có ý nghĩa, xem README.md.
- `scripts/deploy_vps.sh` / `scripts/deploy_vps.ps1` — copy source lên VPS
  qua SSH (tar+ssh pipe, không nhét password), tuỳ cờ cài rustup/chạy
  probe/build/chạy bot nền. Chỉ dùng khi đã có SSH key/agent sẵn tới VPS,
  xem README.md mục "Deploy nhanh lên VPS".
- `docs/RUN.md` (đổi tên từ `docs/VPS_RUN.md`, cụm `strategy-lock-mode2`) —
  cách chạy paper 30 phút (`scripts/paper_run.sh`, WSL hoặc VPS đều chạy
  được) + cách lọc thô `pairs.txt` bằng GoPlus (`scripts/vet_goplus.sh`)
  trước khi Chủ tự vet tay và điền `vetted YYYY-MM-DD`.
- `baocao/` — báo cáo mỗi phiên, không đè file cũ.
