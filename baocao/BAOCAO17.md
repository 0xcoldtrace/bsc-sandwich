1. LÁT: `7.3` — executor paper-mode (nối `calldata.rs` vào pipeline thật,
build 2 tx front-buy/back-sell từ `SandwichQuote`, log paper, deadline
wall-clock+buffer thật, amount_out_min từ slippage thật). **Phát hiện đầu
phiên: cụm này ĐÃ ĐƯỢC LÀM XONG Y HỆT ở `baocao/BAOCAO16.md`** (đọc trước khi
sửa gì — xem ô 2). Phiên này KHÔNG sửa code, chỉ verify lại toàn bộ bằng lệnh
chạy thật (không tin lại output cũ dán sẵn) rồi ghi báo cáo mới (không đè
BAOCAO16, đúng luật CLAUDE.md "một phiên một file mới").

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml,
      baocao/BAOCAO15.md.

LÁT: 7.3 — executor paper-mode: nối calldata.rs (encode_front_buy/
     encode_back_sell) vào pipeline thật, build đủ 2 tx (front-buy,
     back-sell) từ SandwichQuote (front_in/front_out) khi decide_paper_v2 trả
     quyết định có lợi nhuận — nhưng CHỈ log ra (paper/dry-run), TUYỆT ĐỐI
     KHÔNG sendRawTransaction, KHÔNG cần signer/private key thật phiên này
     (dry_run đang mặc định true, giữ nguyên). Thêm deadline thật
     (block.timestamp + buffer cấu hình được) và amount_out_min thật (từ
     slippage — nếu cần thêm field config, liệt kê rõ trong docs/STATE.md,
     KHÔNG tự ý sửa config.toml thật của chủ, chỉ thêm field với default hợp
     lệ). Làm nốt test/docs dính cụm trong cùng phiên.
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/pipeline.rs, src/executor.rs (mới hoặc sửa), src/config.rs,
           src/calldata.rs (nếu cần sửa nhỏ), config.toml (chỉ thêm field
           mới với default, không đổi field cũ), docs/STATE.md, docs/TASKS.md,
           baocao/BAOCAO16.md

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, bật cờ
     live/dry_run=false/bot_armed, GỌI eth_sendRawTransaction dưới bất kỳ
     hình thức nào (kể cả "test only"/"comment out"), đổi pair khỏi WBNB,
     hồi sinh chiều victim bán.
