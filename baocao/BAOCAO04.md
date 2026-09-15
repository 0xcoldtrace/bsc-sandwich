1. LÁT: 3.1+3.2+3.3+4.1 — V2 sandwich math (`getAmountOut` 9975/10000,
binary/ternary search `front_in` ≤ `max_front_bnb`) + victim-still-ok +
sim V3 qua `QuoterV2` đã pin (thật, `eth_call` sống) + tax stub (cache +
hàm `eth_call` thật, giới hạn kỹ thuật đã ghi rõ) + nối thành pipeline
paper `decide_paper` (lõi thuần, test fixture, chưa wire pending-tx thật).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, DEX_REGISTRY.md, docs/STATE.md, baocao/BAOCAO03.md.
Gói 2.x ĐẠT (Grok). Đừng làm lại transport/decoder trừ bug chặn sim.

LÁT: 3.1+3.2+3.3 — math V2 + victim still ok + sim V3 quoter (Infinity skip hooks_unread).
Được làm nốt 4.1 (nối decode+pool+sim 1 tx giấy) nếu thuận tiện cùng phiên.
stack = Rust + alloy. Không npm.

ĐƯỢC ĐỤNG: src/ (sim, tax stub, pipeline paper), Cargo.toml nếu cần, docs/STATE.md, docs/TASKS.md, web API hits/skips nếu pipeline ghi log, baocao/BAOCAO04.md
CẤM: CLAUDE.md, .env, live send, đổi cờ, bịa quote Infinity, pair không-WBNB

[nội dung đầy đủ 3.1/3.2/3.3/Tax stub/4.1 — xem lệnh gốc]

KHÔNG LÀM: 7.x, WSS pending bắt buộc, sửa CLAUDE.md.
ĐẠT CẦN DÁN: cargo test ≥15 dòng cuối; 1 fixture V2 số tay amountOut;
nếu có RPC: 1 quote V3 hoặc ghi MISSING; nếu làm 4.1: 2 case min-size + reason.
VIẾT: baocao/BAOCAO04.md đủ 10 ô. Chữ CHỜ GROK | FAIL | CHƯA XONG. Cấm ĐẠT.
```

3. FILE ĐỔI:
- `src/sim_v2.rs` (MỚI) — `get_amount_out` (U256, `9975/10000`, floor
  division); `PoolReserves`; `simulate_front_then_victim` (áp front rồi
  victim nối tiếp lên cùng pool state); `search_max_front_in` (ternary
  search số nguyên trên `[0, max_front_wei]`, LUÔN kèm `max_front_wei` +
  2 đầu khoảng cuối trong tập ứng viên nên kết quả không bao giờ vượt trần —
  "All-in bị chặn max_front"); `victim_still_ok` (so `amountOut` thật vs
  `amountOutMin` của CHÍNH victim, không phải `min_swap_bnb`). Gas dùng
  thẳng `front_max_gas_bnb_wei + back_max_gas_bnb_wei` từ config (không gọi
  `eth_gasPrice` on-chain, đúng CLAUDE.md). `profit_wei: i128` (có thể âm),
  không phải U256 — lý do ghi trong doc-comment đầu file. 8 test, gồm 1
  fixture tay số khớp tuyệt đối (`get_amount_out_exact_fixture_reserves`,
  không float) + 1 fixture chuỗi đầy đủ front→victim→back
  (`hand_verified_full_sandwich_numbers`) + 1 test "all-in bị chặn
  max_front" (`search_never_exceeds_max_front_bnb`, dựng pool nông cố ý để
  chứng minh tối ưu THẬT bị chặn đúng tại trần, không phải trùng hợp).
- `src/decoder.rs` — thêm field `amount_out_min: U256` vào `DecodedSwap`
  (BUG CHẶN SIM: không có field này thì `3.2` không so được
  `victim_would_revert` — đúng ngoại lệ "đừng làm lại transport/decoder trừ
  bug chặn sim" trong lệnh). Đọc thêm đúng 1 word/nhánh ở index đã biết sẵn
  layout từ BAOCAO03 (không đổi cấu trúc parse hiện có):
  `swapExactETHForTokens` idx0, `swapExactTokensForETH/Tokens` idx1,
  `exactInputSingle` idx6 (`amountOutMinimum`), `exactInput` idx4, UR
  V2/V3 `SwapExactIn` idx2. Thêm 1 test fixture riêng
  (`decode_captures_nonzero_amount_out_min_swap_exact_eth_for_tokens`) với
  giá trị != 0 để chứng minh đọc đúng index, không phải trùng hợp mặc định
  0 của các fixture cũ.
- `src/pool.rs` — thêm `get_reserves_vs_wbnb` (`eth_call token0()` +
  `eth_call getReserves()`, sắp đúng chiều WBNB/token — không đoán theo so
  sánh address, gọi `token0()` thật) + `decode_reserves_return`. Cần cho
  `3.1` chạy được với pool thật (chỉ có địa chỉ pool từ `2.3` chưa đủ, thiếu
  reserve). 4 test mới (2 selector well-known + 2 decode).
- `src/sim_v3.rs` (MỚI) — `build_quote_calldata`/`decode_quote_amount_out`/
  `quote_exact_input_single` gọi `QuoterV2.quoteExactInputSingle` đã pin
  (`venues::V3_QUOTER_V2_ADDRESS`). Struct `QuoteExactInputSingleParams`
  xác nhận qua `curl` RAW source `IQuoterV2.sol` (phát hiện + sửa 1 lần
  `WebFetch` tóm tắt AI cho SAI thứ tự field — ghi chi tiết ở
  `docs/STATE.md` mục "V3 quoter"). Chỉ single-hop (khớp decoder). 5 test
  (calldata layout, decode return, in selector thật để làm bằng chứng) + 1
  `#[ignore]` gọi RPC công khai thật.
