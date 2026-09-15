1. LÁT: `7.3` — executor paper-mode: nối `calldata.rs` (`encode_front_buy`/
`encode_back_sell`, `7.2`/BAOCAO15) vào pipeline THẬT, build đủ 2 tx
(front-buy, back-sell) từ `SandwichQuote` khi `decide_paper_v2` trả quyết
định có lợi nhuận — CHỈ LOG (paper/dry-run), KHÔNG `sendRawTransaction`,
không cần signer/private key thật. Thêm `deadline` thật (wall-clock hiện tại
+ buffer cấu hình được) và `amount_out_min` thật (từ slippage, 2 field
`config.toml` mới với default). Làm nốt test/docs dính cụm trong cùng phiên.

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

LÀM:
- Hàm build 2 tx object (front, back) từ SandwichQuote + calldata.rs, log
  đầy đủ ra logs/bot.jsonl (event tx.build hoặc tương tự) — có địa chỉ to,
  calldata hex, value, gas ước tính.
- Nếu allow_live/dry_run/bot_armed chưa đủ điều kiện live-gate (chắc chắn
  đúng vì ship default dry_run=true) → chỉ log, dừng lại, không có code path
  nào dẫn tới ký/gửi.
- Test xác nhận: dry_run=true thì không có lời gọi sendRaw nào (grep code
  hoặc mock provider đếm số lần gọi = 0).

KHÔNG LÀM: gửi tx thật bất kỳ hình thức nào, đổi stack, pair không-WBNB,
hồi sinh chiều bán, tự ý bật cờ live trong config.toml thật.
NỢ: không tách test/docs ra phiên sau.

ĐẠT CẦN DÁN: cargo test + cargo build --release, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO16.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

3. FILE ĐỔI:
- `src/executor.rs` — thêm khối `7.3`: `PLACEHOLDER_SELF_ADDRESS` (=
  `Address::ZERO`, xem lý do ô này/docs/STATE.md), `compute_deadline(buffer_sec)
  -> U256` (wall-clock `SystemTime::now()` + buffer, KHÔNG PHẢI
  `block.timestamp` on-chain thật — lý do kiến trúc ở docs/STATE.md),
  `apply_slippage(expected, slippage_bps) -> U256` (`expected*(10000-bps)/10000`,
  clamp + checked_mul, không panic), struct `PaperTxLog { label, to,
  calldata_hex, value_wei, gas_est_wei }`, `build_front_buy_paper_tx`/
  `build_back_sell_paper_tx` (gọi `calldata.rs::encode_front_buy`/
  `encode_back_sell` thật), `build_and_log_paper_sandwich(logger, cfg,
  victim_from, token, quote) -> Option<(PaperTxLog, PaperTxLog)>` — cổng
  `cfg.dry_run` (false → chỉ log `tx.build_skipped`, `return None`), log
  `tx.build` đủ `to`/`calldata`/`value_wei`/`gas_est_wei` cho cả 2 tx. 13 test
  mới (slippage x4, deadline x1, roundtrip decode front/back x2, build+log
  dry_run true/false x2, grep-toàn-repo "0 sendRaw" x1 — còn lại là số dư từ
  test cũ 7.1 không đổi).
- `src/pipeline.rs` — thêm `use crate::executor;` + hàm MỚI
  `decide_and_build_paper_v2(...)` bọc `decide_paper_v2` (GIỮ NGUYÊN, không
  sửa): khi `Simulated`, decode lại lấy `token` rồi gọi
  `executor::build_and_log_paper_sandwich`. Trả cùng kiểu
  `(PipelineOutcome, &'static str)`. Test config fixture
  (`test_config_toml`) thêm 2 dòng field mới. 2 test mới
  (`decide_and_build_paper_v2_logs_tx_build_when_simulated`/
  `..._no_tx_build_when_skipped`).
- `src/config.rs` — thêm 2 field bắt buộc `executor_deadline_buffer_sec: u64`
  (ship `120`)/`executor_slippage_bps: u32` (ship `50`). `base_toml()` test
  fixture cập nhật. 2 test mới (`missing_executor_fields_fail_load`,
  `executor_fields_load_ship_defaults`).
