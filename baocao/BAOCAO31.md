# BAOCAO31

## 1. LÁT

`foundation-fix-then-real-sim` — CỤM A (A1+A2+A3+A4+A5+A6+A7) ĐẦY ĐỦ, CỤM B
(B1+B2) đầy đủ kèm bằng chứng RPC thật, **B4 KHÔNG ĐẠT** theo đúng nghĩa hẹp
lệnh yêu cầu → dừng đúng luật, **KHÔNG làm cụm C và D** (C bị chặn bởi gate
B4; D phụ thuộc thiết kế TTL/slippage của C2).

## 2. LỆNH NHẬN

Lệnh Grok "foundation-fix-then-real-sim": 4 cụm A→B→C→D làm TUẦN TỰ trong
cùng phiên, không nhảy cụm; B4 là gate bắt buộc trước khi sang C
("Chưa đạt thì ghi CHƯA XONG, không sang C").

## 3. FILE ĐỔI

- `src/transport.rs` — A1: `PendingTxRaw` thêm `to/hash/gas/gas_price/nonce`,
  sửa `pending_tx_from_rpc<T>`.
- `src/venues.rs` — A2: `enum Venue`, `PANCAKE_ROUTERS`, `venue_for_router`,
  `Venue::as_str`, `SKIP_REASONS` thêm `not_pancake_router`/`sell_direction`.
- `src/decoder.rs` — A3: `enum SwapVenue{V2,V3{fee}}`, `DecodedSwap` thêm
  `venue`/`two_hop`, `from_packed_v3_path_with_fee` (thay
  `from_packed_v3_path`), mọi nhánh decode gán venue/fee đúng.
- `src/pipeline.rs` — A4 hỗ trợ: `PipelineSkip::NotPancakeRouter`/
  `VenueUnpinned`, `precheck_token_and_venue`, `TxLogMeta`, `log_outcome_v2`
  đổi chữ ký thêm `meta`. Test mới: `precheck_token_and_venue_exact_input_single_is_v3_with_fee`,
  `precheck_token_and_venue_v2_functions_are_v2`.
- `src/main.rs` — A4+A6: gate order thật trong `handle_paper_tx`/
  `poll_txpool_pending`/`subscribe_pending_txs` (`passes_router_gate`,
  `SEEN_CAP` 50k xoá theo tuổi, log `hash/to/venue/selector/fee`), xoá
  `FunnelCounters` cũ (chuyển sang `web.rs`), `record_funnel_terminal`.
  Test mới: `passes_router_gate_*` (3), `poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate`,
  `record_funnel_terminal_*` (3).
- `src/pairbook.rs` — A5: fix bug cắt comment cuối dòng trước parse. Test
  mới `pairbook_real_file_format_with_trailing_comment_parses_exactly_1_pool`.
- `src/web.rs` — A6: `FunnelCounters` (struct + 14 field mới theo gate order
  thật) trong `AppStateInner`, `GET /api/funnel`.
- `web/index.html`, `web/app.js` — A6: bảng Funnel mới.
- `CLAUDE.md` — A7: 4 đoạn (mục Math, Skip list, "Cấm tự làm", footer).
- `Cargo.toml`, `Cargo.lock` — B1: thêm `revm = { version = "43",
  default-features = false, features = ["std", "alloydb", "asyncdb"] }`.
- `src/sim_evm.rs` (MỚI) — B2: sim sandwich EVM thật qua `revm`+`AlloyDB`.
- `src/lib.rs` — thêm `pub mod sim_evm;`.
- `docs/STATE.md`, `docs/TASKS.md` — ghi lại quyết định kỹ thuật cụm A/B,
  lý do B4 blocked.

## 4. LỆNH CHẠY

```
cargo test --lib
cargo test --bins
cargo build --release
cargo tree -i alloy-primitives
cargo test --lib sim_evm::tests::real_rpc_sim_evm_matches_sim_v2_when_zero_tax -- --ignored --nocapture
```

## 5. OUTPUT THẬT

### `cargo test --lib` (237 passed, 0 failed, 6 ignored)

```
test result: ok. 237 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 4.45s
```

### `cargo test --bins` (9 passed — test mới cụm A trong `main.rs`)