- `src/tax.rs` (MỚI, "Tax stub") — `TaxCache`/`TaxMeasurement` (cache +
  staleness theo `tax_cache_blocks`, đúng/đầy đủ) +
  `measure_roundtrip_via_router` (2 `eth_call` THẬT `getAmountsOut` mua rồi
  bán qua V2 Router đã pin) — nhưng đã CHỨNG MINH (không phải giả định,
  xem doc-comment đầu file + `docs/STATE.md`) hàm này KHÔNG phát hiện được
  fee-on-transfer tax thật (router không đọc lại `balanceOf` sau transfer).
  `honeypot_or_tax` vẫn mặc định khi cache trống/hết hạn — đúng luật. 8 test
  (cache hit/miss/stale, decode/build calldata, loss_bps nguyên không float).
- `src/pipeline.rs` (MỚI, `4.1`) — `decide_paper` (lõi thuần, sync, không
  cần RPC — test được ngay bằng fixture reserve+tx, đúng quy ước repo) nối
  decode → path WBNB → victims.txt (`not_in_list`/`below_min`) → tax cache
  (`honeypot_or_tax`) → `sim_v2` (`victim_would_revert`/`unprofitable`) →
  `Simulated`. Chỉ xử lý chiều victim MUA token bằng WBNB
  (`path.token_a==WBNB`) — chiều bán ghi CÒN NỢ (docs/STATE.md). Thêm
  `log_outcome` ghi `tx.skip`/`sim.result` vào `logs/bot.jsonl` (chưa wire
  vào vòng lặp pending-tx thật — không tồn tại ở phiên này). 6 test: 2 case
  min-size (`victim_b_below_min...` = `below_min`,
  `victim_a_passes_min_size_but_skips_honeypot_or_tax...` = `honeypot_or_tax`
  với reason rõ), 1 test inject cache tax=0 để A ra `Simulated`
  (`victim_a_reaches_sim_when_tax_cache_injected_zero`), 1 test `not_in_list`,
  1 test `log_outcome`.
