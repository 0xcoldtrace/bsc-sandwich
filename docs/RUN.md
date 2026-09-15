# docs/RUN.md — chạy PAPER 30 phút + vet `pairs.txt`

Mục đích: chạy bot ở chế độ **paper (dry_run)** trên máy thật (WSL hoặc VPS,
có `.env` với `BSC_HTTP`/`BSC_WS` thật) trong 30 phút, thu số liệu funnel/
skip/tax/validate — và lọc thô token ứng viên cho `pairs.txt` (mode 2) bằng
GoPlus Security trước khi Chủ tự vet tay.

Script dùng chung: `scripts/paper_run.sh` (phiên "wsl-env-rules-paperrun",
2026-09-15, cập nhật ở cụm `strategy-lock-mode2`) — chạy được cả trên WSL
(máy dev) lẫn VPS, không có bước SSH nào bên trong (SSH chỉ nằm ở
`scripts/deploy_vps.sh`/`.ps1`, dùng để ĐƯA source lên VPS trước khi chạy,
không liên quan script paper run). `scripts/vps_paper_run.sh` cũ giờ chỉ là
alias forward sang `paper_run.sh`, giữ lại để không phá tham chiếu cũ trong
BAOCAO. File này đổi tên từ `docs/VPS_RUN.md` ở cụm `strategy-lock-mode2`
(chạy được trên CẢ WSL lẫn VPS, tên cũ gây hiểu nhầm chỉ dành cho VPS).

## Lọc thô `pairs.txt` bằng GoPlus (`scripts/vet_goplus.sh`)

Bước ĐẦU TIÊN trước khi Chủ tự vet tay + điền `vetted YYYY-MM-DD` vào
`pairs.txt` (xem CLAUDE.md mục "Chiến lược đã chốt"):

```bash
scripts/vet_goplus.sh pairs.txt
```

- Gọi GoPlus Security `token_security` API công khai (chain 56, không cần
  key) cho MỖI token trong `pairs.txt`, in bảng `PASS`/`REVIEW`/`FAIL` +
  dòng tóm tắt số lượng.
- Yêu cầu `curl` + `jq` trên máy chạy. Gọi TUẦN TỰ (không batch — GoPlus âm
  thầm bỏ qua địa chỉ ngoài đầu tiên khi batch) + retry-backoff khi gặp
  rate-limit (`.code != 1` dù HTTP 200) — KHÔNG bao giờ đoán `PASS` khi
  thiếu dữ liệu thật.
- **CHỈ là lọc thô** — `PASS` nghĩa "không cờ đỏ/vàng nào từ GoPlus", KHÔNG
  phải "Chủ đã vet". Chủ vẫn phải tự đọc contract/BscScan/thử swap nhỏ trước
  khi điền `vetted` — script KHÔNG tự sửa `pairs.txt`.

## KHÔNG bao giờ

- Script `scripts/paper_run.sh` **KHÔNG** bật `allow_live`/`bot_armed`/
  `dry_run=false`, **KHÔNG** gửi tx, **KHÔNG** ký, **KHÔNG** in `PRIVATE_KEY`/
  URL RPC có token/IP VPS. Nó chỉ đọc file + gọi API local `127.0.0.1`.
- Redact mọi secret trước khi dán log cho Grok.

## Điều kiện

1. Máy chạy (WSL hoặc VPS) đã có repo + `.env` (chứa `BSC_HTTP`/`BSC_WS`
   thật của Chủ).
2. Đã `cargo build --release` được (script tự build lại).
3. `BSC_WS` nên là host `publicnode` (script tự verify host, **không in URL**).

## Chạy

```bash
# Trong thu muc repo, tren WSL hoac VPS deu chay duoc:
scripts/paper_run.sh --minutes 30 --port 8799
```

Script tự in máy đang chạy (`WSL`/`VPS`, tự phát hiện qua `/proc/version`) và
`sha256sum` của binary vừa build ngay đầu phần kết quả — dán nguyên vào
BAOCAO theo luật #2 (`CLAUDE.md` mục "3 luật bổ sung").