- `config.toml` — CHỈ THÊM 2 dòng field mới (không đổi field cũ):
  `executor_deadline_buffer_sec = 120`, `executor_slippage_bps = 50`, kèm
  comment giải thích cụm `7.3`.
- `docs/STATE.md` — thêm mục `7.3` (cuối file): lý do không đụng
  `main.rs`/`web.rs`, 2 field Config mới, quyết định wall-clock deadline,
  công thức slippage, quyết định placeholder `Address::ZERO`, cấu trúc
  `PaperTxLog`, `decide_and_build_paper_v2`, cổng `dry_run`, cách test "0
  sendRaw" hoạt động (kể cả lỗi tự-báo-mình lúc đầu và cách sửa), còn nợ.
- `docs/TASKS.md` — dòng `7.3` đổi từ "CHƯA LÀM" sang "MỘT PHẦN (paper-mode,
  BAOCAO16)" (KHÔNG ghi "XONG" — gửi tx thật vẫn chưa làm). Mục nợ `7.2` cập
  nhật thêm đoạn "phần nối vào pipeline thật ĐÃ ĐÓNG THÊM ở BAOCAO16".
- `baocao/BAOCAO16.md` — MỚI (file này).

KHÔNG ĐỤNG: `src/main.rs`, `src/web.rs`, `src/logger.rs`, `src/venues.rs`,
`src/decoder.rs`, `src/sim_v2.rs`, `src/calldata.rs` (API giữ NGUYÊN, không
sửa), `src/pairbook.rs`, `src/tax.rs`, `src/transport.rs`, `src/victims.rs`,
`src/state.rs`, `Cargo.toml` (không thêm crate/feature nào — `git diff --stat
Cargo.lock` rỗng, xem ô 5), `DEX_REGISTRY.md`, `CLAUDE.md`, `victims.txt`
thật, `.env`.

4. LỆNH CHẠY:
```
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -5
```

5. OUTPUT THẬT:
```
$ cargo test 2>&1 | tail -25
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
$ cargo build --release 2>&1 | tail -5
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `release` profile [optimized] target(s) in 20.98s
```

Đối chiếu số lượng test: BAOCAO15 có 159 passed. Phiên này thêm: `config.rs`
+2, `pipeline.rs` +2, `executor.rs` +10 (slippage x4, deadline x1, roundtrip
front/back x2, build+log dry_run true/false x2, grep "0 sendRaw" x1) = +14 →
159 + 14 = 173, khớp đúng số `cargo test` in ra ở trên. 0 `FAILED`, 2
`ignored` giữ nguyên (test RPC thật `#[ignore]` từ trước, không đổi phiên
này).

Test 7.3 riêng, lọc chạy để xác nhận:
```
$ cargo test 2>&1 | grep -E "^test (executor::tests|pipeline::tests::decide_and_build|config::tests::(missing_executor|executor_fields))"
test config::tests::executor_fields_load_ship_defaults ... ok
test config::tests::missing_executor_fields_fail_load ... ok
test executor::tests::apply_slippage_10000_bps_is_zero_no_panic ... ok
test executor::tests::apply_slippage_100_bps_is_1_percent_off ... ok
test executor::tests::apply_slippage_over_10000_bps_clamped_no_panic ... ok
test executor::tests::apply_slippage_zero_bps_keeps_full_amount ... ok
test executor::tests::build_back_sell_paper_tx_roundtrips_through_decoder ... ok
test executor::tests::compute_deadline_is_now_plus_buffer_within_tolerance ... ok
test executor::tests::build_front_buy_paper_tx_roundtrips_through_decoder ... ok
test executor::tests::load_signer_empty_env_var_is_err_no_panic ... ok
test executor::tests::load_signer_garbage_hex_is_err_no_panic ... ok
test executor::tests::load_signer_missing_env_var_is_err_no_panic ... ok
test executor::tests::load_signer_valid_hex_key_parses_ok ... ok
test executor::tests::dry_run_blocks_live_even_if_everything_else_is_green ... ok
test executor::tests::gate_check_zero_gas_cap_is_a_failure_reason ... ok
test executor::tests::gate_check_ok ... ok
test executor::tests::halt_lock_blocks_live ... ok
test executor::tests::gate_check_fail ... ok
test pipeline::tests::decide_and_build_paper_v2_no_tx_build_when_skipped ... ok
test executor::tests::build_and_log_paper_sandwich_skips_when_dry_run_false ... ok
test executor::tests::build_and_log_paper_sandwich_logs_tx_build_when_dry_run_true ... ok
test executor::tests::no_send_raw_transaction_call_anywhere_in_src ... ok
test pipeline::tests::decide_and_build_paper_v2_logs_tx_build_when_simulated ... ok
```