- `src/venues.rs` — thêm `pub const` cho `V2_FACTORY_ADDRESS`/
  `V2_ROUTER_ADDRESS`/`V3_FACTORY_ADDRESS`/`V3_QUOTER_V2_ADDRESS` (address
  ĐÃ PIN sẵn từ `1.1/1.2`, chỉ khai báo hằng dùng chung để `pool.rs`/
  `sim_v3.rs`/test không lặp lại literal string 2 nơi — `registry_snapshot`
  cập nhật tham chiếu lại đúng hằng này, KHÔNG đổi giá trị).
- `src/main.rs` — thêm `mod pipeline; mod sim_v2; mod sim_v3; mod tax;`.
  Không đổi logic runtime nào khác (không wire pending-tx, không đổi
  `connect_rpc`).
- `docs/STATE.md` — thêm mục "V2 sandwich math", "V3 quoter — phát hiện
  lệch field order giữa 2 nguồn" (bài học `curl` raw vs `WebFetch` tóm tắt
  AI), "Tax stub — giới hạn kỹ thuật đã xác nhận", "Pipeline paper 4.1 —
  phạm vi".
- `docs/TASKS.md` — đánh dấu `3.1/3.2/3.3/4.1` XONG kèm phạm vi, cập nhật
  Nợ/MISSING.
- Không đụng `DEX_REGISTRY.md` (không pin address mới), `CLAUDE.md`, `.env`,
  cờ live, `victims.txt` thật. Không sửa `web.rs`/`state.rs` (pipeline chưa
  wire vào web/main loop nên `/api/hits`/`/api/skips` vẫn đọc `logs/bot.jsonl`
  như cũ, không cần đổi route).

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test sim_v3::tests::print_computed_selector_for_evidence -- --nocapture
cargo test pool::tests::real_rpc_v2_get_pair_wbnb_usdt -- --ignored --nocapture
cargo test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt -- --ignored --nocapture
cargo test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero -- --nocapture
```
(2 lệnh `--ignored` dùng RPC công khai `https://bsc-dataseed.binance.org/`
giống quy ước `pool.rs`/BAOCAO03 — KHÔNG đọc `.env`, không phải RPC runtime
của bot. `.env`/`BSC_HTTP` thật đã được chủ điền phiên trước nhưng phiên
này không cần tới vì không chạy `cargo run`.)

5. OUTPUT THẬT:

