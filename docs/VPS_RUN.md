# docs/VPS_RUN.md — chạy PAPER 30 phút trên VPS (cụm `evm-validate-fixed-then-wire` D3)

Mục đích: chạy bot ở chế độ **paper (dry_run)** trên VPS thật (có `.env` với
`BSC_HTTP`/`BSC_WS` thật) trong 30 phút, với cấu hình ngưỡng-0 tối đa hoá số
candidate để **quan sát đường EVM (`sim_engine="evm"`) hoạt động trên mempool
sống** và thu số liệu funnel/skip/tax/validate.

## KHÔNG bao giờ

- Script `scripts/vps_paper_run.sh` **KHÔNG** bật `allow_live`/`bot_armed`/
  `dry_run=false`, **KHÔNG** gửi tx, **KHÔNG** ký, **KHÔNG** in `PRIVATE_KEY`/
  URL RPC có token/IP VPS. Nó chỉ đọc file + gọi API local `127.0.0.1`.
- Redact mọi secret trước khi dán log cho Grok.

## Điều kiện

1. VPS đã có repo + `.env` (chứa `BSC_HTTP`/`BSC_WS` thật của Chủ).
2. Đã `cargo build --release` được (script tự build lại).
3. `BSC_WS` nên là host `publicnode` (script tự verify host, **không in URL**).

## Chạy

```bash
# TREN VPS, trong thu muc repo:
scripts/vps_paper_run.sh --minutes 30 --port 8799
```

Script sẽ:

1. Verify `BSC_WS` host = publicnode (grep host, không in URL đầy đủ).
2. `cargo build --release`.
3. Xoá `state/halt.lock` / `*.req` cũ.
4. Tạo 1 **config TẠM** (không đụng `config.toml` thật) copy từ `config.toml`
   rồi override: `min_profit_bnb=0`, `min_reserve_wbnb=0`, `max_roundtrip_tax=0`,
   `pairs_min_swap_bnb=0`, `min_profit_usdt=0`, `min_reserve_usdt=0`,
   `pair_scan_universal=true`, `scan_quote_usdt=true`, `sim_engine="evm"`,
   `web_port=<port>`. Ngưỡng-0 để KHÔNG lọc mất candidate nào ở tầng kinh tế —
   quan sát tối đa số tx đi tới đường EVM.
5. Chạy bot ngầm (binary nhận config làm **tham số vị trí thứ 1**), đợi 30 phút.
6. In: 30 dòng `funnel.minute` cuối, `/api/skips`, `/api/funnel`, `/api/tax`,
   `/api/validate`, 20 dòng `tx.skip` (token+venue), 10 dòng `sim.evm`, số
   `Simulated` (đếm `sim.evm` có `decision=simulated`).
7. `halt` sạch: ghi `state/halt.lock` + kill PID.

## Đọc kết quả

- `/api/funnel`: phễu theo gate — `seen → not_pancake_router → decode_fail →
  not_wbnb_pair → venue_v3|venue_v2 → no_pool → below_min → thin_liq →
  honeypot_or_tax → unprofitable → victim_would_revert → simulated`, cộng
  `sim_error` (số tx EVM/RPC không chạy được — nếu cao là RPC quá tải).
- `/api/tax`: cache tax đo tự động (mỗi dòng có `buy_bps`/`sell_bps`/`honeypot`/
  `quote`), TTL theo `tax_cache_ttl_sec`.
- `/api/validate`: chỉ số SỐNG của validator nhúng (B3.4) — `within_1pct_ratio`
  là tỉ lệ dự đoán victim khớp on-chain ≤1%. Đây là thước đo độ chính xác sim
  thay cho việc replay sandwich lịch sử.
- `sim.evm` (log): mỗi candidate tới bước sim EVM — `front_in`/`profit_after_gas`/
  `victim_success`/`buy_tax_bps`/`sell_tax_bps`/`attempts`/`ms`/`decision`.

## Lưu ý hạ tầng (đo thật, xem `docs/STATE.md` mục cụm này)

- RPC công khai giữ state ~128 block (~96s). Đường EVM fork tại **block hiện
  tại** nên luôn trong cửa sổ — không phụ thuộc archive.
- `sim_engine="evm"` mở 1 fork mỗi candidate tới bước sim (số này ít sau các
  gate rẻ). Nếu RPC không kham nổi, tăng `sim_error` — có thể đổi tạm
  `sim_engine="v2"` (hot-reload) để so sánh tải.
