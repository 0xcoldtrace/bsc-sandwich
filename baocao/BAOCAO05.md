1. LÁT: config — mọi ngưỡng so sánh (min_profit_bnb, max_front_bnb,
min_reserve_wbnb, max_roundtrip_tax, min_swap_bnb từng ví) đọc thẳng từ
`Config`/`VictimBook`, không hardcode literal trong `pipeline.rs`/`sim_v2.rs`
production path; thêm `config_reload_sec` hot-reload `config.toml` giống
`victims_reload_sec` (không cần restart); validate load CHỈ 4 lý do (thiếu
field, chain_id≠56, số âm/không hữu hạn, parse lỗi) — cho phép
`min_profit_bnb=0`/`max_roundtrip_tax=0`/`max_front_bnb` rất lớn.

**Ghi chú cho Grok**: phiên này chủ gửi lệnh MỚI (dán dưới, ô 2) giữa lúc
Claude Code đang đọc/nghiên cứu để làm `5.1` (subscribe pending-tx +
wire `decide_paper` vào `main.rs`), CHƯA viết dòng code nào của `5.1` lúc
đó. Lệnh mới đổi hẳn sang cụm config-hot-reload (không dính `5.1`), nên
phiên này làm TRỌN cụm config-hot-reload, `5.1` (pending-tx) vẫn CHƯA LÀM —
xem ô 10 và `docs/TASKS.md`. Đây là baocao/BAOCAO05.md (chưa có BAOCAO05
nào trước đó trong phiên `5.1` cũ vì phiên đó bị đổi lát trước khi ra file).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, config.toml, src/config.rs, src/pipeline.rs, src/sim_v2.rs, src/victims.rs
LÁT: config — min/max chủ chỉnh tự do, đổi file là chạy, không chặn cứng trong Rust.

ĐƯỢC ĐỤNG: config.toml, src/config.rs, pipeline/sim/victims/web nếu đang hardcode số, docs/STATE.md, CLAUDE.md CHỈ đoạn config min/max, baocao/BAOCAO05.md (hoặc BAOCAO05b nếu 5.1 đã có số khác)

LÀM:
1) Mọi ngưỡng so sánh đọc Config / VictimBook:
   min_profit_bnb, max_front_bnb, min_reserve_wbnb, max_roundtrip_tax,
   max_exposure_bnb, max_consecutive_loss, gas wei, tax_cache_blocks,
   min_swap_bnb từng ví.
2) Cấm literal 0.01/1.5/20 trong sim/pipeline (test được hardcode fixture, production path không).
3) Fail load CHỈ: thiếu field, chain_id≠56, số âm, parse lỗi.
   Cho phép min_profit_bnb=0, max_roundtrip_tax=0, max_front_bnb rất lớn (vd 100).
4) Thêm config_reload_sec (ship 15): hot-reload config.toml giống victims.
   Đổi min_profit lúc bot đang chạy → lần decide sau dùng số mới, không crash.
5) Test:
   - load max_front_bnb=100, min_profit_bnb=0 → OK
   - load min_profit_bnb=-1 → fail
   - đổi config giữa 2 lần decide_paper (reload) → trần front theo số mới
6) README 5 dòng: sửa config.toml / victims.txt rồi không cần sửa Rust.

KHÔNG LÀM: live send, decoder mới.

ĐẠT CẦN DÁN: cargo test ≥15 dòng + 1 case min_profit=0 load OK.

VIẾT BAOCAO đủ 10 ô. CHỜ GROK. Cấm ĐẠT.
```
(kèm khối "Cách chỉnh sau này" tham khảo, đã phản ánh vào README — xem ô 3.)

3. FILE ĐỔI:
- `config.toml` — thêm `config_reload_sec = 15` (field mới, bắt buộc, ship
  15 khớp `victims_reload_sec`).
- `src/config.rs` — thêm field `config_reload_sec: u64` +
  `#[serde(skip)] last_reload: Option<Instant>` vào `Config`. Thêm
  `ConfigError::InvalidNumber(String)` + `Config::validate()` chặn số
  âm/không hữu hạn cho 5 field ngưỡng (`min_profit_bnb`, `max_front_bnb`,
  `min_reserve_wbnb`, `max_roundtrip_tax`, `max_exposure_bnb`) — biên trên
  KHÔNG chặn. Thêm hàm free `bnb_f64_to_wei(f64) -> U256` (round-based, có
  doc-comment nêu rõ đánh đổi độ chính xác so `victims::bnb_str_to_wei`) +
  method `Config::min_profit_wei/max_front_wei/min_reserve_wei/
  max_roundtrip_tax_bps/gas_wei`. Thêm `Config::reload_if_due(path,
  interval, now) -> bool` — CÙNG khuôn mẫu `VictimBook::reload_if_due`
  (`src/victims.rs`): reload lỗi thì GIỮ config cũ + log, không panic.
  `Config::from_str` tự đặt `last_reload = Some(Instant::now())` khi load
  xong (khớp quy ước `VictimBook`). 12 test mới (2 case ĐẠT yêu cầu: `max_front_bnb=100 + min_profit_bnb=0` OK,
  `min_profit_bnb=-1` fail; cộng `each_negative_threshold_field_fails_load`
  test riêng 4 field khác; `bnb_f64_to_wei`/`max_roundtrip_tax_bps` chính
  xác; 2 test reload file thật — pass + giữ nguyên khi lỗi).