Script sẽ:

1. Verify `BSC_WS` host = publicnode (grep host, không in URL đầy đủ).
2. `cargo build --release`.
3. Xoá `state/halt.lock` / `*.req` cũ.
4. Tạo 1 **config TẠM** (không đụng `config.toml` thật) copy từ `config.toml`
   rồi CHỈ override ngưỡng kinh tế về `0` (`min_profit_bnb`,
   `min_reserve_wbnb`, `max_roundtrip_tax`, `pairs_min_swap_bnb`,
   `min_profit_usdt`, `min_reserve_usdt`) + `web_port=<port>`. Cụm
   `strategy-lock-mode2`: KHÔNG còn ép `pair_scan_universal`/
   `scan_quote_usdt`/`sim_engine` — 3 field đó GIỮ NGUYÊN giá trị ship trong
   `config.toml` thật (mode 2 only, `sim_engine="v2"`), để paper run phản
   ánh ĐÚNG hành vi sản phẩm đang chạy thay vì 1 chế độ đo riêng.
5. In nhanh số dòng `pairs.txt` (tổng/đã có `vetted`/chưa vet — grep thô,
   xem log `pair.reload`/`pair.unvetted` để có số THẬT từ chính bot).
6. Chạy bot ngầm (binary nhận config làm **tham số vị trí thứ 1**), đợi 30 phút.
7. In: 30 dòng `funnel.minute` cuối, `/api/skips`, `/api/funnel`, `/api/tax`,
   `/api/validate`, 20 dòng `tx.skip` (token+venue), 10 dòng `sim.evm`, số
   `Simulated` (đếm `sim.evm` có `decision=simulated`).
8. `halt` sạch: ghi `state/halt.lock` + kill PID.

## Đọc kết quả

- `/api/funnel`: phễu theo gate — `seen → not_pancake_router → decode_fail →
  not_wbnb_pair → venue_v3|venue_v2 → no_pool → below_min → thin_liq →
  honeypot_or_tax → unprofitable → victim_would_revert → simulated`, cộng
  `sim_error` (số tx EVM/RPC không chạy được — nếu cao là RPC quá tải).
- `/api/tax`: cache tax đo tự động bằng `pairs_vet_task` NỀN (mỗi dòng có
  `buy_bps`/`sell_bps`/`honeypot`/`quote`), TTL theo `tax_cache_ttl_sec`.
- `/api/pairs`: cụm `strategy-lock-mode2` — thêm cột `vetted_at`/`candidate`/
  `buy_bps`/`sell_bps`/`honeypot`/`last_vet_sec_ago` (kết quả `pairs_vet_task`
  gần nhất cho từng pool).
- `/api/validate`: chỉ số SỐNG của validator nhúng (B3.4) — `within_1pct_ratio`
  là tỉ lệ dự đoán victim khớp on-chain ≤1%. Đây là thước đo độ chính xác sim
  thay cho việc replay sandwich lịch sử.

## Lưu ý hạ tầng (đo thật, xem `docs/STATE.md` mục cụm này)

- RPC công khai giữ state ~128 block (~96s). Fork EVM (`pairs_vet_task`,
  validator, đo lại trước ký live — xem CLAUDE.md mục "Chiến lược đã chốt")
  luôn mở tại block hiện tại nên luôn trong cửa sổ — không phụ thuộc archive.
- Cụm `strategy-lock-mode2`: `sim_engine="v2"` (ship) — đường nóng KHÔNG mở
  fork EVM mỗi tx nữa, nên log `sim.evm`/`sim_error` từ đường nóng SẼ RỖNG
  trong 1 lần chạy bình thường (không phải lỗi — token trong `pairs.txt` đã
  vet, quyết định `Simulated` dùng công thức đóng V2 + gas thật). Muốn quan
  sát lại đường EVM per-tx (so sánh/đối chiếu), đổi tạm `sim_engine="evm"`
  trong `config.toml` (hot-reload, không cần build lại).