`cargo test` (78 passed, 2 ignored — 2 ignored là test cần RPC sống, KHÔNG
nằm trong suite mặc định; đủ ≥15 dòng cuối):
```
running 80 tests
test decoder::tests::decode_too_short_calldata_is_decode_fail ... ok
test decoder::tests::decode_rejects_three_token_path_not_wbnb_pair ... ok
test decoder::tests::decode_captures_nonzero_amount_out_min_swap_exact_eth_for_tokens ... ok
test decoder::tests::decode_v2_swap_exact_eth_for_tokens_wbnb_to_token ... ok
test decoder::tests::well_known_selectors_match ... ok
test pipeline::tests::address_not_in_victims_txt_is_not_in_list ... ok
test pipeline::tests::victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured ... ok
test pipeline::tests::victim_b_below_min_is_skipped_with_clear_reason ... ok
test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero ... ok
test pipeline::tests::log_outcome_writes_expected_events ... ok
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ignored
test pool::tests::get_reserves_selector_matches_well_known_value ... ok
test pool::tests::token0_selector_matches_well_known_value ... ok
test sim_v2::tests::get_amount_out_exact_fixture_reserves ... ok
test sim_v2::tests::get_amount_out_zero_input_or_reserve_is_none ... ok
test sim_v2::tests::hand_verified_full_sandwich_numbers ... ok
test sim_v2::tests::victim_still_ok_pass_and_revert ... ok
test sim_v2::tests::search_never_exceeds_max_front_bnb ... ok
test sim_v2::tests::search_result_front_in_always_within_bounds_even_with_large_max ... ok
test sim_v3::tests::decode_quote_amount_out_reads_first_word ... ok
test sim_v3::tests::calldata_layout_selector_then_5_static_words_no_offset ... ok
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ignored
test sim_v3::tests::print_computed_selector_for_evidence ... ok
test tax::tests::cache_hit_within_window_then_stale_after_tax_cache_blocks ... ok
test tax::tests::build_calldata_has_correct_selector_and_length ... ok
test tax::tests::loss_bps_exact_integer_math ... ok
(… config/pool/decoder/executor/state/transport/venues/victims/logger đều
pass, không lặp lại hết cho gọn — tổng 78 passed)

test result: ok. 78 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Fixture V2 số tay (`get_amount_out_exact_fixture_reserves`, reserveIn=
reserveOut=1000, amountIn=100):
```
amountInWithFee = 100*9975 = 997_500
numerator = 997_500*1000 = 997_500_000
denominator = 1000*10000 + 997_500 = 10_997_500
amountOut = 997_500_000 / 10_997_500 = 90 (floor, dư 7_725_000)
```
`assert_eq!(out, U256::from(90u64))` — pass.

Chuỗi đầy đủ (`hand_verified_full_sandwich_numbers`, gas=0): front_in=100 ->
front_out=90 -> reserve sau front (1100,910) -> victim_in=50 -> victim_out=39
-> reserve sau victim (1150,871) -> back_out=107 -> profit=107-100=7. Tất cả
assert bằng đúng số nguyên trên, pass.

Quote V3 THẬT (`--ignored`, RPC công khai, WBNB->USDT, fee tier dò trong 4
tier PIN):
```
running 1 test
V3 QuoterV2 quote: 0.01 WBNB -> 7230011422939965972 USDT wei (fee tier=100)
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out; finished in 0.79s
```
(0.01 WBNB ≈ 7.23 USDT — hợp lý ở ngưỡng giá BNB hiện tại, không bịa số.)

Selector `quoteExactInputSingle` tính THẬT bởi `keccak256` của crate
`alloy` (không hardcode hex nhớ tay):
```
running 1 test
quoteExactInputSingle selector = 0xc6a5026a
test sim_v3::tests::print_computed_selector_for_evidence ... ok
```

`getPair` V2 THẬT (`--ignored`, cùng quy ước BAOCAO03/02):
```
running 1 test
V2 getPair(USDT, WBNB) real pair = 0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out; finished in 0.65s
```

2 case min-size + reason (4.1, `--nocapture` cho case ra sim):
- Ví A (min 0.01 BNB), amount 0.05 BNB, token CHƯA đo tax -> `honeypot_or_tax`
  (`victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured` — pass).
- Ví B (min 0.5 BNB), amount 0.05 BNB -> `below_min`
  (`victim_b_below_min_is_skipped_with_clear_reason` — pass).
- Ví A + cache tax=0 inject thủ công (`measured_at_block=995`, block hiện
  tại=1000, trong `tax_cache_blocks=30`) -> `Simulated`:
```
running 1 test
pipeline sim OK: front_in=1499999999999999838 back_out=1539011922260705697 profit_wei=33011922260705859
test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero ... ok
```
(front_in ≈ 1.4999999... BNB, đúng bị chặn sát trần `max_front_bnb=1.5`
config ship, `profit_wei` dương ~0.033 BNB — U256/i128 thật, không bịa.)

6. CHAIN: `0x38` (56) xác nhận trước mỗi lần `eth_call` thật trong 2 test
`--ignored` (dùng `provider.get_chain_id()`, `assert_eq!(chain_id, 56)`
trước khi gọi bất kỳ contract nào — khớp pattern BAOCAO02/03). RPC công khai
dùng để verify: `https://bsc-dataseed.binance.org/` (KHÔNG phải `.env`/RPC
runtime của bot, giữ nguyên quyết định ở `docs/STATE.md` từ BAOCAO02):
- `V2Factory.getPair(USDT, WBNB)` -> `0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae`
  (khác address(0), dán ở mục 5).
- `QuoterV2.quoteExactInputSingle` (WBNB->USDT, fee=100) -> `7230011422939965972`
  wei USDT cho `10000000000000000` wei WBNB đầu vào — dán ở mục 5.