Bằng chứng dòng log `tx.build` THẬT (chạy `cargo test build_and_log_paper_sandwich_logs_tx_build_when_dry_run_true -- --nocapture`, `println!` giữ lại trong test làm bằng chứng, cùng quy ước `pipeline.rs` đã dùng ở BAOCAO04):
```
tx.build log THAT: {"back":{"calldata":"0x18cbafe500000000000000000000000000000000000000000000000270801d946c94000000000000000000000000000000000000000000000000000000c97dfdf46fb00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006aa83beb0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000cccccccccccccccccccccccccccccccccccccccc000000000000000000000000bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c","gas_est_wei":3000000000000000,"label":"back_sell","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","value_wei":"0"},"deadline":"1789410283","event":"tx.build","front":{"calldata":"0x7ff36ab50000000000000000000000000000000000000000000000026d60c1459a1d800000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006aa83beb0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000bb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c000000000000000000000000cccccccccccccccccccccccccccccccccccccccc","gas_est_wei":3000000000000000,"label":"front_buy","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","value_wei":"50000000000000000"},"self_address_placeholder":true,"token":"0xcccccccccccccccccccccccccccccccccccccccc","ts":"2026-09-14T18:22:43.562299+00:00","victim_from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
```
`to` cả 2 tx = `0x10ed43c718714eb63d5aa57b78b54704e256024e` — đúng V2 Router
đã pin (`DEX_REGISTRY.md`, chỉ khác viết hoa/thường, cùng địa chỉ). `deadline
1789410283` là timestamp Unix thật tại thời điểm chạy test + 120s buffer
(khớp `executor_deadline_buffer_sec` ship). `self_address_placeholder: true`
đúng thiết kế — không có địa chỉ ví thật nào bị log như thể là thật.

Xác nhận không thêm crate mới (chỉ dùng type/module đã pin sẵn trong `alloy`
từ `7.2`, không sửa `Cargo.toml`):
```
$ git diff --stat Cargo.lock
(rỗng — không có output)
```
`cargo build --release 2>&1 | grep -i warn` → rỗng (không có `warning` nào).

Sự cố nhỏ đã tự sửa trong phiên: lần chạy ĐẦU TIÊN của test
`no_send_raw_transaction_call_anywhere_in_src` tự báo LỖI vào CHÍNH dòng code
của nó (chuỗi tìm kiếm `"send_raw_transaction("` là literal tĩnh trong file
`executor.rs` — bản thân dòng chứa chuỗi tìm kiếm bị chính nó bắt được) — sửa
bằng cách ghép chuỗi tìm kiếm từ 2 mảnh tại runtime
(`format!("{}{}", "send_raw_transaction", "(")`) để không có literal liên tục
nào trong bất kỳ file `.rs` nào khớp chính nó; chạy lại xanh 173/173 (không
phải lỗi logic build/log, chỉ là hiệu ứng "test tự soi mình" — xem
docs/STATE.md mục `7.3`).

6. CHAIN: Không gọi RPC/on-chain nào mới phiên này (`compute_deadline` dùng
`SystemTime::now()`, không phải `eth_getBlockByNumber`; `apply_slippage`
thuần toán). `chain_id: 56` không đổi, không sửa `DEX_REGISTRY.md`/
`src/venues.rs`. Địa chỉ router dùng trong log (`V2_ROUTER_ADDRESS`) đối
chiếu lại DEX_REGISTRY.md bằng mắt: đúng `0x10ED43C718714eb63d5aA57B78B54704E256024E`
đã pin (khớp giá trị lowercase `0x10ed43c718714eb63d5aa57b78b54704e256024e`
trong log ô 5). `eth_getCode`: MISSING (không cần cho cụm này, không đụng
pin nào).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (không
nằm trong phạm vi ĐƯỢC ĐỤNG của lệnh này).