- `src/pipeline.rs` — `decide_paper` đổi CHỮ KÝ: nhận `cfg: &Config` thay 3
  tham số rời (`tax_cache_blocks`/`max_front_wei`/`gas_wei`) của `4.1`. Thêm
  variant `PipelineSkip::ThinLiq` (`"thin_liq"`, enum đã có tên trong
  `SKIP_REASONS`/CLAUDE.md từ trước nhưng CHƯA có nhánh nào sinh ra nó).
  Thêm 2 nhánh mới trong `decide_paper`: `thin_liq` (reserve WBNB <
  `cfg.min_reserve_wei()`, đặt sau victims.txt check, trước tax cache) và mở
  rộng `honeypot_or_tax` (cache tươi nhưng `roundtrip_tax_bps >
  cfg.max_roundtrip_tax_bps()` vẫn skip) + mở rộng `unprofitable` (lãi dương
  nhưng dưới `cfg.min_profit_wei()` vẫn skip, không chỉ check `<= 0`). Cập
  nhật TẤT CẢ test cũ dùng `decide_paper` sang chữ ký mới qua helper
  `test_config()`/`test_config_toml(overrides)`. Thêm 4 test mới:
  `thin_liq_skip_when_pool_reserve_below_min_reserve_config`,
  `unprofitable_skip_when_profit_positive_but_below_min_profit_bnb_config`,
  `honeypot_or_tax_skip_when_measured_tax_exceeds_max_roundtrip_tax_config`,
  `changing_config_between_two_decide_paper_calls_changes_front_cap` (ĐẠT
  yêu cầu #5 "đổi config giữa 2 lần decide_paper").
- `src/web.rs` — `AppStateInner.config: Config` -> `RwLock<Config>` (cần
  cho hot-reload runtime); `status()`/`venues()` đọc qua `.read().await`
  (2 chỗ duy nhất trong repo dùng field này ngoài `main.rs`).
- `src/main.rs` — construct `AppStateInner{ config: RwLock::new(cfg), ...}`;
  thêm 1 `tokio::spawn` task nền reload `config.toml` mỗi `config_reload_sec`
  (khuôn mẫu Y HỆT task reload `victims.txt` đã có sẵn — gọi
  `Config::reload_if_due`, log event `config.reload` khi đổi thật); sửa 1
  chỗ đọc `app_state.config.dry_run` -> `.read().await.dry_run`. Bắt buộc
  đụng file này dù không nằm rõ trong "ĐƯỢC ĐỤNG" vì đây là nơi DUY NHẤT có
  thể spawn task hot-reload runtime (cùng vị trí task reload victims đã có
  từ `0.1+0.2+0.3`, không có chỗ khác hợp lý hơn) — ghi rõ ở đây để Grok
  biết đây là việc dính bắt buộc, không phải lấn cụm.
- `CLAUDE.md` — CHỈ đoạn "## Config — thiếu field = fail load": thêm
  `config_reload_sec` vào danh sách field bắt buộc + dòng Ship; thêm 1 câu
  mới nêu rõ 5 field ngưỡng chủ chỉnh tự do, không hardcode, fail-load chỉ 4
  lý do, cho phép biên 0/lớn — đúng phạm vi "CHỈ đoạn config min/max" trong
  lệnh.
- `docs/STATE.md` — thêm mục "Config chỉnh tự do + hot-reload" (quyết định
  giữ `f64` thay vì đổi `victims.txt`-style string, đánh đổi độ chính xác
  `bnb_f64_to_wei`, thứ tự skip mới, verify runtime thật).
- `docs/TASKS.md` — thêm dòng cụm config-hot-reload (XONG), cập nhật `5.1`
  ghi rõ CHƯA LÀM vì phiên bị đổi lát, thêm mục Nợ cho
  `max_exposure_bnb`/`max_consecutive_loss`/`gas_reserve_bnb_wei` (validate
  xong nhưng chưa có nơi tiêu thụ logic — CHƯA TỪNG có, không phải hardcode
  bị bỏ sót).
- `README.md` — thêm mục ngắn "Chỉnh ngưỡng sau này (không đụng code)".
- KHÔNG đụng: `victims.txt` thật, `.env`, `DEX_REGISTRY.md`, `src/decoder.rs`,
  cờ live nào, `src/executor.rs` (không viết decoder mới, không gửi tx).

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok -- --nocapture
cargo test config::tests::min_profit_bnb_negative_fails -- --nocapture
cargo test pipeline::tests::changing_config_between_two_decide_paper_calls_changes_front_cap -- --nocapture
```
Runtime smoke test thật (KHÔNG đụng `config.toml`/`victims.txt`/`.env` thật
của repo — copy sang `/tmp/smoke/config.toml`, đổi `web_port=18787` +
`config_reload_sec=2` để quan sát nhanh, port 8787 thật lúc đó đang bị 1
tiến trình `node.exe` không liên quan chiếm, không phải bsc_sandwich cũ):
```
target/debug/bsc_sandwich.exe /tmp/smoke/config.toml &
curl http://127.0.0.1:18787/api/venues   # scan_v2=true luc dau
# sua /tmp/smoke/config.toml: scan_v2 = true -> false, doi qua config_reload_sec
curl http://127.0.0.1:18787/api/venues   # scan_v2=false, KHONG restart
```

5. OUTPUT THẬT:

`cargo test` (90 passed, 2 ignored — 2 ignored cần RPC sống, không đổi từ
BAOCAO04, không nằm trong suite mặc định; ≥15 dòng cuối thật):
```
running 92 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok ... ok
test config::tests::chain_id_1_fails ... ok
test config::tests::max_roundtrip_tax_bps_rounds_to_nearest ... ok
test config::tests::min_profit_bnb_negative_fails ... ok
test config::tests::dry_run_true_blocks_live_gate ... ok
test config::tests::missing_min_profit_bnb_fails ... ok
test config::tests::max_roundtrip_tax_zero_load_ok ... ok
test config::tests::load_ok ... ok
test config::tests::each_negative_threshold_field_fails_load ... ok
test config::tests::reload_keeps_old_config_on_parse_error_no_panic ... ok
test config::tests::reload_respects_interval_and_picks_up_new_min_profit ... ok
test pipeline::tests::thin_liq_skip_when_pool_reserve_below_min_reserve_config ... ok
test pipeline::tests::honeypot_or_tax_skip_when_measured_tax_exceeds_max_roundtrip_tax_config ... ok
test pipeline::tests::unprofitable_skip_when_profit_positive_but_below_min_profit_bnb_config ... ok
test pipeline::tests::changing_config_between_two_decide_paper_calls_changes_front_cap ... ok
test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero ... ok
test pipeline::tests::victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured ... ok
test pipeline::tests::victim_b_below_min_is_skipped_with_clear_reason ... ok
test pipeline::tests::address_not_in_victims_txt_is_not_in_list ... ok
test pipeline::tests::log_outcome_writes_expected_events ... ok
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ignored
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ignored
(... decoder/pool/sim_v2/sim_v3/state/tax/transport/venues/victims/logger/
executor đều pass, không lặp lại hết cho gọn — tổng đủ số dưới)

test result: ok. 90 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Case ĐẠT #1 (`max_front_bnb=100, min_profit_bnb=0` → OK):
```
running 1 test
test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out; finished in 0.00s
```
(assert bên trong: `cfg.max_front_bnb==100.0`, `cfg.min_profit_bnb==0.0`,
`cfg.min_profit_wei()==U256::ZERO` — pass.)

Case ĐẠT #2 (`min_profit_bnb=-1` → fail):
```
running 1 test
test config::tests::min_profit_bnb_negative_fails ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out; finished in 0.00s
```
(assert `Err(ConfigError::InvalidNumber("min_profit_bnb"))` — pass, đúng
"fail load".)

Case ĐẠT #3 (đổi config giữa 2 lần `decide_paper` → trần front đổi):
```
running 1 test
test pipeline::tests::changing_config_between_two_decide_paper_calls_changes_front_cap ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out; finished in 0.00s
```
(`cfg_before.max_front_bnb=1.5` → `front_before` áp sát trần cũ; "reload"
đổi `max_front_bnb=0.001` → `front_after <= cfg_after.max_front_wei()` VÀ
`front_after < front_before` — cả 2 assert pass, chứng minh `Config` mới
thay `Config` cũ là lần `decide_paper` SAU dùng số mới ngay.)

Runtime hot-reload THẬT (binary chạy, không phải unit test — port scratch
`18787`, không đụng file thật của repo):
```
--- venues truoc khi sua config.toml ---
{"venues":[...{"family":"V2","scan_enabled":true,...}...]}
--- sua scan_v2 = true -> false trong /tmp/smoke/config.toml, doi qua config_reload_sec=2s ---
--- venues sau khi sua ---
{"venues":[...{"family":"V2","scan_enabled":false,...}...]}
```
(`grep '"family":"V2"' | grep scan_enabled` xác nhận `scan_enabled:false`
sau lần reload, KHÔNG restart binary.) `logs/bot.jsonl` (chạy từ cwd repo,
thư mục gitignored, không vào git):
```
{"event":"config.reload","max_front_bnb":1.5,"max_roundtrip_tax":0.005,"min_profit_bnb":0.01,"min_reserve_wbnb":20.0,"ts":"2026-09-14T12:57:08.433574400+00:00"}
{"event":"config.reload","max_front_bnb":1.5,"max_roundtrip_tax":0.005,"min_profit_bnb":0.01,"min_reserve_wbnb":20.0,"ts":"2026-09-14T12:57:12.436397700+00:00"}
{"event":"config.reload","max_front_bnb":1.5,"max_roundtrip_tax":0.005,"min_profit_bnb":0.01,"min_reserve_wbnb":20.0,"ts":"2026-09-14T12:57:14.438312600+00:00"}
```
`/api/health` trong lúc chạy: `{"status":"ok"}` — boot không crash sau khi
đổi `AppStateInner.config` sang `RwLock<Config>`.

6. CHAIN: KHÔNG có `eth_call`/`eth_getCode` mới phiên này (cụm config
thuần Rust + hot-reload, không đụng pin/venue) — `0x38` chỉ được xác nhận
gián tiếp qua `Config::validate()` (`chain_id != 56` fail load, test
`chain_id_1_fails` vẫn pass, không đổi hành vi cũ) và qua log
`bsc_sandwich boot: chain_id=56 ...` khi chạy binary thật ở mục 5. Không có
gì để dán `getCode`/`eth_call` thật — ghi `MISSING (không thuộc phạm vi
phiên này)` đúng luật, không bịa.

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không
nằm trong ĐƯỢC ĐỤNG, không pin address mới, không đổi contract nào).