```
test tests::passes_router_gate_true_for_none_unknown_source ... ok
test tests::passes_router_gate_true_for_pinned_router_false_for_others ... ok
test tests::passes_router_gate_rejects_fake_router_even_with_real_v2_selector ... ok
test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok
test tests::record_funnel_terminal_simulated_increments_simulated_bucket ... ok
test tests::record_funnel_terminal_venue_unpinned_increments_venue_v3 ... ok
test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok
test tests::record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket ... ok
test tests::poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### `cargo build --release`

```
Finished `release` profile [optimized] target(s) in 28.31s
```
(rebuild sau đó `0.57s` — không đổi gì thêm, xanh ổn định).

### `cargo tree -i alloy-primitives` — ĐÚNG 1 bản `1.7.3`, `revm` không kéo lệch

```
alloy-primitives v1.7.3
├── alloy-chains v0.2.38
│   └── alloy-provider v2.4.2
│       ├── alloy v2.4.2
│       │   └── bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
│       └── revm-database v43.0.0
│           └── revm v43.0.2
│               └── bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
├── alloy-consensus v2.4.2
... (toàn bộ cây còn lại đều quy về CÙNG alloy-primitives v1.7.3, xác nhận
    bằng `grep -c "^alloy-primitives v"` = 1)
```

### Test RPC thật `sim_evm::tests::real_rpc_sim_evm_matches_sim_v2_when_zero_tax` — 3 lần chạy, mempool BSC sống (dán cả 3, không chọn lọc)

**Lần 1 — khớp CHÍNH XÁC 0% (đúng yêu cầu "NỢ" bắt buộc "sim_evm 1 pool fixture cho profit khớp sim_v2 khi tax=0"):**
```
candidate token=0x113d68c8cca4fe5ba25f49c00784079d168e7777: victim_success=true buy_tax_bps=Some(0) sell_tax_bps=Some(0) sim_v2.profit=-1 sim_evm.profit=-1
=> DUNG lam fixture doi chung: token=0x113d68c8cca4fe5ba25f49c00784079d168e7777 lech profit sim_v2 vs sim_evm = 0.0000%
test sim_evm::tests::real_rpc_sim_evm_matches_sim_v2_when_zero_tax ... ok
```

**Lần 2 — EVM thật tái hiện ĐÚNG 2 revert Solidity thật (không phải bug sim):**
```
candidate token=0x335f343365fbd879327b69216d03cf5693ec6017: sim_evm loi (sim_evm thuc thi loi: front-buy revert/halt: Revert { ...
output: 0x08c379a0...
"ds-math-sub-underflow"
}), thu candidate ke
candidate token=0xba17aacf40e8b0d08774e979a324d01bd73abf83: sim_evm loi (sim_evm thuc thi loi: back-sell revert/halt: Revert { ...
"PancakeLibrary: INSUFFICIENT_INPUT_AMOUNT"
}), thu candidate ke
thread '...' panicked: khong candidate nao ... co ca buy_tax lan sell_tax do duoc va <=10bps
```
(test FAILED lần này — không có candidate zero-tax nào trong mempool lúc đó
để so sánh nghiêm ngặt, nhưng 2 revert trên là bằng chứng cơ chế đúng: sim
EVM biết pool quá mỏng/token dị thường sẽ revert thật, điều công thức đóng
`sim_v2` không thể biết).

**Lần 3 — phát hiện THẬT 1 token có buy-tax ~3% (chuẩn bị sẵn cho cụm C, CHƯA wire):**
```
candidate token=0x5e8eb66fe15518b5ab94c14b92f6ec2512f592cb: sim_evm loi (back-sell revert INSUFFICIENT_INPUT_AMOUNT), thu candidate ke
candidate token=0xcf0e0225215ae96fa6eb71b5a67cbbe6ed5a7777: victim_success=true buy_tax_bps=Some(299) sell_tax_bps=Some(0) sim_v2.profit=-1 sim_evm.profit=-1
thread '...' panicked: khong candidate nao ... co ca buy_tax lan sell_tax do duoc va <=10bps
```
(test FAILED lần này vì token duy nhất zero-sell-tax lại có buy_tax=299bps
— đúng thiết kế test chỉ chấp nhận CẢ HAI ≤10bps làm fixture "zero-tax". Bản
thân số `299` là PHÁT HIỆN THẬT có giá trị, không phải lỗi.)

**Phát hiện kỹ thuật quan trọng (đã sửa, ghi ở docs/STATE.md):**
1. `WrapDatabaseAsync::new` cần tokio runtime **multi-thread** —
   `#[tokio::test]` mặc định single-thread trả lỗi ngay
   `"can tokio multi-thread runtime"` — sửa bằng
   `#[tokio::test(flavor = "multi_thread")]`. Production không ảnh hưởng
   (`main.rs::main` đã multi-thread).
2. RPC công khai `bsc-dataseed.binance.org` KHÔNG phải archive node đầy đủ —
   fork lùi vài trăm block cho lỗi thật `-32000 missing trie node`. Đổi
   chiến lược: fork tại block `latest` + dùng tx ĐANG CHỜ thật trong
   `txpool_content` (khớp đúng kịch bản sản xuất).

