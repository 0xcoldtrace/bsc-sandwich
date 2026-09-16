# docs/DOC_MAP.md — Bản đồ toàn bộ file trong repo

## Thứ tự đọc cho Claude Code (đầu mỗi phiên, theo AGENTS.md)

1. `AGENTS.md` — luật, không đổi trừ khi lệnh bảo sửa.
2. `docs/STATE.md` — mục "TRẠNG THÁI HIỆN TẠI" ở đầu file trước, rồi tới
   quyết định kỹ thuật chi tiết theo tên cụm cần biết.
3. `docs/TASKS.md` — cụm nào xong, cụm nào nợ, cụm nào hoãn.
4. `DEX_REGISTRY.md` — venue nào đã pin (getCode thật), venue nào DISABLED.
5. `config.toml` — field runtime hiện tại.
6. `baocao/BAOCAO{số lớn nhất}.md` — phiên gần nhất đã làm gì, còn nợ gì.

## Thứ tự đọc cho Chủ (người vận hành, không cần biết Rust)

1. `README.md` — hướng dẫn vận hành đầy đủ: cài đặt, cấu hình, vet
   `pairs.txt`, chạy paper, dashboard, đọc log, sự cố thường gặp, an toàn.
2. `docs/RUN.md` — chi tiết chạy paper 30 phút (WSL/VPS) + checklist deploy
   VPS đầy đủ (bảo mật, SSH, systemd).

## Toàn bộ file trong repo (trừ `.git/`, `target/`, `key/`, `state/`, `logs/`, `artifacts/` — sinh ra lúc chạy, không phải nguồn)

### Gốc repo

| File | Mô tả |
|---|---|
| `AGENTS.md` | Luật vận hành đầy đủ cho Claude Code — không tự sửa trừ khi được lệnh. |
| `README.md` | Hướng dẫn vận hành chính cho Chủ (không cần biết Rust). |
| `DEX_REGISTRY.md` | Venue PancakeSwap đã pin (V2/V3/V4-Infinity) — địa chỉ, `eth_getCode`, nguồn. |
| `Cargo.toml` / `Cargo.lock` | Khai báo dependency Rust + version khoá thực tế. |
| `config.toml` | Toàn bộ ngưỡng/cờ runtime — nguồn sự thật duy nhất cho giá trị ship. |
| `vps.json` | `chain_id` + RPC placeholder — **CÓ được code đọc thật**
(`src/transport.rs::VpsFallback`/`src/main.rs`) làm fallback khi `.env` rỗng, không phải file trang trí. |
| `.env` | Bí mật thật (gitignored) — không có trong repo git, chỉ tồn tại trên máy chạy. |
| `.env.example` | Mẫu 4 biến `.env` + comment giải thích từng biến. |
| `.gitignore` / `.gitattributes` | Loại trừ file nhạy cảm/sinh ra lúc chạy; ép line-ending. |
| `victims.txt` | Mode 1 (wallet-mode) — hiện TẮT (`wallet_scan_enabled=false`), file chỉ còn 1 dòng comment ghi rõ lý do + định dạng cũ, giữ lại vì `config.toml` vẫn trỏ `victims_path` (thiếu file = fail load). |
| `pairs.txt` | Mode 2 (pair-mode) — nguồn candidate DUY NHẤT đang bật. Token do Chủ tự vet tay, xem README.md mục 4. |

### `docs/`

| File | Mô tả |
|---|---|
| `docs/STATE.md` | Quyết định kỹ thuật cố định theo từng cụm (nguồn chi tiết nhất) + mục "TRẠNG THÁI HIỆN TẠI" tóm tắt ở đầu file. |
| `docs/TASKS.md` | Bảng cụm/BAOCAO/commit/trạng thái + nợ còn thật + mục "Hoãn, lý do". |
| `docs/DOC_MAP.md` | Chính file này. |
| `docs/RUN.md` | Vận hành chi tiết: chạy paper (WSL/VPS) + checklist deploy VPS đầy đủ. |

### `baocao/`

| File | Mô tả |
|---|---|
| `baocao/README.md` | Giải thích khuôn 10 ô + luật commit/hash cho mỗi báo cáo phiên. |
| `baocao/BAOCAO01.md` … `BAOCAO36.md` | Báo cáo từng phiên làm việc, không sửa lại (lịch sử). |
| `baocao/BAOCAO_AUDIT_2026-09-15.md` | Báo cáo audit độc lập toàn diện trước live (F-01..F-27, V-01..V-12). |

### `src/` (mã Rust — xem `src/lib.rs` để biết module nào còn sống/dùng ở đâu)

