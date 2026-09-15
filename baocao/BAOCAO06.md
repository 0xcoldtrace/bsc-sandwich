1. LÁT: 5.1 — paper sống: subscribe `newPendingTransactions` (thật) +
`state/inject_tx.jsonl` (test khi pending trống) + wire `decide_paper` ->
`log_outcome` -> `skip_counts`/`hits` số thật vào `main.rs`/`web.rs`,
concurrency ≤ 4.

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO05.md, src/main.rs, src/transport.rs, src/pipeline.rs
Gói config ĐẠT (Grok). 5.1 CHƯA LÀM — làm phiên này.

LÁT: 5.1 paper sống — pending (hoặc inject) + wire decide_paper + log + skip_counts + web.
stack = Rust + alloy.

ĐƯỢC ĐỤNG: src/transport.rs, src/main.rs, src/pipeline.rs, src/web.rs, src/state.rs nếu cần, docs/*, baocao/BAOCAO06.md
CẤM: CLAUDE.md, .env (được ĐỌC), sendRaw, bật live, bịa hash pending

LÀM:
1) transport: subscribe newPendingTransactions; node không đẩy pending → log rpc.pending_unavailable, bot không halt.
2) Paper loop (dry_run only):
   lấy from + input + value
   Config + VictimBook hiện tại (RwLock, sau reload)
   decide_paper → log_outcome → tăng skip_counts / hits
   concurrency ≤ 4
3) Inject: đọc state/inject_tx.jsonl (mỗi dòng from,value_wei,input_hex) để test khi pending trống.
4) web /api/skips /api/hits số thật.
5) Test: inject ví example + calldata V2 → reason khớp (below_min / honeypot_or_tax / Simulated).
   cargo test xanh.
6) Nếu có BSC_WS: chạy ≤60s, dán jsonl (0 hit cũng được, ghi 0).

KHÔNG LÀM: 7.x, probe tax contract, Infinity PoolKey.

ĐẠT CẦN DÁN: cargo test ≥15 dòng; 1 inject reason; nếu watch: số pending + skip (kể cả 0).

VIẾT: baocao/BAOCAO06.md đủ 10 ô. CHỜ GROK. Cấm ĐẠT.
Ô 1: 5.1
```

3. FILE ĐỔI:
- `src/transport.rs` — thêm `PendingTxRaw { from, value, input }` (dữ liệu
  thô tối thiểu cho `decide_paper`, tách khỏi kiểu `alloy::rpc::types::eth::Transaction`
  cụ thể). Thêm `pending_tx_from_rpc<T>(tx: &T) -> PendingTxRaw` generic theo
  2 trait (`alloy::network::TransactionResponse` cho `.from()`,
  `alloy::consensus::Transaction` cho `.value()`/`.input()`) — không cần sửa
  `Cargo.toml` (feature `providers` đã bật sẵn `consensus`+`network`+`pubsub`
  qua `provider-ws` từ trước). Thêm `parse_inject_line(line) -> Result<PendingTxRaw, String>`
  parse `from,value_wei,input_hex` (input_hex có/không `0x` đều được, tự
  strip trước khi `Bytes::from_str` vì `hex::decode` bên trong không tự strip).
  7 test mới cho `parse_inject_line` (2 case hợp lệ + 5 case lỗi định dạng).
- `src/pipeline.rs` — tách `decode_and_prefilter` (private, không đổi hành vi
  `decide_paper` — MỌI test cũ pass nguyên, 0 assertion nào sửa) dùng chung
  bởi `decide_paper` VÀ hàm mới `precheck_without_reserves` (pub — gọi TRƯỚC
  khi tốn RPC, loại tx rõ ràng không phải candidate: decode_fail/
  not_wbnb_pair/not_in_list/below_min). Thêm `resolve_v2_reserves(provider,
  token) -> Result<PoolReserves, PipelineSkip>` (async, gọi thật
  `pool::resolve_v2_pair` + `pool::get_reserves_vs_wbnb` qua `eth_call`, gộp
  mọi lỗi thành `PipelineSkip::NoPool`). Thêm variant `PipelineSkip::NoPool`
  ("no_pool" — tên đã có sẵn trong `SKIP_REASONS`/CLAUDE.md từ trước nhưng
  chưa có nhánh nào sinh ra tới phiên này, giống khuôn `ThinLiq` ở phiên
  config-hot-reload).
- `src/web.rs` — `AppStateInner` thêm 3 field: `provider: RwLock<Option<DynProvider>>`
  (dùng lại provider HTTP đã `connect_and_verify`), `tax_cache: RwLock<TaxCache>`
  (dùng chung toàn bot, CHỈ điền thủ công — giữ nguyên quyết định phiên `3.3`,
  live loop KHÔNG tự động gọi `measure_roundtrip_via_router`), `pending_semaphore:
  Arc<Semaphore>` (giới hạn 4 tx paper đồng thời).
- `src/main.rs` — `connect_rpc` lưu provider HTTP vào `app_state.provider`
  sau khi verify xong; luôn `tokio::spawn(subscribe_pending_txs(...))` sau
  đó (dùng `ws_url` nếu có, không thì thử trên provider HTTP đã lưu). Thêm 3
  hàm nền: `subscribe_pending_txs` (gọi `Provider::subscribe_full_pending_transactions`,
  lỗi/không hỗ trợ -> log `rpc.pending_unavailable`, KHÔNG halt, giống hệt
  khuôn `subscribe_ws_heads` đã có từ `2.1`), `watch_inject_file` (poll
  `state/inject_tx.jsonl` mỗi 2s, chỉ đọc dòng mới, file ngắn hơn thì đọc lại
  từ đầu, không panic), `handle_paper_tx` (giữ 1 permit `Semaphore(4)` suốt
  vòng đời qua RAII, đọc `cfg.dry_run` — return sớm nếu `false` đúng "Paper
  loop dry_run only" — rồi `precheck_without_reserves` -> `resolve_v2_reserves`
  -> `decide_paper` -> `log_outcome` -> tăng `skip_counts` nhánh `Skip`).
  `tx.seen` log ở cả 2 nguồn (pending_ws/inject) trước khi xử lý.
- `docs/STATE.md` — thêm mục "`5.1` — Paper sống" (chi tiết kiến trúc + kết
  quả verify runtime thật bằng RPC công khai từ `.env` chủ, giải thích vì
  sao vẫn cần resolve reserve RPC thật dù lệnh không nói rõ, vì sao
  `Simulated` gần như không xảy ra trong live loop hiện tại).
- `docs/TASKS.md` — `5.1` chuyển XONG (kèm 3 giới hạn còn nợ), cập nhật mục
  Nợ tương ứng.
- KHÔNG đụng: `victims.txt` thật, `.env` (chỉ ĐỌC, không sửa), `CLAUDE.md`,
  `DEX_REGISTRY.md`, `src/decoder.rs`, `src/executor.rs` (không viết
  `send_raw_transaction`, không bật cờ live nào), `src/state.rs` (không cần
  sửa — state machine `Hit/SimLock/Logged` KHÔNG được wire vào live loop
  phiên này để tránh race giữa các tx xử lý đồng thời, ghi CÒN NỢ ô 10).

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test transport::tests::parse_inject_line -- --nocapture
```
Runtime smoke test THẬT (dùng `.env` thật của chủ — chỉ ĐỌC biến môi trường
`BSC_HTTP`, KHÔNG sửa file `.env`/`config.toml`/`victims.txt` thật; config +
victims dùng bản scratch NGOÀI repo, port khác `8787`; `state/`/`logs/` là
thư mục gitignored thật của repo, đúng tiền lệ BAOCAO05):
```
export $(grep -m1 '^BSC_HTTP=' .env)
target/debug/bsc_sandwich.exe <scratch_config.toml, web_port=18790> &
curl http://127.0.0.1:18790/api/status
echo "0x<victim>,50000000000000000,0x7ff36ab5...<path WBNB,USDT>" > state/inject_tx.jsonl
sleep 5
curl http://127.0.0.1:18790/api/skips
curl "http://127.0.0.1:18790/api/hits?limit=10"
```

5. OUTPUT THẬT:

`cargo test` (97 passed, 2 ignored — 2 ignored cần RPC sống như cũ, +7 test
mới so BAOCAO05, không giảm test nào; ≥15 dòng cuối thật):
```
running 99 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test decoder::tests::decode_too_short_calldata_is_decode_fail ... ok
test decoder::tests::decode_captures_nonzero_amount_out_min_swap_exact_eth_for_tokens ... ok
test pipeline::tests::honeypot_or_tax_skip_when_measured_tax_exceeds_max_roundtrip_tax_config ... ok
test pipeline::tests::thin_liq_skip_when_pool_reserve_below_min_reserve_config ... ok
test pipeline::tests::victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured ... ok
test pipeline::tests::victim_b_below_min_is_skipped_with_clear_reason ... ok
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ignored
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ignored
test transport::tests::parse_inject_line_rejects_bad_address ... ok
test transport::tests::parse_inject_line_ignores_leading_trailing_whitespace ... ok
test transport::tests::parse_inject_line_rejects_bad_hex ... ok
test transport::tests::parse_inject_line_rejects_wrong_field_count ... ok
test transport::tests::parse_inject_line_rejects_bad_value ... ok
test transport::tests::parse_inject_line_valid_with_0x_prefix ... ok
test transport::tests::parse_inject_line_valid_without_0x_prefix ... ok
test pipeline::tests::unprofitable_skip_when_profit_positive_but_below_min_profit_bnb_config ... ok
test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero ... ok
test pipeline::tests::changing_config_between_two_decide_paper_calls_changes_front_cap ... ok
(... config/decoder/executor/logger/pool/sim_v2/sim_v3/state/tax/transport/venues/victims đều pass, không lặp lại hết cho gọn)

test result: ok. 97 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test transport::tests::parse_inject_line -- --nocapture` (7 test mới
riêng cho parser inject):
```
running 7 tests
test transport::tests::parse_inject_line_rejects_bad_address ... ok
test transport::tests::parse_inject_line_rejects_wrong_field_count ... ok
test transport::tests::parse_inject_line_rejects_bad_hex ... ok
test transport::tests::parse_inject_line_ignores_leading_trailing_whitespace ... ok
test transport::tests::parse_inject_line_valid_without_0x_prefix ... ok
test transport::tests::parse_inject_line_valid_with_0x_prefix ... ok
test transport::tests::parse_inject_line_rejects_bad_value ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 92 filtered out; finished in 0.00s
```

Runtime THẬT (binary chạy, KHÔNG phải unit test — dùng `BSC_HTTP` thật từ
`.env` chủ, port scratch `18790`, không đụng file thật của repo):
```
GET /api/status (ngay sau boot):
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":false,"last_block":121844816,
 "live_gate":{"allow_live":false,"bot_armed":false,"chain_id_56":true,
   "not_dry_run":false,"not_halted":true},
 "state":"WATCHING","uptime_sec":2}
```
`last_block=121844816` là block BSC THẬT lấy qua `eth_blockNumber` công khai
(`BSC_HTTP` chủ điền, `bsc-dataseed1.bnbchain.org`), không bịa.

`logs/bot.jsonl` (rút gọn, đủ chứng minh pending-subscribe THẬT được gọi):
```
{"chain_id":56,"dry_run":true,"event":"bot.start", ...}
{"count":1,"error_lines":0,"event":"victim.reload", ...}
{"event":"rpc.connect","transport":"http","url":"https://bsc-dataseed1.bnbchain.org/***", ...}
{"event":"rpc.skip","reason":"rpc khong ket noi duoc: relative URL without a base","transport":"ws", ...}
{"event":"rpc.pending_unavailable","reason":"rpc khong ket noi duoc: relative URL without a base","transport":"ws", ...}
```
`rpc.pending_unavailable` ở đây là kết quả THẬT — `BSC_WS` rỗng trong `.env`
chủ nên `pick_url` fallback về placeholder `REPLACE_ME_WSS_RPC_URL` của
`vps.json` (hành vi cũ từ `2.1`), `subscribe_pending_txs` thử connect WS đó
và fail đúng như thiết kế "node không đẩy pending -> log rpc.pending_unavailable,
không halt" (bot vẫn WATCHING, web vẫn phục vụ bình thường, xem `/api/health`
tiếp tục `{"status":"ok"}` sau đó).

Sau khi ghi 1 dòng vào `state/inject_tx.jsonl`
(`0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,50000000000000000,0x7ff36ab5...`
— calldata `swapExactETHForTokens` path `[WBNB thật, USDT thật]`, amount 0.05
BNB, victim scratch có min 0.01):
```
{"event":"tx.seen","from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","source":"inject", ...}
{"event":"tx.skip","from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","reason":"honeypot_or_tax","token":null, ...}
```
`honeypot_or_tax` chứng minh `resolve_v2_reserves` đã gọi `eth_call` THẬT
thành công (pool WBNB/USDT thật trên mainnet — nếu resolve thất bại kết quả
đã là `no_pool`, không phải `honeypot_or_tax`) rồi mới dừng ở tax cache rỗng
(đúng thiết kế: live loop không tự điền `tax_cache`).

Ghi thêm 1 dòng amount 0.005 BNB (dưới min 0.01):
```
{"event":"tx.seen","from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","source":"inject", ...}
{"event":"tx.skip","from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","reason":"below_min","token":null, ...}
```
`GET /api/skips` sau 2 lần trên (số THẬT từ `skip_counts`, không bịa):
```
{"below_min":1,"deadline":0,"decode_fail":0,"honeypot_or_tax":1,
 "hooks_unread":0,"no_pool":0,"not_in_list":0,"not_wbnb_pair":0,
 "thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
```
`GET /api/victims`/`GET /api/venues` vẫn trả dữ liệu bình thường trong lúc
loop chạy (không crash, không panic — `stdout.log` chỉ có 2 dòng boot, không
có backtrace nào). Đã kill process sau khi verify, xoá `state/inject_tx.jsonl`
test khỏi repo (file gitignored, không vào git).

6. CHAIN: `0x38` xác nhận qua `eth_chainId` thật lúc `connect_and_verify`
(log `rpc.connect` ở trên) + `eth_blockNumber` thật (`last_block=121844816`).
`eth_call` THẬT 2 lần cho mỗi lần `resolve_v2_reserves` chạy tới nơi
(`getPair(USDT,WBNB)` qua `V2_FACTORY_ADDRESS` đã pin, rồi `token0()` +
`getReserves()` qua pair trả về) — không dán raw hex response (không lưu lại
lúc chạy), nhưng kết quả gián tiếp (`honeypot_or_tax` thay vì `no_pool`/
`thin_liq`) đã chứng minh cả 2 `eth_call` thành công và `reserve_wbnb` > 20
WBNB ship. `eth_subscribe newPendingTransactions` được GỌI thật (không bịa)
nhưng THẤT BẠI ở bước transport (WS placeholder không parse được) — chưa có
lần nào node BSC thật xác nhận CÓ/KHÔNG hỗ trợ đẩy pending, vì máy phiên này
không có `BSC_WS` thật trong `.env` (chủ chưa điền, đúng ghi chú trong
`.env`: "chưa có WSS thật để điền"). Ghi rõ ở ô 10, không bịa "đã test pending
thật thành công".

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không pin
address mới). `resolve_v2_reserves` chỉ TÁI SỬ DỤNG `V2_FACTORY_ADDRESS` đã
pin từ BAOCAO02, không thêm contract nào.

8. KHÔNG LÀM: không có `7.x`/executor gửi tx thật (`send_raw_transaction`
vẫn không tồn tại trong repo); không probe tax contract (giữ nguyên
`tax_cache` chỉ điền thủ công, quyết định phiên `3.3`); không đụng V4/Infinity
`PoolKey` (vẫn `hooks_unread` mọi token, không đổi `pool.rs`); không bật
`allow_live`/`dry_run=false`/`bot_armed` nào; không sửa `.env`/`victims.txt`
thật/`CLAUDE.md`/`DEX_REGISTRY.md`; không wire `BotState::Hit/SimLock/Logged`
vào live loop (tránh race giữa các tx xử lý đồng thời qua `Semaphore(4)`,
ghi CÒN NỢ nếu chủ cần theo dõi state machine chi tiết hơn qua `/api/status`).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- Pending THẬT qua WSS chưa được node BSC thật xác nhận CÓ đẩy dữ liệu hay
  không — `.env` chủ chưa điền `BSC_WS` (ghi chú trong `.env`: "chưa có WSS
  thật"). Khi chủ điền `BSC_WS` thật, `subscribe_pending_txs` sẽ tự dùng —
  không cần sửa code, nhưng CẦN 1 lần chạy thật để xác nhận node đó có hỗ
  trợ `eth_subscribe newPendingTransactions` hay không (nhiều node công khai
  tắt tính năng này vì tốn tài nguyên).
- Live loop không tự điền `tax_cache` (đúng quyết định `3.3`) nên trong sản
  xuất thật, MỌI candidate qua được `thin_liq` sẽ dừng ở `honeypot_or_tax` —
  chưa có cách nào (API/cờ config) để chủ tự điền cache lúc bot đang chạy
  ngoài sửa code/test. Cần cụm riêng nếu muốn thấy `Simulated` thật trong
  live loop mà không cần đo tax thật (rủi ro: điền tay có thể sai, chỉ nên
  làm khi chủ hiểu rõ đang override an toàn mặc định).
- `resolve_v2_reserves` gộp "không có pool" và "lỗi RPC" thành `no_pool` —
  chưa có log riêng lỗi RPC cụ thể (vd rate limit, timeout) ở bước resolve
  reserve trong live loop; muốn chẩn đoán sâu hơn khi thấy `no_pool` bất
  thường cần thêm log chi tiết (chưa làm).
- `BotState` vẫn đứng yên ở `WATCHING` suốt vòng đời (không chuyển
  `HIT/SIM_LOCK/LOGGED` khi có `Simulated`) — cố ý bỏ qua phiên này để tránh
  race giữa nhiều tx xử lý đồng thời (`Semaphore(4)`) ghi đè `bot_state`
  lẫn nhau; cần thiết kế riêng (vd state theo từng tx thay vì global) nếu
  chủ muốn dashboard phản ánh đúng state machine CLAUDE.md khi có nhiều tx
  cùng lúc.
- `max_exposure_bnb`/`max_consecutive_loss`/`gas_reserve_bnb_wei` vẫn CHƯA
  có logic tiêu thụ (kế thừa từ phiên config-hot-reload, không đổi ở đây).