8. KHÔNG LÀM: không gửi tx thật (`send_raw_transaction` vẫn không tồn tại
trong repo); không đổi `dry_run`/`allow_live`/`bot_armed`/cờ live nào; không
sửa `.env`/`victims.txt` thật/`DEX_REGISTRY.md`; không viết decoder mới
(`src/decoder.rs` không đổi 1 dòng nào); không quay lại wire `5.1`
(pending-tx subscription/`state/inject_tx.jsonl`) — chủ đổi lát trước khi
`5.1` có code; không wire `max_exposure_bnb`/`max_consecutive_loss`/
`gas_reserve_bnb_wei` vào logic thật (chưa có nơi tiêu thụ hợp lý, chỉ
validate load — ghi rõ CÒN NỢ, không bịa risk-guard giả); không đụng
`web/index.html`/`web/app.js` (không hardcode số ở đó, kiểm tra rồi, không
cần sửa).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `5.1` (subscribe pending-tx thật + wire `decide_paper`/`log_outcome`/
  `skip_counts` vào `main.rs`) VẪN CHƯA LÀM — lệnh gốc phiên này (đầu hội
  thoại) là `5.1`, nhưng chủ gửi lệnh mới đổi hẳn sang config-hot-reload
  trước khi có code `5.1` nào được viết. Cần lệnh riêng cho `5.1` ở phiên
  sau (đã có gợi ý format `state/inject_tx.jsonl` từ lệnh gốc, chưa dùng).
- `max_exposure_bnb`/`max_consecutive_loss`/`gas_reserve_bnb_wei`: validate
  load xong (số âm fail) nhưng CHƯA có risk-guard/wallet-balance-check nào
  tiêu thụ — cần cụm riêng (khả năng thuộc `5.1+`/`7.x`, nơi có vòng lặp
  live/số dư ví thật để so sánh).
- `skip_counts` (`AppStateInner`, `/api/skips`) vẫn CHƯA được tăng ở đâu cả
  (kế thừa từ BAOCAO04, không phải nợ mới) — chỉ tăng khi `5.1` nối vòng
  lặp pending-tx thật gọi `decide_paper` rồi cập nhật map này.
- Port `8787` thật trên máy dev hiện bị `node.exe` (PID khác, không liên
  quan bot) chiếm — không phải lỗi code, chỉ là môi trường máy chủ lúc test;
  `cargo run` bình thường (không đổi `web_port`) sẽ lỗi bind nếu tiến trình
  đó còn sống, chủ tự xử lý (đổi `web_port` trong `config.toml` hoặc tắt
  tiến trình đang giữ cổng) khi chạy thật.