| File | Mô tả |
|---|---|
| `src/lib.rs` | Khai `pub mod` cho toàn bộ module dưới, để `src/main.rs` và `src/bin/rpc_probe.rs` dùng lại chung 1 bản logic. |
| `src/main.rs` | Binary chính (`bsc_sandwich`) — orchestrate: connect RPC, subscribe pending-tx, vòng lặp paper, web server, các task nền (`pairs_vet_task`, `watch_inject_file`, `halt_watch_task`, ...). |
| `src/config.rs` | Load/validate/hot-reload `config.toml`. |
| `src/victims.rs` | Parse/hot-reload `victims.txt` (mode 1, hiện tắt bằng cờ). |
| `src/pairbook.rs` | Parse/hot-reload `pairs.txt` (mode 2) + gate `vetted`/`pairs_vet_task`. |
| `src/venues.rs` | Registry venue Pancake đã pin (`PANCAKE_ROUTERS`, `SKIP_REASONS`). |
| `src/decoder.rs` | Giải mã calldata router Pancake đã pin thành `DecodedSwap`. |
| `src/pool.rs` | Resolve pool V2/V3/V4-Infinity qua `eth_call`/`eth_getLogs` thật. |
| `src/sim_v2.rs` | Công thức đóng V2 (đường nóng mode 2, `sim_engine="v2"`). |
| `src/sim_v3.rs` | Sim V3 qua `QuoterV2.quoteExactInputSingle` đã pin. |
| `src/sim_evm.rs` | Sim EVM thật qua `revm` — dùng cho vet nền/pre-sign/validator (KHÔNG dùng trên đường nóng, xem AGENTS.md "Chiến lược đã chốt"). |
| `src/tax.rs` | Cache tax roundtrip theo block + hàm `eth_call` đo (giới hạn kỹ thuật, xem `docs/STATE.md`). |
| `src/pipeline.rs` | Lõi quyết định paper (`decide_paper`/`decide_paper_v2`/quote-aware) — decode → gate → sim → outcome. |
| `src/calldata.rs` | Encode calldata front-buy/back-sell V2 Router (dùng cho `7.2`/`7.3` paper-build). |
| `src/executor.rs` | Live gate check + load signer + build/log tx paper-mode (**chưa có hàm gửi tx thật**). |
| `src/relay.rs` | Build request `eth_sendBundle`/`eth_sendMevBundle` 3 leg `[front, victim, back]` (48 Club/BlockRazor, đã sửa F-01) — chưa nối pipeline/HTTP gửi thật. |
| `src/shadow.rs` | Shadow mode (`live_mode="shadow"`) — ký THẬT front/back bằng `PRIVATE_KEY` (`alloy-signer-local`), pre-sign re-vet, KHÔNG BAO GIỜ gửi/broadcast. |
| `src/transport.rs` | Kết nối RPC (HTTP/WSS, đa URL failover), subscribe pending-tx, `vps.json` fallback, `fetch_raw_tx_verified` (tái tạo raw tx đã ký từ hash). |
| `src/state.rs` | Enum state bot + `halt.lock`/`*.req`. |
| `src/logger.rs` | Ghi `logs/bot.jsonl`. |
| `src/web.rs` | Axum server + toàn bộ route `/api/*` (kể cả `/api/shadow`), serve static `web/`. |
| `src/bin/rpc_probe.rs` | Binary phụ đo RTT/`chain_ok` từng URL RPC (không gửi tx), dùng qua `scripts/run_rpc_probe.sh`. |
| `src/bin/competitor_recon.rs` | Binary phụ trinh sát đối thủ MEV thật qua `eth_getLogs`/`eth_getTransactionByHash` (chỉ đọc, không gửi tx) — cụm `competitor-recon-and-strategy`. |

### `web/` (dashboard tĩnh, serve qua `axum`)

| File | Mô tả |
|---|---|
| `web/index.html` | Khung trang dashboard (các khối theo AGENTS.md mục "Web"). |
| `web/app.js` | Gọi `/api/*`, render bảng/số liệu. |
| `web/style.css` | Style tối giản. |

### `scripts/`

| File | Mô tả |
|---|---|
| `scripts/vet_goplus.sh` | Lọc thô `pairs.txt` bằng GoPlus Security API trước khi Chủ tự vet tay. |
| `scripts/paper_run.sh` | Chạy paper N phút (WSL hoặc VPS, dùng chung) + build + halt sạch cuối phiên. |
| `scripts/deploy_vps.sh` | Copy source lên VPS qua SSH (tar+ssh pipe) + tuỳ cờ cài rustup/build/chạy nền. Chỉ có bản Linux/macOS/git-bash — không có bản Windows PowerShell (đã xoá, xem "Đã bỏ"). |
| `scripts/run_rpc_probe.sh` | Chạy `rpc_probe` đọc `.env`, không in secret — PHẢI chạy trên VPS vận hành thật để RTT có ý nghĩa. |
| `scripts/latency_probe.sh` | Đo latency ngoài (bash+curl thuần), công cụ chẩn đoán phụ, không sửa code sản phẩm. |
| `scripts/block_latency.sh` | Đọc `rpc.block` trong `logs/bot.jsonl` để đo độ trễ nhận block, công cụ chẩn đoán phụ. |

## Đã bỏ (xoá ở cụm `docs-cleanup-mode2`, 2026-09-15 — lý do trong `baocao/BAOCAO37.md`)

| File đã xoá | Lý do |
|---|---|
| `victims.example.txt` | Mode 1 (wallet-mode) tắt mặc định; `victims.txt` giữ lại 1 dòng comment ghi định dạng cũ, không cần file ví dụ riêng nữa. |
| `scripts/vps_paper_run.sh` | Chỉ là alias forward sang `scripts/paper_run.sh` (đã dùng chung cho cả WSL lẫn VPS từ cụm `wsl-env-rules-paperrun`) — không còn lý do giữ 2 tên cho cùng 1 script. |
| `scripts/deploy_vps.ps1` | Bản PowerShell/Windows — dev đã chuyển hẳn sang WSL (`AGENTS.md`: "dev = WSL"), không còn máy Windows nào chạy script này. |
| `scripts/run_rpc_probe.ps1` | Cùng lý do trên (bản Windows của `run_rpc_probe.sh`). |

`docs/VPS_RUN.md` đã đổi tên thành `docs/RUN.md` từ cụm `strategy-lock-mode2`
(trước cụm này, không phải xoá) — không phải file bị xoá, chỉ đổi tên.