```

**QUAN TRỌNG — lệnh này chỉ nói đọc tới `baocao/BAOCAO15.md` và bảo ghi vào
`baocao/BAOCAO16.md`, nhưng file đó ĐÃ TỒN TẠI** (repo đã có sẵn
`baocao/BAOCAO16.md` với đúng cụm `7.3` này, cùng nội dung 10 ô, cùng số test
173, cùng dòng log `tx.build` mẫu). Vì CLAUDE.md ghi rõ "một phiên một file
mới ... không đè", phiên này viết `baocao/BAOCAO17.md` (số kế tiếp thật trong
`baocao/`) thay vì ghi đè `BAOCAO16.md`. Nhiều khả năng Grok gửi nhầm lại lệnh
cũ (giống hệt lệnh đã sinh ra BAOCAO16) — chủ/Grok nên xác nhận lại đã đọc
BAOCAO16 hay chưa trước khi ra lệnh cụm tiếp theo, để tránh nợ vòng copy lặp.

3. FILE ĐỔI: KHÔNG file source nào (không cần sửa gì — toàn bộ `LÀM` của lệnh
đã có sẵn trong repo, xem đối chiếu ô 5/6 dưới). Chỉ tạo:
- `baocao/BAOCAO17.md` — MỚI (file này).

Đối chiếu trực tiếp mã nguồn hiện tại (không suy đoán từ BAOCAO16, tự grep
lại phiên này):
```
$ grep -n "compute_deadline\|apply_slippage\|build_and_log_paper_sandwich\|PaperTxLog\|PLACEHOLDER_SELF_ADDRESS" src/executor.rs | head
144:pub const PLACEHOLDER_SELF_ADDRESS: Address = Address::ZERO;
150:pub fn compute_deadline(buffer_sec: u64) -> U256 {
161:pub fn apply_slippage(expected: U256, slippage_bps: u32) -> U256 {
174:pub struct PaperTxLog {
234:pub fn build_and_log_paper_sandwich(

$ grep -n "decide_and_build_paper_v2" src/pipeline.rs | head -3
363:pub fn decide_and_build_paper_v2(
1155:    fn decide_and_build_paper_v2_logs_tx_build_when_simulated() {
1190:    fn decide_and_build_paper_v2_no_tx_build_when_skipped() {

$ grep -n "executor_deadline_buffer_sec\|executor_slippage_bps" src/config.rs config.toml
src/config.rs:78:    pub executor_deadline_buffer_sec: u64,
src/config.rs:85:    pub executor_slippage_bps: u32,
config.toml:409:executor_deadline_buffer_sec = 120
config.toml:410:executor_slippage_bps = 50
```
Khớp 100% mô tả `LÀM` của lệnh: build 2 tx (front/back) từ `SandwichQuote`,
log qua `logger` khi `Simulated`, cổng `dry_run` (`build_and_log_paper_sandwich`
trả `None` + log `tx.build_skipped` khi `dry_run=false`), `deadline`
wall-clock+buffer thật, `amount_out_min` từ slippage thật — 2 field
`config.toml` mới đã có sẵn với default (`120`/`50`), không field cũ nào bị
đổi.

KHÔNG ĐỤNG: toàn bộ `src/`, `config.toml`, `docs/STATE.md`, `docs/TASKS.md`,
`baocao/BAOCAO16.md` — không sửa gì (đã đúng, không cần ghi thêm).

4. LỆNH CHẠY (chạy lại THẬT phiên này, không dùng lại output cũ dán trong
BAOCAO16):
```
cargo test 2>&1 | tail -40
cargo build --release 2>&1 | tail -10
cargo build --release 2>&1 | grep -i warn
cargo test 2>&1 | grep -E "^test (executor::tests|pipeline::tests::decide_and_build|config::tests::(missing_executor|executor_fields))"
```

5. OUTPUT THẬT:
```
$ cargo test 2>&1 | tail -40
test venues::tests::every_pinned_contract_has_nonzero_get_code_len ... ok
test venues::tests::newer_family_still_disabled_not_deleted ... ok
test venues::tests::scan_and_live_flags_pass_through_unchanged ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
test venues::tests::wbnb_pinned_and_nonzero ... ok
test victims::tests::bnb_to_wei_basic ... ok
test victims::tests::bnb_to_wei_rejects_garbage ... ok
test victims::tests::checksum_and_lowercase_same_wallet ... ok
test victims::tests::duplicate_address_last_line_wins ... ok
test victims::tests::garbage_lines_logged_and_skipped_no_panic ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test pipeline::tests::decide_and_build_paper_v2_logs_tx_build_when_simulated ... ok
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 173 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.09s

     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-7ea757478be717ce.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\bin\rpc_probe.rs (target\debug\deps\rpc_probe-62a9296a36bef9b2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bsc_sandwich

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
```
$ cargo build --release 2>&1 | tail -10
    Finished `release` profile [optimized] target(s) in 0.46s
```
(Không có gì để build lại — binary release đã xanh sẵn từ trước, không có
thay đổi source nào phiên này nên Cargo không recompile, đúng kỳ vọng vì
không sửa file `.rs` nào.)
```
$ cargo build --release 2>&1 | grep -i warn
(rỗng — không có warning nào)
```
```
$ cargo test 2>&1 | grep -E "^test (executor::tests|pipeline::tests::decide_and_build|config::tests::(missing_executor|executor_fields))"
test config::tests::executor_fields_load_ship_defaults ... ok
test config::tests::missing_executor_fields_fail_load ... ok
test executor::tests::apply_slippage_10000_bps_is_zero_no_panic ... ok
test executor::tests::apply_slippage_100_bps_is_1_percent_off ... ok
test executor::tests::apply_slippage_over_10000_bps_clamped_no_panic ... ok
test executor::tests::apply_slippage_zero_bps_keeps_full_amount ... ok
test executor::tests::compute_deadline_is_now_plus_buffer_within_tolerance ... ok
test executor::tests::build_back_sell_paper_tx_roundtrips_through_decoder ... ok
test executor::tests::build_front_buy_paper_tx_roundtrips_through_decoder ... ok
test executor::tests::load_signer_empty_env_var_is_err_no_panic ... ok
test executor::tests::load_signer_valid_hex_key_parses_ok ... ok
test executor::tests::load_signer_garbage_hex_is_err_no_panic ... ok
test executor::tests::dry_run_blocks_live_even_if_everything_else_is_green ... ok
test executor::tests::gate_check_zero_gas_cap_is_a_failure_reason ... ok
test executor::tests::gate_check_ok ... ok
test executor::tests::halt_lock_blocks_live ... ok
test executor::tests::load_signer_missing_env_var_is_err_no_panic ... ok
test executor::tests::gate_check_fail ... ok
test pipeline::tests::decide_and_build_paper_v2_no_tx_build_when_skipped ... ok
test executor::tests::build_and_log_paper_sandwich_logs_tx_build_when_dry_run_true ... ok
test executor::tests::build_and_log_paper_sandwich_skips_when_dry_run_false ... ok
test executor::tests::no_send_raw_transaction_call_anywhere_in_src ... ok
test pipeline::tests::decide_and_build_paper_v2_logs_tx_build_when_simulated ... ok
```
Đủ 22 test riêng cụm `7.3`/config đi kèm, tất cả `ok`, kể cả
`no_send_raw_transaction_call_anywhere_in_src` — xác nhận LẠI (không chỉ tin
BAOCAO16) rằng không có `sendRawTransaction`/tương đương nào trong `src/`.
173 tổng số khớp đúng số BAOCAO16 đã báo (không tăng/giảm vì không đổi test
nào).

6. CHAIN: Không gọi RPC/on-chain nào phiên này (không sửa code). `chain_id:
56` không đổi, không đụng `DEX_REGISTRY.md`/`src/venues.rs`. `eth_getCode`:
MISSING (không cần cho phiên chỉ-verify này).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02.

8. KHÔNG LÀM:
- Không sửa bất kỳ file `.rs`/`config.toml`/`docs/*.md` nào — cụm `7.3` đã
  tồn tại đúng phạm vi lệnh từ `BAOCAO16`, sửa lại sẽ là code thừa/trùng lặp
  không cần thiết.
- Không ghi đè `baocao/BAOCAO16.md` — đúng luật CLAUDE.md, viết file mới
  (`BAOCAO17.md`) thay vào đó.
- Không `sendRaw`/bật `allow_live`/`dry_run=false`/`bot_armed` — không đụng
  `config.toml`.
- Không hồi sinh chiều victim bán — không đụng `pipeline.rs`/`sim_v2.rs`.
- Không nhảy sang việc nối `main.rs::handle_paper_tx` (vẫn ngoài `ĐƯỢC ĐỤNG`
  của lệnh gốc lẫn lệnh phiên này) — giữ nguyên trạng thái nợ đã ghi ở
  BAOCAO16 ô 10.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU (giữ nguyên y hệt BAOCAO16, chưa có gì đổi vì phiên này
không sửa code):
- `main.rs::handle_paper_tx` CHƯA đổi sang gọi `pipeline::decide_and_build_paper_v2`
  (main.rs không nằm trong `ĐƯỢC ĐỤNG` của cả 2 lệnh 7.3 tới giờ) — live loop
  pending-tx thật HIỆN TẠI vẫn gọi `decide_paper_v2` cũ (đã tự grep lại xác
  nhận ở ô 3/4: `src/main.rs:777` gọi `pipeline::decide_paper_v2(...)`,
  KHÔNG phải bản `_and_build_`), nghĩa là KHÔNG có `tx.build` nào được log từ
  live loop thật cho tới khi phiên sau đổi dòng gọi này (chỉ 1 dòng, thêm
  tham số `&app_state.logger`).
- `to` (địa chỉ nhận) vẫn là placeholder `Address::ZERO` — chưa dẫn xuất được
  địa chỉ ví thật từ `B256` (`load_signer`, nợ từ `7.1`).
- `deadline` dùng wall-clock (`SystemTime::now()`), không phải
  `block.timestamp` on-chain thật.
- Chỉ có calldata V2 Router (đúng phạm vi `7.2`) — V3/UR/V4 vẫn chưa có hàm
  build calldata nào.
- `RiskGuard::record_result` vẫn chưa gọi tự động ở đâu.
- Gửi tx thật (`sendRawTransaction`) vẫn HOÀN TOÀN CHƯA LÀM.
- **Nợ mới ghi nhận ở phiên này**: nếu Grok định ra lệnh cụm tiếp theo, nên
  đọc `baocao/BAOCAO16.md` (không phải chỉ `BAOCAO15.md`) trước khi soạn lệnh,
  để tránh lặp lại đúng 1 cụm đã đóng — phiên này không tốn thời gian sửa code
  sai (không có gì để sửa) nhưng tốn 1 vòng verify + 1 file báo cáo không cần
  thiết nếu đã đối chiếu `docs/TASKS.md` trước (dòng `7.3` đã ghi rõ "MỘT
  PHẦN (paper-mode, BAOCAO16)" từ trước).