Test giữ `#[ignore]` (đúng quy ước repo `real_rpc_*`), kết quả PHỤ THUỘC
thành phần mempool sống tại thời điểm chạy — không xác định (flaky) là bản
chất RPC thật, không phải lỗi code.

## 6. CHAIN

`0x38` (56) xác nhận qua `eth_chainId` mọi lần gọi RPC (`bsc-dataseed.binance.org`,
mainnet công khai). `eth_blockNumber`/`eth_getBlockByNumber`/`txpool_content`/
`eth_call` (qua `pool::resolve_v2_pair`/`get_reserves_vs_wbnb`) đều trả dữ
liệu thật trong 3 lần chạy `sim_evm` ở ô 5 (block số thật ~121,9xx,xxx tại
thời điểm chạy — không bịa).

## 7. REGISTRY

Không đổi pin nào — `DEX_REGISTRY.md` giữ nguyên. `venues.rs::PANCAKE_ROUTERS`
(A2) chỉ tham chiếu LẠI đúng 5 địa chỉ đã pin sẵn trong `DEX_REGISTRY.md`
(V2 Router, V3 SwapRouter, SmartRouter, UR v3-cũ, UR Infinity) — không thêm
địa chỉ mới nào, không getCode mới.

## 8. KHÔNG LÀM

- Quote USDT trong `sim_evm.rs` (chỉ WBNB + V2 pool phiên này).
- Full EVM ternary search cho `front_in` (dùng ước lượng `sim_v2` làm điểm
  duy nhất, lý do RPC cost — xem docs/STATE.md).
- `pipeline.rs` gọi `sim_evm::simulate_sandwich` (B3) — production live loop
  KHÔNG đổi hành vi, vẫn dùng `sim_v2`/`TaxCache` cũ 100%.
- Cụm C (đo tax tự động wire vào `tax.rs`/`pipeline.rs`, TaxCache key đổi
  `(token,quote)`, allowlist 8 token, TTL theo phút) — BLOCKED bởi gate B4.
- Cụm D (config.toml field mới D1, `calldata.rs` USDT D2, VPS D3) — D1/D2
  phụ thuộc thiết kế C2; D3 riêng cũng BLOCKED (xem ô dưới, không có SSH).
- Không sửa `victims.txt` thật, không đụng `PRIVATE_KEY`/`PRIVATE_TX_URL`,
  không bật bất kỳ cờ live nào, không gửi tx thật (0 `sendRaw` — không có
  hàm ký/gửi nào được thêm, `sim_evm.rs` chỉ chạy trong `revm` cục bộ, không
  gọi `eth_sendRawTransaction`/`eth_sendTransaction` bao giờ).
- Không đụng VPS/SSH — xác nhận lại sandbox phiên này VẪN không có outbound
  port 22 (`timeout 10 bash -c "cat < /dev/tcp/<ip>/22"` treo/timeout cho cả
  2 IP VPS còn trong `~/.ssh/known_hosts`), trong khi HTTPS/RPC công khai
  vẫn thông — cùng giới hạn hạ tầng đã ghi nhận ở BAOCAO27/28/30.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **B4 thật sự (gate bắt buộc trước C)**: cần 3 tx hash SANDWICH THẬT
  (front+victim+back đã xảy ra) — sandbox phiên này không có BscScan
  API/trình duyệt để tìm. Cần Grok: (a) cấp 3 hash cụ thể để Code replay
  trực tiếp, hoặc (b) xác nhận bằng chứng thay thế đã có ở B2 (khớp 0% với
  token zero-tax + tái hiện đúng revert thật + phát hiện đúng tax thật trên
  mempool sống) là ĐỦ để coi B4 "đạt tinh thần" trong điều kiện sandbox này,
  hoặc (c) cấp kênh có quyền truy cập BscScan/trình duyệt.
- Quote USDT cho `sim_evm.rs` (storage-override hoặc thêm 1 hop WBNB→USDT).
- Full EVM search cho `front_in` tối ưu (hiện chỉ 1 điểm từ `sim_v2`).
- Cụm B3 (nối `sim_evm` vào `pipeline.rs`) — hàm sẵn sàng, chưa gọi.
- Cụm C toàn bộ (đo tax tự động, TaxCache key `(token,quote)`, allowlist 8
  token, TTL phút) — chờ B4.
- Cụm D toàn bộ (config field mới, USDT calldata, VPS seed+rerun+GATE) —
  chờ C xong trước (D1 phụ thuộc thiết kế TTL của C2), D3 riêng còn chờ
  kênh SSH.
- `gate_order` mới ở A4 chưa lặp filter (b) [decode] trước khi spawn trong
  `poll_txpool_pending` (chỉ lặp gate (a) — quyết định có chủ đích, xem
  docs/STATE.md, đổi lại nếu Grok muốn đúng nghĩa đen "lọc a+b").
