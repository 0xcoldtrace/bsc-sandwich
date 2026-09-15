1. LÁT: 2.1+2.2+2.3 — HTTP/WSS thật (alloy-provider) + decoder calldata router
Pancake đã pin (V2/V3/SmartRouter/Universal Router, path token/WBNB) + resolve
pool token/WBNB (V2/V3 qua eth_call thật, V4/Infinity = hooks_unread có lý do
kỹ thuật), một phiên đủ cả cụm, wire vào `main.rs` paper-safe.

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, DEX_REGISTRY.md, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO02.md.
Gói 1.x ĐẠT (Grok). Đừng làm lại getCode trừ address mới.

LÁT: 2.1+2.2+2.3 — HTTP/WSS + decoder path token/WBNB + resolve pool mọi family đã PIN.
stack = Rust + alloy. Không npm/viem.

ĐƯỢC ĐỤNG: Cargo.toml, src/* (transport, decoder, pair, main wire paper-safe), docs/STATE.md, docs/TASKS.md, baocao/BAOCAO03.md
CẤM: CLAUDE.md, .env (được ĐỌC BSC_HTTP/BSC_WS nếu chủ đã điền), cờ live, sendRaw, pair không-WBNB, đoán tick/hooks

2.1 Transport / 2.2 Decoder / 2.3 Resolve pool token/WBNB (nội dung đầy đủ
xem lệnh gốc — đã làm đủ cả 3, chi tiết ở mục 3-7 dưới).

KHÔNG LÀM: binary search sandwich, tax cache đầy đủ, live send, đổi scan_v4.
ĐẠT CẦN DÁN: cargo test ≥15 dòng cuối; nếu có RPC: chainId 0x38 + 1 getPair
hoặc getPool raw rút gọn; 1 fixture decode V2 dán struct parse (from/path/amountIn).
VIẾT: baocao/BAOCAO03.md đủ 10 ô. Chữ CHỜ GROK | FAIL | CHƯA XONG. Cấm ĐẠT.
```

3. FILE ĐỔI:
- `Cargo.toml` — bump `alloy-primitives` `"0.8"` -> `"1"` (bắt buộc, xem lý do
  ở mục 6/docs/STATE.md); thêm `alloy = "2"` (umbrella crate,
  `default-features=false`, features `std, provider-http, provider-ws,
  rpc-types-eth, reqwest-rustls-tls`) khoá thực tế trong `Cargo.lock` là
  `2.4.2`; thêm `url = "2"`.
- `Cargo.lock` — cập nhật theo dependency mới (alloy 2.4.2 + họ crate con,
  reqwest, tokio-tungstenite, v.v.).
- `src/transport.rs` (MỚI, cụm 2.1) — `connect_and_verify(url)` dùng
  `ProviderBuilder::new().connect(url)` (tự nhận diện http/https/ws/wss),
  xác nhận `eth_chainId==56` (sai chain -> `ConnectError::ChainMismatch`,
  không lẫn với "không kết nối được"); `redact_rpc_url` (chỉ giữ
  scheme+host+port, bỏ path/query trước khi log); `VpsFallback::load` đọc
  `rpc_http`/`rpc_ws` từ `vps.json`; `pick_url` ưu tiên env, fallback
  `vps.json`, `None` nếu cả hai rỗng. 8 test (không cần RPC sống) + 1 test
  `#[tokio::test]` xác nhận placeholder fail nhanh không panic.
- `src/decoder.rs` (MỚI, cụm 2.2) — `decode_swap_calldata(calldata, tx_value)`
  nhận diện 7 selector (suy bằng `keccak256(chữ_ký_hàm)`, không hardcode hex
  mù): `swapExactETHForTokens`, `swapExactTokensForETH`,
  `swapExactTokensForTokens` (V2 Router — dùng chung selector cho SmartRouter
  vì cùng chữ ký hàm khi có), `exactInputSingle`/`exactInput` (V3 SwapRouter,
  dùng chung cho SmartRouter cùng lý do), `execute(bytes,bytes[])`/
  `execute(bytes,bytes[],uint256)` (Universal Router, chỉ decode command
  `V2_SWAP_EXACT_IN`(0x08)/`V3_SWAP_EXACT_IN`(0x00), đúng 1 command/1 input
  mỗi tx). Path != 2 token hoặc không đầu nào là WBNB -> `not_wbnb_pair`.
  Selector lạ/calldata méo -> `decode_fail`. 12 test (bao gồm đối chiếu
  selector well-known, path 3-token, path 2-token không WBNB, multihop V3,
  UR command lạ).
- `src/pool.rs` (MỚI, cụm 2.3) — `resolve_v2_pair`/`resolve_v3_pool` gọi
  `eth_call` thật qua `Provider::call` (selector suy bằng keccak, không
  hardcode); `V3_FEE_TIERS=[100,500,2500,10000]` xác nhận nguồn
  `PancakeV3Factory.sol` (mục 6); `resolve_infinity_pool` trả `hooks_unread`
  cho mọi token phiên này (lý do kỹ thuật: Infinity dùng `PoolId=hash(PoolKey)`,
  không có `getPool(token0,token1)` — xem `docs/STATE.md`, nguồn
  `infinity-core/src/types/PoolKey.sol`). 10 test + 1
  `#[ignore]` test gọi RPC công khai thật (mục 5/6).
- `src/main.rs` — thêm `mod transport; mod decoder; mod pool;`; thêm
  `connect_rpc(app_state)` (gọi sau khi victim-reload task spawn, trước khi
  bind web server) nối `BSC_HTTP` (đồng bộ, cập nhật `last_block` qua
  `get_block_number()`) và `BSC_WS` (spawn task nền `subscribe_ws_heads`,
  loop `sub.recv()` cập nhật `last_block` + log `rpc.block`, lỗi/rớt kết nối
  chỉ log `rpc.skip` rồi dừng task, KHÔNG halt bot — đúng "Không halt vì WSS
  im"). Không có hàm gửi tx nào được thêm.
- `docs/STATE.md` — thêm mục "Version cụ thể (phiên 2.1+2.2+2.3)" (lý do bump
  alloy-primitives, dependency thêm, lý do dùng umbrella crate `alloy`), mục
  "Resolve pool V3 — fee tier" (nguồn `PancakeV3Factory.sol`), mục "Resolve
  pool V4/Infinity — vì sao hooks_unread" (nguồn `PoolKey.sol`), cập nhật
  "Không làm trong phiên này".
- `docs/TASKS.md` — đánh dấu `2.1/2.2/2.3` XONG (BAOCAO03) kèm phạm vi cụ
  thể, cập nhật mục Nợ/MISSING.
- Không đụng `DEX_REGISTRY.md`, `CLAUDE.md`, `.env`, cờ live, `victims.txt`
  thật.

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test decode_v2_swap_exact_eth_for_tokens_wbnb_to_token -- --nocapture
cargo test pool::tests::real_rpc_v2_get_pair_wbnb_usdt -- --ignored --nocapture
cargo run -- config.smoke.toml     # web_port=18789 tam thoi, xoa ngay sau
curl -s http://127.0.0.1:18789/api/health
curl -s http://127.0.0.1:18789/api/status
BSC_HTTP="https://bsc-dataseed.binance.org/" cargo run -- config.smoke.toml   # verify transport that (khong dung .env, chi env var 1 lan)
```

5. OUTPUT THẬT:

`cargo test` (51 passed, 1 ignored — ignored là test cần RPC sống, KHÔNG
nằm trong suite mặc định, đúng "test không bắt live node"; ≥15 dòng cuối):
```
running 52 tests
test decoder::tests::decode_too_short_calldata_is_decode_fail ... ok
test decoder::tests::decode_rejects_three_token_path_not_wbnb_pair ... ok
test decoder::tests::decode_rejects_two_token_path_without_wbnb ... ok
test decoder::tests::decode_v2_swap_exact_eth_for_tokens_wbnb_to_token ... ok
test decoder::tests::decode_v2_swap_exact_tokens_for_tokens_token_to_wbnb ... ok
test decoder::tests::decode_unknown_selector_is_decode_fail ... ok
test decoder::tests::decode_v3_exact_input_single_wbnb_pair ... ok
test decoder::tests::decode_universal_router_unknown_command_is_decode_fail ... ok
test decoder::tests::decode_universal_router_v2_swap_exact_in_wbnb_pair ... ok
test decoder::tests::well_known_selectors_match ... ok
test decoder::tests::decode_v3_exact_input_single_hop_wbnb_pair ... ok
test decoder::tests::decode_v3_exact_input_multihop_is_not_wbnb_pair ... ok
test pool::tests::decode_address_return_reads_last_20_bytes ... ok
test pool::tests::calldata_encodes_selector_then_two_address_words ... ok
test pool::tests::calldata_get_pool_encodes_fee_right_aligned_uint24 ... ok
test pool::tests::decode_address_return_too_short_is_none ... ok
test pool::tests::get_pair_selector_matches_well_known_value ... ok
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ignored
test pool::tests::get_pool_selector_matches_well_known_value ... ok
test pool::tests::infinity_pool_resolve_is_hooks_unread_this_session ... ok
test pool::tests::v3_fee_tiers_match_pancake_factory_constructor ... ok
test pool::tests::zero_address_pair_is_no_pool ... ok
test transport::tests::pick_url_prefers_env_over_vps_fallback ... ok
test transport::tests::placeholder_url_fails_fast_no_panic ... ok
test config::tests::load_ok ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
(… các test config/state/logger/victims/venues/executor cũ đều pass, không
lặp lại hết ở đây cho gọn)

test result: ok. 51 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Fixture decode V2 — struct parse thật (path/amountIn; KHÔNG có `from` vì đó
là field của tx envelope (`tx.from`, người ký gửi giao dịch), không nằm
trong calldata — decoder chỉ đọc calldata, `from` sẽ có khi wire vào pipeline
pending-tx thật ở `3.x/4.1`):
```
fixture decode struct: DecodedSwap { selector_name: "swapExactETHForTokens", amount_in: 10000000000000000, path: TwoTokenPath { token_a: 0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c, token_b: 0x111111111111111111111111111111111111beef }, to: 0x2222222222222222222222222222222222222222, deadline: Some(9999999999) } token_vs_wbnb=0x111111111111111111111111111111111111beef
test decoder::tests::decode_v2_swap_exact_eth_for_tokens_wbnb_to_token ... ok
```
(`token_a` = WBNB pinned đúng, `token_vs_wbnb` = token còn lại — path/amountIn
đúng struct decode thật, không bịa.)

RPC thật — dùng RPC công khai `https://bsc-dataseed.binance.org/` (giống
cách BAOCAO02 pin registry; KHÔNG phải `.env`/RPC runtime của bot, chỉ verify
code `2.1/2.3` một lần vì máy chưa có `.env`):

`connect_and_verify` thật qua `main.rs::connect_rpc` (chạy `cargo run` với
`BSC_HTTP` set qua biến môi trường, không sửa `.env`/`vps.json`):
```
$ curl -s http://127.0.0.1:18789/api/status
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,"halt_lock":false,
 "last_block":121834424,"live_gate":{...},"state":"WATCHING","uptime_sec":5}
$ cat logs/bot.jsonl
{"event":"rpc.connect","transport":"http","url":"https://bsc-dataseed.binance.org/***", ...}
```
(`last_block` lấy thật từ `provider.get_block_number()` sau khi
`eth_chainId` xác nhận `0x38` bên trong `connect_and_verify` — nếu sai chain
sẽ trả `ConnectError::ChainMismatch`, không log `rpc.connect`.)

`resolve_v2_pair` thật (`cargo test -- --ignored`, factory V2 pinned +
WBNB pinned, token = USDT BSC-USD `0x55d398...7955`, token nổi tiếng chỉ để
verify không phải victim bịa):
```
running 1 test
V2 getPair(USDT, WBNB) real pair = 0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.65s
```

Boot không có `.env` (mặc định, không set env nào) — `rpc.skip` cả 2
transport, boot vẫn chạy, web server vẫn trả `/api/health`:
```
{"event":"rpc.skip","reason":"rpc khong ket noi duoc: relative URL without a base","transport":"http","url":"***invalid_url***"}
{"event":"rpc.skip","reason":"rpc khong ket noi duoc: relative URL without a base","transport":"ws","url":"***invalid_url***"}
$ curl -s http://127.0.0.1:18789/api/health
{"status":"ok"}
```

6. CHAIN: `0x38` (56) xác nhận BÊN TRONG `connect_and_verify` (gọi
`eth_chainId` thật qua `provider.get_chain_id()` trước khi trả provider —
sai chain thì hàm trả lỗi, không log `rpc.connect`). Verify thực tế phiên
này qua RPC công khai `https://bsc-dataseed.binance.org/` (không phải
`.env`/RPC runtime — máy vẫn chưa có `.env`, `2.1` trong code đã sẵn sàng
nhận `.env` thật khi chủ điền):
- `eth_call getPair(USDT, WBNB)` trên `PancakeV2Factory` pinned
  (`0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`) -> pool thật
  `0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae` (khác address(0), không phải
  `no_pool`) — dán ở mục 5.
- `get_block_number()` qua HTTP provider thật -> `121834424` — dán ở mục 5.
- Nguồn xác nhận `V3_FEE_TIERS`/`PoolKey` (không phải `eth_getCode`, là đọc
  source Solidity trực tiếp phiên này qua `raw.githubusercontent.com`):
  `github.com/pancakeswap/pancake-v3-contracts/.../PancakeV3Factory.sol`
  (constructor `feeAmountTickSpacing[100/500/2500/10000]`) và
  `github.com/pancakeswap/infinity-core/src/types/PoolKey.sol`
  (struct `PoolKey{currency0,currency1,hooks,poolManager,fee,parameters}`).

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md` phiên này (không nằm trong ĐƯỢC
ĐỤNG). Venue vẫn khớp BAOCAO02: V2/V3/V4-Infinity PINNED, "Bản mới hơn"
DISABLED. `src/pool.rs`/`src/decoder.rs` dùng đúng address đã pin trong
`src/venues.rs` (WBNB, V2 Factory/Router, V3 Factory/SwapRouter/SmartRouter/
QuoterV2/UR-cũ, UR Infinity) — không thêm/bịa contract mới.

8. KHÔNG LÀM: không sim sandwich/binary-search front (`3.x`), không cache
tax roundtrip đầy đủ (`max_roundtrip_tax` chưa dùng tới ở cụm này), không
gửi tx thật (không có `send_raw_transaction`/hàm ký tx nào trong repo),
không đổi `dry_run/allow_live/bot_armed`/`scan_v4`, không sửa `.env`/
`victims.txt` thật, không sửa `CLAUDE.md`/`DEX_REGISTRY.md`, không decode
multicall Universal Router nhiều command/tx, không pin biến thể V2-style
4-tham-số riêng của SmartRouter (chưa xác nhận chữ ký chính xác — ghi
MISSING trong `docs/STATE.md`, không đoán), không tự suy `hooks`/
`parameters` cho pool V4/Infinity của token bất kỳ (trả `hooks_unread` thay
vì bịa `PoolKey`), không wire decoder/pool vào vòng lặp watch-mempool thật
(thuộc `3.x/4.1`, chỉ có module thuần + test phiên này).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `3.1/3.2/3.3` — math V2 (search front) + victim-still-ok + sim V3/V4 quoter
  đã pin; nối `decoder`+`pool`+`transport` thành 1 pipeline watch-mempool
  thật (`4.1`) — cần WSS pending-tx subscription (`eth_subscribe
  newPendingTransactions` hoặc tương đương), CHƯA làm phiên này (transport.rs
  mới có `subscribe_blocks`, chưa có pending-tx).
- SmartRouter V2-style (4 tham số, không `deadline`) — cần tìm đúng chữ ký
  hàm nguồn (docs/ABI PancakeSwap SmartRouter) trước khi thêm selector, hiện
  tại calldata dạng đó rơi `decode_fail` (không bịa).
- V4/Infinity resolve pool thật (khác `hooks_unread` mặc định) — cần pin
  cách tìm `PoolKey` cho 1 token cụ thể (quét event `Initialize` trên
  `CLPoolManager`/`BinPoolManager` hoặc dùng subgraph/indexer chính thức) —
  chưa có nguồn pin trong phiên này, ghi rõ trong `docs/STATE.md`.
- `.env` thật (`BSC_HTTP`/`BSC_WS`) vẫn chưa được chủ điền — code `2.1` đã
  sẵn sàng nhận, verify bằng RPC công khai một lần (mục 5/6), không phải RPC
  runtime lâu dài của bot.
- Universal Router multicall (nhiều command/input trong 1 `execute()`) —
  ngoài phạm vi phiên này, đang `decode_fail` có chủ đích, không đoán thứ tự.
