# BAOCAO32

## 1. LÁT

`evm-validate-wire-tax` — CHỈ làm B4' (validate tự động thay BscScan).
**B4' KHÔNG ĐẠT ĐỦ SỐ** theo đúng ngưỡng lệnh yêu cầu cho B4'.3(a)/(b) →
dừng đúng luật ("Tuần tự, gate ở B4'"), **KHÔNG làm B3, C, D**.

## 2. LỆNH NHẬN

Lệnh Grok "evm-validate-wire-tax": B4' (4 mục con B4'.1→B4'.4, tự động,
không cần BscScan) → B3 (nối sim_evm vào pipeline) → C (đo tax tự động) →
D1/D2 (config/calldata USDT) → D3 (script VPS). Tuần tự, gate ở B4'.

## 3. FILE ĐỔI

- `src/sim_evm.rs` — B4'.1 viết lại hoàn toàn test
  `real_rpc_sim_evm_matches_sim_v2_when_zero_tax` (chạy qua MỌI candidate
  đo được, 3 assertion theo đúng lệnh). B4'.2: log đầy đủ front_in/
  front_out/victim_out/back_out cho từng candidate + giải thích `profit=-1`.
  B4'.3(a): test mới `real_rpc_victim_prediction_matches_onchain` +
  `predict_victim_token_delta`. B4'.3(b): test mới
  `real_rpc_replay_real_sandwich_triplets` + `replay_real_triplet`, quét
  block bằng Swap event (không lệ thuộc `tx.to()==v2_router`). B4'.4: test
  mới `real_rpc_ternary_search_evm_warm_cache_timing` +
  `refine_front_in_with_evm` (ternary search EVM trên 1 fork warm) +
  helper `probe_erc20_balance_slot`/`set_erc20_balance`/
  `mapping_storage_key`/`pack_v2_reserves_slot`/`unpack_v2_reserves_slot`/
  `reset_pair_reserves`. Refactor `simulate_sandwich` thành `open_fork` +
  `build_evm` + `run_sandwich` (tách async/sync, KHÔNG đổi hành vi — test
  cũ B2 vẫn chạy qua đường này, chỉ code-move). Thêm helper dùng chung
  `poll_wbnb_v2_candidates` (gom logic quét mempool trước đây trùng lặp
  trong 1 test, giờ 3 test cùng dùng). Sửa 3 bug thật phát hiện qua chạy
  sống (xem ô 5/docs/STATE.md): `read_balance` dùng sai `caller` (EIP-3607),
  assertion (iii) quá nghiêm ở biên làm tròn, panic thay vì skip khi không
  dò được storage slot. Sửa 1 bug K-invariant: `reset_all` (B4'.4) thiếu
  reset `balanceOf(pair)` thật bên cạnh reserve cache slot 8.
- `src/pool.rs` — thêm `get_raw_reserves_and_token0` (raw reserve0/reserve1
  + token0, dùng cho B4'.4 reset) và `get_pair_tokens` (token0+token1 từ
  CHỈ địa chỉ pair, dùng cho B4'.3(b) nhận diện WBNB-pair từ Swap event).
- `docs/STATE.md`, `docs/TASKS.md` — ghi lại đầy đủ 5 lần chạy, 3+1 bug
  thật, kết luận B4' không đạt đủ số + nguyên nhân (RPC rate-limit).

## 4. LỆNH CHẠY

```
cargo build --lib
cargo build --lib --tests
cargo test --lib
cargo build --release
cargo test --lib sim_evm:: -- --ignored --nocapture --test-threads=1
```

## 5. OUTPUT THẬT

### `cargo test --lib` (237 passed, 0 failed, 9 ignored — 6 cũ + 3 test B4' mới)

```
test venues::tests::scan_and_live_flags_pass_through_unchanged ... ok
test venues::tests::skip_reasons_contains_both_not_wbnb_pair_and_not_quote_pair ... ok
test venues::tests::skip_reasons_contains_not_pancake_router_and_sell_direction ... ok
test venues::tests::usdt_pinned_and_nonzero ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
test venues::tests::venue_for_router_matches_all_5_pinned_routers ... ok
test venues::tests::venue_for_router_none_for_unknown_router ... ok
test venues::tests::wbnb_pinned_and_nonzero ... ok
test victims::tests::bnb_to_wei_basic ... ok
test victims::tests::bnb_to_wei_rejects_garbage ... ok
test victims::tests::checksum_and_lowercase_same_wallet ... ok
test victims::tests::duplicate_address_last_line_wins ... ok
test victims::tests::garbage_lines_logged_and_skipped_no_panic ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test relay::tests::build_and_log_relay_bundle_previews_logs_single_event_with_both_requests ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 237 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 4.10s
```

### `cargo build --release`

```
Finished `release` profile [optimized] target(s) in 23.30s
```

### B4'.2 — 3 dòng log đủ số (nguồn: lần chạy sống, RPC công khai)

```
candidate token=0xcf0e0225215ae96fa6eb71b5a67cbbe6ed5a7777 pair=0x2f46584ebe98cd2b48ab027d504ed79e8da6a484 reserve_wbnb=288593195569282666667 reserve_token=11182837484560936917899735 victim_in=15572000000000000: sim_v2[front_in=2 front_out=77305 victim_out=601866056738056843107 back_out=1 profit=-1] sim_evm[token_received=74986 back_out=1 profit=-1] victim_success=true buy_tax_bps=Some(299) sell_tax_bps=Some(0) gas_wei=0
```
Giải thích SỐ THẬT: `front_out=77305` (sim_v2, không tax) vs
`token_received=74986` (sim_evm, tax thật) — chênh `2319/77305 = 3.00%`,
KHỚP CHÍNH XÁC `buy_tax_bps=299` (2.99%) đo được. Cả 2 đường tính ĐÚNG ở
bước trung gian; `back_out` CUỐI cùng bị chia nguyên (floor) về CÙNG 1 wei
ở cả 2 phía vì số tuyệt đối quá nhỏ (`front_in=2` wei) — đây là RANH GIỚI
LÀM TRÒN SỐ NGUYÊN, không phải bug logic (dẫn tới sửa B4'.1 mục (iii) từ
`<` thành `<=`, xem ô 8).

### B4'.4 — thời gian 1 lần sim đầy đủ (ms), warm-cache vs cold-fetch

```
attempt#0 front_in=981284966311454512 LOI=... thoi_gian=2663.03ms
attempt#1 front_in=1240642483155727256 LOI=... thoi_gian=8.42ms
attempt#2 front_in=894832460696696930 LOI=... thoi_gian=8.92ms
...
attempt#18 front_in=1500000000000000000 LOI=... thoi_gian=8.37ms
TONG B4'.4: 19 lan sim, trung binh 148.54ms/lan
```
Lần sim ĐẦU (fork mới, fetch AlloyDB thật) mất `2663-3441ms` (tuỳ lần
chạy); 18 lần SAU (cùng fork, chỉ reset storage local, KHÔNG fetch lại)
mất `3-10ms` mỗi lần — nhanh hơn `~300-1000 lần`, KHÔNG có RPC call mới
(xác nhận qua log: không có dòng `poll`/lỗi RPC nào giữa các attempt).

### VERIFY slot 8 (reserve packing) khớp `eth_getStorageAt` thật — 3 pair khác nhau, 3 lần chạy độc lập

```
VERIFY slot8: pair=0xabec2aa5723bee0d151d6e92724284ef266bd11a eth_getStorageAt(8)=0x6aa8a2690000000000086b5658a52dde9f8000007c056a632db808ee8cd6abe6 -> unpack(reserve0=38382688247314102095464279014, reserve1=155308419466194231168, ts=1789436521) vs getReserves() THAT (reserve0=38382688247314102095464279014, reserve1=155308419466194231168)
VERIFY slot8: pair=0x75d4194912c22e6878c606a374fe090967bf61aa eth_getStorageAt(8)=0x6aa8a56e0000000e79d630dd178de13f3b4200000000000ac196e9a552d4b7e5 -> unpack(reserve0=198417034529190754277, reserve1=17500318943260191171230530, ts=1789437294) vs getReserves() THAT (reserve0=198417034529190754277, reserve1=17500318943260191171230530)
VERIFY slot8: pair=0xabec2aa5723bee0d151d6e92724284ef266bd11a eth_getStorageAt(8)=0x6aa8a7690000000000088acc501d7d3add1100007a4cab6fcad1cf67bf1d2c77 -> unpack(reserve0=37849859147366663403081641079, reserve1=157575409549744725265, ts=1789437801) vs getReserves() THAT (reserve0=37849859147366663403081641079, reserve1=157575409549744725265)
```
CẢ 3 LẦN khớp bit-for-bit — layout `UniswapV2Pair.sol` slot 8 dùng được để
reset reserve cache mà không cần fetch lại RPC.

### B4'.3(a) — KHÔNG đạt ngưỡng ≥5 tx đủ điều kiện (dán output thật, không bịa)

```
poll_wbnb_v2_candidates #12: tich luy 1 candidate (distinct)
...
poll_wbnb_v2_candidates #39: tich luy 1 candidate (distinct)
thu thap 3 candidate de theo doi mined that
resolve duoc pair cho 3/3 candidate
3/3 candidate da mined THAT trong thoi gian cho
victim 0x8fe7feb88b06117cf29a2a3ba52e3b9451dc87ef98063b4838072fae5886ff36: eth_getLogs loi (server returned an error response: error code -32005: limit exceeded), bo qua
victim 0x572adc813cdf0c3c699e4508cc11a7a65584a89ec7fb24091145f9e9d1640b10: eth_getLogs loi (server returned an error response: error code -32005: limit exceeded), bo qua
victim 0xcbcacbba8690f329d7cb4062852dbb4e1b088fe07a8d72885ac8167f90efe5ce: eth_getLogs loi (server returned an error response: error code -32005: limit exceeded), bo qua
TONG B4'.3(a): 0 tx du dieu kien (pair khong co Swap nao khac cung block) / 3 theo doi, 0 lech <=1% (0.0%)
SKIP (khong phai FAIL): chi 0 tx du dieu kien (<5 yeu cau theo lenh) trong lan chay nay - ban chat mempool/block that, thu lai sau
test sim_evm::tests::real_rpc_victim_prediction_matches_onchain ... ok
```
Cơ chế CHẠY ĐÚNG (3/3 candidate gom được đều mined thật, đúng luồng dự
đoán → chờ mined → kiểm tra cô lập), nhưng bước kiểm tra cô lập
(`eth_getLogs`) bị RPC công khai rate-limit (`-32005`) đúng lúc cần dùng —
0/3 đủ điều kiện đánh giá. Chạy 5 lần độc lập (khác thời điểm, khác block
range) đều KHÔNG đạt ≥5 tx đủ điều kiện.

### B4'.3(b) — KHÔNG đạt ngưỡng ≥3 bộ replay được (dán output thật)

```
quet 301 block (301 loi RPC) tu 121944410 lui ve 121944110, tim thay 0 ung vien sandwich THAT (idx i,i+1,i+2 cung pair, i&i+2 cung from)
SKIP (khong phai FAIL): khong tim thay bo ba nao khop dinh nghia sandwich trong 300 block gan nhat
```
Lần chạy trên: `301/301 block` LỖI RPC (rate-limit hoàn toàn khi gọi
`eth_getLogs` liên tục cho từng block) — không phải "thật sự 0 sandwich",
mà là KHÔNG QUÉT ĐƯỢC. Các lần chạy khác quét thành công (0 lỗi RPC) vẫn
cho 0 ứng viên trong 300 block — CHƯA phân biệt được dứt điểm "hiếm thật"
với "mẫu quá nhỏ" vì mỗi lần chỉ quét được 300 block (~15 phút thật trên
BSC).

## 6. CHAIN

`0x38` (56) xác nhận qua `eth_chainId` mọi lần gọi RPC
(`bsc-dataseed.binance.org`, mainnet công khai, KHÔNG có `.env` phiên này
nên không dùng được RPC riêng của chủ). `eth_blockNumber`/
`eth_getBlockByNumber`/`eth_getStorageAt`/`eth_getLogs`/`eth_call`/
`txpool_content`/`eth_getTransactionReceipt` đều trả dữ liệu THẬT trong
5 lần chạy (block số thật ~121,94x,xxx tại thời điểm chạy) — một phần lớn
lời gọi `eth_getLogs`/`txpool_content` bị RPC trả lỗi `-32005: limit
exceeded` (rate-limit, KHÔNG bịa/che giấu, log rõ mọi lỗi).

## 7. REGISTRY

Không đổi pin nào — `DEX_REGISTRY.md` giữ nguyên. `pool.rs::get_pair_tokens`/
`get_raw_reserves_and_token0` chỉ gọi `token0()`/`token1()`/`getReserves()`
trên pair đã resolve qua factory V2 đã pin — không thêm địa chỉ mới.

## 8. KHÔNG LÀM

- B3 (nối `sim_evm::simulate_sandwich`/`refine_front_in_with_evm` vào
  `pipeline.rs`/`main.rs::handle_paper_tx`, thêm `sim_engine` config) —
  BLOCKED bởi gate B4' theo đúng luật lệnh.
- C (đo tax tự động `measure_tax_evm`, TaxCache key `(token,quote)`,
  allowlist 8 token) — BLOCKED, phụ thuộc B3.
- D1/D2/D3 (config.toml field mới, calldata USDT, script VPS) — BLOCKED,
  phụ thuộc C; D3 riêng còn thêm giới hạn không có SSH (chưa kiểm tra lại
  phiên này vì không tới lượt).
- Không sửa `victims.txt` thật, không đụng `PRIVATE_KEY`/`.env`, không bật
  cờ live, không gửi tx thật (0 `sendRaw` — `sim_evm.rs` chỉ chạy trong
  `revm` cục bộ qua `AlloyDB` READ-ONLY fork, không gọi
  `eth_sendRawTransaction`/`eth_sendTransaction` bao giờ, xác nhận lại
  bằng đọc code — không có hàm ký/gửi nào được thêm phiên này).
- Không cố "ép" B4'.3(a)/(b) đạt số bằng cách hạ ngưỡng tự ý (5→ít hơn,
  3→ít hơn) hay nới lỏng định nghĩa "cùng pair"/"cùng from" — giữ ĐÚNG số
  lệnh gốc, báo cáo thật KHÔNG đạt thay vì bịa/nới lỏng tiêu chí.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **B4'.3(a)/(b) cần RPC không rate-limit** để có cơ hội đạt đủ ngưỡng số
  trong 1 lần chạy — máy dev phiên này KHÔNG có `.env`/RPC riêng, chỉ dùng
  `bsc-dataseed.binance.org` công khai (bị `-32005` liên tục khi gọi
  `eth_getLogs`/`txpool_content` dồn dập trong vài phút). Cần Grok: (a)
  cấp RPC riêng (chủ có `.env` thật với `BSC_HTTP`/`BSC_WS` từ các phiên
  trước, phiên này sandbox không có), hoặc (b) xác nhận ngưỡng số hiện tại
  là hợp lý để Code thử lại nhiều lần hơn/dàn trải thời gian dài hơn (vượt
  quá 1 phiên), hoặc (c) chấp nhận B4'.1+B4'.2+B4'.4 (đã đạt/đo được với
  bằng chứng RPC thật) là đủ căn cứ kỹ thuật để cho phép sang B3 (dù chưa
  đúng nghĩa đen số lượng mẫu B4'.3 yêu cầu).
- B4'.4: `INSUFFICIENT_INPUT_AMOUNT` ở back-sell cho 1 token cụ thể sau khi
  front-buy đã thành công (bug K-invariant đã sửa) — nghi ngờ token có cơ
  chế nội bộ phi chuẩn (reflection/anti-bot), CHƯA xác định nguyên nhân
  gốc. Không chặn B4'.4 (không có ngưỡng PASS/FAIL) nhưng cần debug thêm
  nếu B3 sau này cần ternary-search-EVM hoạt động ổn định trên MỌI token.
- 3 hàm storage-probe mới (`probe_erc20_balance_slot`/`set_erc20_balance`/
  `mapping_storage_key`) đã verify hoạt động đúng qua sentinel test thật —
  sẵn sàng tái dùng cho B3.3 (USDT storage-override) khi B3 được phép làm.
- `poll_wbnb_v2_candidates` có warning "never used" khi build KHÔNG kèm
  `--tests` (đúng — hàm chỉ dùng trong `#[cfg(test)]`, không phải bug,
  không ảnh hưởng `cargo build --release`).