- `V3Factory.getPool(USDT, WBNB, fee)` dò 4 tier PIN, pool thật tồn tại ở
  tier `100` (dùng lại `pool::resolve_v3_pool` đã có từ BAOCAO03, không viết
  lại).

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md` phiên này (không nằm trong ĐƯỢC
ĐỤNG, không pin address mới). `src/venues.rs` chỉ thêm `pub const` tham
chiếu LẠI đúng address đã pin từ `1.1/1.2` (không đổi giá trị, không thêm
contract mới) — `registry_snapshot`/`DEX_REGISTRY.md` vẫn khớp nhau.

8. KHÔNG LÀM: không sửa `transport.rs` (chưa có bug chặn sim ở đó); không
sửa cấu trúc parse hiện có của `decoder.rs` (chỉ thêm 1 field đọc thêm 1
word/nhánh — bug chặn sim, có ghi rõ lý do ở mục 3); không sim V4/Infinity
(vẫn `hooks_unread` mọi token, chưa pin `PoolKey`, không gọi bừa
`CLPoolManager`/`BinPoolManager`/`CLQuoter`/`BinQuoter`); không tự động điền
`TaxCache` từ `measure_roundtrip_via_router` trong pipeline (giới hạn kỹ
thuật đã chứng minh, không bịa số đo); không viết hợp đồng "probe" đo tax
thật (rủi ro bytecode tay, ngoài phạm vi phiên); không wire `pipeline.rs`
vào vòng lặp pending-tx/`main.rs` (chưa có subscribe pending-tx thật, thuộc
`5.1+`); không đổi `web.rs`/`/api/*`; không gửi tx thật (`send_raw_transaction`
vẫn không tồn tại trong repo); không đổi `dry_run/allow_live/bot_armed`/cờ
live nào; không sửa `.env`/`victims.txt` thật/`CLAUDE.md`/`DEX_REGISTRY.md`;
pipeline chỉ hỗ trợ chiều victim MUA token bằng WBNB (ghi CÒN NỢ chiều bán).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- Wire `pipeline::decide_paper` + `pipeline::log_outcome` vào vòng lặp
  pending-tx thật trong `main.rs` — cần `transport.rs` thêm
  `eth_subscribe newPendingTransactions` (chỉ có `subscribe_blocks` hiện
  tại). Thuộc `5.1` (CLAUDE.md: "Không nhảy 7.x live send trước khi paper
  4.1/5.1 có output" — 4.1 đã có output paper qua fixture, `5.1` là bước
  chạy ngắn thật với pending-tx sống).
- `pipeline.rs` chưa hỗ trợ chiều victim BÁN token lấy WBNB (chưa có model
  sandwich tương ứng — không đối xứng với chiều mua, cần thiết kế riêng,
  không bịa).
- Đo tax thật (fee-on-transfer, `balanceOf` trước/sau) cần hợp đồng "probe"
  triển khai tạm trong 1 `eth_call` — chưa viết (rủi ro bytecode tay cao,
  cần cụm riêng có thời gian kiểm kỹ, hoặc dùng compiler Solidity nếu có
  trong môi trường sau này). Tới lúc đó `honeypot_or_tax` vẫn là default an
  toàn.
- V4/Infinity vẫn chưa sim được (kế thừa nợ từ BAOCAO03: `resolve_infinity_pool`
  luôn `hooks_unread`, chưa pin cách tìm `PoolKey`/quét event `Initialize`).
- `web.rs` `/api/hits`/`/api/skips` vẫn đọc `logs/bot.jsonl`/`skip_counts`
  như cũ — `skip_counts` (HashMap trong `AppStateInner`) vẫn CHƯA được tăng
  ở đâu cả (kế thừa từ trước, không phải nợ mới của phiên này) vì chưa có
  vòng lặp thật gọi `pipeline::decide_paper` rồi cập nhật nó; sẽ làm cùng
  lúc với việc wire pending-tx ở `5.1`.
- SmartRouter V2-style 4-tham-số riêng (nợ cũ từ BAOCAO03) — vẫn chưa pin
  chữ ký, chưa đụng tới phiên này.