8. KHÔNG LÀM:
- Không `sendRaw`/bật `allow_live`/`bot_armed`/`dry_run=false` trong
  `config.toml` thật — chỉ THÊM 2 dòng field mới, mọi field cũ (kể cả
  `dry_run = true`) giữ nguyên y hệt (xem diff `config.toml` ô 3).
- Không đổi pair khỏi WBNB — `build_front_buy_paper_tx`/`build_back_sell_paper_tx`
  vẫn cố định 1 đầu path là `wbnb()` (hằng số `WBNB_ADDRESS` đã pin), y hệt
  `calldata.rs` gốc.
- Không hồi sinh chiều victim bán dưới bất kỳ hình thức nào — `executor.rs`/
  `decide_and_build_paper_v2` chỉ đọc `SandwichQuote`/`token` từ nhánh
  `Simulated` của `decide_paper_v2` (đã CHỈ xử lý chiều mua từ BAOCAO14),
  không thêm nhánh/hàm nào cho chiều bán.
- Không có bất kỳ hàm `sendRawTransaction`/tương đương nào — xác nhận bằng
  test grep toàn `src/` (`no_send_raw_transaction_call_anywhere_in_src`, ô5),
  không phải chỉ lời khẳng định suông.
- Không cần signer/private key thật — `build_and_log_paper_sandwich` không
  nhận tham số `Signer`/`B256` nào, chỉ dùng `PLACEHOLDER_SELF_ADDRESS`
  (`Address::ZERO`, đánh dấu rõ `self_address_placeholder: true` trong log).
- Không đổi `main.rs`/`web.rs` — 2 file này KHÔNG nằm trong `ĐƯỢC ĐỤNG` phiên
  này, nên `decide_and_build_paper_v2` CHƯA được gọi từ live loop
  (`handle_paper_tx` vẫn gọi `decide_paper_v2` cũ) — ghi rõ CÒN NỢ ở ô 10,
  không âm thầm bỏ qua.
- Không tách test/docs ra phiên sau — 15 test mới (2+2+13, xem ô 3) +
  `docs/STATE.md`/`docs/TASKS.md` đã cập nhật cùng phiên này.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `main.rs::handle_paper_tx` CHƯA đổi sang gọi `pipeline::decide_and_build_paper_v2`
  (main.rs không nằm trong ĐƯỢC ĐỤNG phiên này) — live loop pending-tx thật
  HIỆN TẠI vẫn gọi `decide_paper_v2` cũ, KHÔNG build/log `tx.build` nào, dù
  hàm mới đã sẵn sàng/test xanh. Việc của phiên sau: đổi 1 dòng gọi hàm ở
  `main.rs` (thêm tham số `&app_state.logger`), không đổi logic đọc kết quả
  phía sau.
- `to` (địa chỉ nhận) vẫn là placeholder `Address::ZERO` — `load_signer`
  (`7.1`) chỉ trả `B256` thô, không dẫn xuất được địa chỉ ví thật (cần
  `alloy-signer-local`/`k256`, chưa bật feature vì lý do version lệch đã ghi
  từ `7.1`/`5.2`). KHÔNG ảnh hưởng paper/log (không gửi đi), nhưng executor
  LIVE thật (`7.x` xa hơn) bắt buộc phải giải quyết trước khi có ý nghĩa.
- `deadline` dùng wall-clock (`SystemTime::now()`), KHÔNG PHẢI `block.timestamp`
  on-chain thật (lý do kiến trúc: hàm build tx thuần/sync, không có `Provider`
  — xem docs/STATE.md). Sai lệch nhỏ (~vài giây so buffer 120s), chấp nhận
  được cho paper-mode.
- Chỉ build calldata V2 Router (đúng phạm vi `7.2`) — V3/UR/V4 vẫn không có
  hàm build calldata nào.
- `RiskGuard::record_result` vẫn CHƯA gọi tự động ở đâu (nợ từ `7.1`, build/log
  paper không phải giao dịch thật nên không có kết quả lỗ/lãi thật để ghi).
- Gửi tx thật (`sendRawTransaction`) vẫn HOÀN TOÀN CHƯA LÀM — không có hàm
  ký/gửi nào trong repo, cần lệnh Grok riêng cho `7.3` thật (executor live).
