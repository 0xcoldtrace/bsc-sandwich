# BAOCAO39 — cụm `hotpath-fix-then-decoder-ur` (Phần A + Phần B)

## 1. LÁT
`hotpath-fix-then-decoder-ur` — Phần A (4 fix nóng: A1 pairs.txt quote USDT,
A2 tax gate USDT MODE 2 ONLY, A3 tách `rpc_error`/`no_pool`, A4 giảm RPC
`known_pair`+`ReserveCache`) rồi Phần B (`decoder-coverage`: B1 lấy mẫu thật,
B2 UR đa command, B3 SmartRouter multicall, B4 cross-check venue, B5 2/3 mục
nợ BAOCAO38).

## 2. LỆNH NHẬN
Khối lệnh Grok `hotpath-fix-then-decoder-ur` (Phần A trước, DoD 60 phút chỉ
chạy sau khi A đạt; Phần B sau). Máy: WSL. Git HEAD bắt đầu:
`6b49943defecda408cb1a4235e96af79e5b75322`. Không subagent ghi file (luật #4)
— toàn bộ code/docs phiên này do phiên chính viết trực tiếp (không dùng
Agent/subagent nào cho việc ghi).

## 3. FILE ĐỔI

Commit `9275cb856003c23204a1e06d28b13992643c878f` (Phần A):
- `src/pairbook.rs` — A1: `PairEntry.quote`, `parse_pairs_line` nhận
  `token,WBNB` hoặc `token,USDT`, `PairResolver::get_pair` thêm tham số
  `quote`, `tokens_to_vet` trả kèm quote, `known_pair` (reverse index
  `(token,quote)->pair_addr`). +5 test.
- `src/pipeline.rs` — A2: `PipelineSkip::RpcError`,
  `resolve_v2_reserves`/`resolve_reserves_for_quote` tách `no_pool`/
  `rpc_error`, `resolve_v2_reserves_known_pair` (A4), `decide_paper_quote`
  thêm `pairbook`/`pair_addr`, áp MODE 2 ONLY cho nhánh USDT. +6 test.
- `src/transport.rs` — A4: `ReserveCache` (`(pair,block)->reserves`). +1 test.
- `src/venues.rs` — thêm `"rpc_error"` vào `SKIP_REASONS`.
- `src/web.rs` — `FunnelCounters::record_rpc_error`, `AppStateInner.reserve_cache`.
- `src/main.rs` — `resolve_reserves_cached` helper (A4, dùng chung 2 nhánh
  WBNB/USDT), wire `known_pair`+`ReserveCache` vào `handle_paper_tx`,
  `record_funnel_terminal` map `RpcError`.
- `pairs.txt` — CHỈ comment đầu file (định dạng cột 2 WBNB/USDT).
- `CLAUDE.md` — mục pairs.txt + Skip (thêm `rpc_error`).

Commit `8f8caf25e737e3720d045fcb8c8a61b136384d9a` (Phần B):
- `src/decoder.rs` — B2: `decode_universal_router` viết lại (đa command,
  `PERMIT2_PERMIT`+`payerIsUser`, `WRAP_ETH`+sentinel `CONTRACT_BALANCE`,
  deadline `execute3`). B3: `decode_multicall` (`multicall(bytes[])`/
  `multicall(uint256,bytes[])`), 4 selector mới
  (`exactInputSingleNoDeadline`/`exactOutputSingle(NoDeadline)`/
  `exactOutput(NoDeadline)`). Fix bug pre-existing `exactInput` (2-lớp offset
  ABI, xem mục 5). B4: `venue_matches_router` +4 tên selector. +16 test.
- `src/main.rs` — B5: `gas_units_boot_task` chờ `pair.reload` lần đầu qua
  `Notify` (thay đoán 60s cố định), log `amount_in` nhánh USDT.
- `src/web.rs` — `AppStateInner.pairs_first_reload_done: Arc<Notify>`.
- `tests/fixtures/ur_calldata.jsonl` — 45 dòng calldata THẬT (B1).
- `CLAUDE.md` — mục "Decode được phép" (UR đa command + multicall).
- `docs/STATE.md` — mục "decoder-coverage" đầy đủ + cập nhật TRẠNG THÁI HIỆN TẠI.
- `docs/TASKS.md` — 2 hàng mới + cập nhật nợ `decoder-coverage`.

Commit `0b90200d6de439f2c4d008360ca6b26fb2b2258e` (sót lại từ Phần B):
- `README.md` — mục 4 (pairs.txt quote USDT) + mục 7 (bảng skip reason,
  thêm `rpc_error`) — không đổi binary (xác nhận: sha256 sau commit này vẫn
  `eafc4ca7...`, giống hệt trước, vì chỉ đổi `.md`).

**KHÔNG đụng**: `.env`, dòng token `pairs.txt`, `config.toml` giá trị, cờ
live, `sim_v2.rs`/`sim_evm.rs` logic.

## 4. LỆNH CHẠY

```bash
cargo build --release
cargo test --release
sha256sum target/release/bsc_sandwich
git log -1 --format="%H %ci"
git status --short

scripts/paper_run.sh --minutes 5 --port 18801    # A-DoD, SAU commit Phan A
scripts/paper_run.sh --minutes 60 --port 18802   # DoD 60 phut, SAU commit Phan B
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).

### `cargo build --release` + `cargo test --release` (sau commit Phần B, HEAD `8f8caf2`)

```
Finished `release` profile [optimized] target(s) in 8.76s   (0 warning)
test result: ok. 329 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```
329 lib + 15 main = **344 passed** (tăng từ 322 đầu phiên = 6b49943, +22 test
— Phần A +6, Phần B +16). Build `cargo build --release` (không phải test) =
0 warning. `cargo test --release` có 1 warning (`transport_pending_tx_from_alloy`
never used, `src/sim_evm.rs:1742`) — **PRE-EXISTING, không do phiên này gây
ra** (xác nhận: warning này đã xuất hiện trước khi bất kỳ dòng nào của
`decoder.rs` được sửa trong phiên — thuộc backlog `test-hygiene`/F-22-F-23,
chưa làm).

Binary sha256 (SAU commit Phần A `9275cb8`): dùng cho A-DoD
`9552b0ae4126fe506ddeb6d87cc15bc2ae0800adb968070bec142448cba2013a`.
Binary sha256 (SAU commit Phần B `8f8caf2`): dùng cho DoD 60 phút
`eafc4ca74e6395c2ae47d2397b60abb97cca55beea9c73d5456a3d4be25386b3`.

### A-DoD — paper_run 5 phút (SAU commit Phần A `9275cb8`)

```
== may chay: WSL == binary sha256 = 9552b0ae4126fe506ddeb6d87cc15bc2ae0800adb968070bec142448cba2013a ==
== git HEAD = 9275cb856003c23204a1e06d28b13992643c878f ==
== pairs.txt: tong=89 vetted=89 chua_vet=0 ==
```

30 dòng `funnel.minute` cuối (mẫu 3 phút giữa, dán nguyên văn):
```json
{"below_min":0,"decode_fail":417,"gas_cap":0,"honeypot_or_tax":0,"no_pool":2,"nonce_future":0,"nonce_stale":0,"not_pancake_router":10510,"not_wbnb_pair":176,"rpc_error":1,"seen":11255,"sim_error":0,"simulated":0,"thin_liq":0,"unprofitable":3,"venue_v2":116,"venue_v3":6,"ts":"...12:54:19+00:00"}
{"below_min":0,"decode_fail":410,"gas_cap":0,"honeypot_or_tax":0,"no_pool":0,"nonce_future":0,"nonce_stale":0,"not_pancake_router":11887,"not_wbnb_pair":144,"rpc_error":3,"seen":12594,"sim_error":0,"simulated":2,"thin_liq":0,"unprofitable":8,"venue_v2":109,"venue_v3":4,"ts":"...12:55:25+00:00"}
```

`/api/skips` (tích luỹ 5 phút):
```json
{"below_min":0,"decode_fail":1856,"gas_cap":0,"honeypot_or_tax":0,"hooks_unread":0,"no_pool":5,"nonce_future":0,"nonce_stale":0,"not_in_list":711,"not_pancake_router":0,"not_quote_pair":79,"not_wbnb_pair":0,"rpc_error":9,"sell_direction":894,"sim_error":0,"thin_liq":0,"unprofitable":20,"venue_unpinned":8,"victim_would_revert":0}
```

`/api/pairs`: `{"count":89,"error_lines":0,...}` — **A1 ĐẠT** (trước fix:
82/7, sau fix: 89/0).

**A-DoD 4 số (dán từ output thật)**:
1. `/api/pairs` = **89/0** (count/error_lines) — A1 ĐẠT.
2. `honeypot_or_tax` = **0** xuyên suốt mọi dòng `funnel.minute` VÀ trong
   `/api/skips` tích luỹ — cả 2 quote (USDT thấy rõ qua `tx.skip` mẫu dán ở
   mục dưới: `"quote":"usdt","reason":"not_in_list"` — KHÔNG còn
   `honeypot_or_tax` giả, đúng luật MODE 2 ONLY, token không trong
   `pairs.txt` → `not_in_list`, không phải tax giả) — A2 ĐẠT.
3. `no_pool`=5, `rpc_error`=9 — 2 counter TÁCH RIÊNG, đều có số thật (trước
   fix: `rpc_error` luôn 0, mọi lỗi RPC gộp vào `no_pool`) — A3 ĐẠT.
4. `seen_to_decision_ms` qua `/api/econ`: **p50≈0.015ms, p95≈3692ms** (trước
   fix — BAOCAO38: p50≈30.000ms, p95≈14.864ms — p50 giảm ~2 triệu lần nhờ
   `known_pair`/`ReserveCache` bỏ hẳn `eth_call getPair` cho token đã có
   trong `pairs.txt`; **p95 vẫn cao, CHƯA đạt mục tiêu <500ms** — ghi thật,
   không hạ mục tiêu, xem ô 10) — A4 MỘT PHẦN ĐẠT (p50 vượt xa mục tiêu
   <1000ms, p95 chưa đạt).

Mẫu `tx.skip` USDT (chứng minh A2 hoạt động đúng — pool resolve được, KHÔNG
còn honeypot_or_tax giả, chỉ đúng `not_in_list` vì token không trong
`pairs.txt`):
```json
{"amount_in":null,"pair":"0x462b30a53c8552035506e82ce7b6a6c418b0477f","quote":"usdt","reason":"not_in_list","reserve_quote":"12920669706728220500490044","source":"usdt","token":"0x7cc151c2ee1016d7b586fb818759b6ae07719999",...}
```

Evidence file đầy đủ: `baocao/evidence/phaseA_5min_paperrun.txt`.

### B1 — bảng thống kê 45 mẫu calldata THẬT (`tests/fixtures/ur_calldata.jsonl`)

| Router | selector | n | Ý nghĩa (verify `keccak256` thật) |
|---|---|---|---|
| UR Infinity | `0x24856bc3` | 27 | `execute(bytes,bytes[])` — 27/27 là `SEAPORT_V1_5` (NFT, không phải swap) |
| UR Infinity | `0x3593564c` | 3 | `execute(bytes,bytes[],uint256)` — 1 `WRAP_ETH,V3_OUT,UNWRAP_WETH`, 1 `SEAPORT`, 1 `PERMIT2_TRANSFER_FROM,SEAPORT,SWEEPx4` |
| UR v3-cũ | `0x3593564c` | 5 | 3× `WRAP_ETH,V2_SWAP_EXACT_IN[,TRANSFER]`, 1× `WRAP_ETH,V3_SWAP_EXACT_IN`, 1× `V2_SWAP_EXACT_OUT` |
| SmartRouter | `0x04e45aaf` | 3 | `exactInputSingle(...)` biến thể KHÔNG deadline |
| SmartRouter | `0x5ae401dc` | 2 | `multicall(uint256,bytes[])` |
| SmartRouter | `0x09b81346` | 3 | `exactOutput(...)` KHÔNG deadline, exact-OUT |
| SmartRouter | `0xac9650d8` | 1 | `multicall(bytes[])` |
| SmartRouter | `0x5023b4df` | 1 | `exactOutputSingle(...)` KHÔNG deadline |

**Phát hiện định lượng quan trọng nhất**: ~81% (36/45, riêng UR Infinity
27/30) calldata `execute()` "decode_fail" cũ là lệnh NFT marketplace
(`SEAPORT_V1_5`), KHÔNG PHẢI swap bị bỏ sót — vẫn `decode_fail` ĐÚNG sau
B2 (không phải bug còn sót). Chi tiết đầy đủ + hash từng dòng: xem
`docs/STATE.md` mục "decoder-coverage" và `tests/fixtures/ur_calldata.jsonl`.

### `cargo test --release` (danh sách test mới B2-B4, trích)

```
test decoder::tests::decode_universal_router_real_seaport_calldata_is_correctly_decode_fail ... ok
test decoder::tests::decode_universal_router_real_wrap_eth_then_v2_swap_decodes ... ok
test decoder::tests::decode_universal_router_real_permit2_then_v2_swap_decodes ... ok
test decoder::tests::decode_universal_router_real_wrap_eth_then_v3_swap_has_v3_venue ... ok
test decoder::tests::decode_universal_router_permit2_then_v2_swap_payer_not_user_is_decode_fail ... ok
test decoder::tests::decode_universal_router_contract_balance_sentinel_with_wrap_eth_uses_tx_value ... ok
test decoder::tests::decode_universal_router_contract_balance_sentinel_without_wrap_eth_is_decode_fail ... ok
test decoder::tests::decode_universal_router_execute3_deadline_is_captured ... ok
test decoder::tests::decode_universal_router_multiple_swap_commands_takes_first ... ok
test decoder::tests::decode_multicall_bytes_wraps_exact_input_single_no_deadline ... ok
test decoder::tests::decode_multicall_ignores_non_swap_subcalls ... ok
test decoder::tests::decode_multicall_two_swap_subcalls_is_decode_fail ... ok
test decoder::tests::decode_multicall_with_deadline_propagates_to_subcall_without_own_deadline ... ok
test decoder::tests::decode_exact_output_no_deadline_matches_real_smartrouter_calldata ... ok
test decoder::tests::decode_exact_output_single_no_deadline_decodes_v3_venue ... ok
test decoder::tests::venue_matches_router_new_no_deadline_and_exact_output_selectors_two_way_check ... ok
test decoder::tests::decode_v3_exact_input_single_hop_wbnb_pair ... ok   (fixture SUA sau fix bug 2-lop offset)
test decoder::tests::decode_v3_exact_input_multihop_is_not_wbnb_pair ... ok   (fixture SUA sau fix bug 2-lop offset)
```

### DoD 60 phút (SAU commit Phần B `8f8caf2`)

```
== may chay: WSL == binary sha256 = eafc4ca74e6395c2ae47d2397b60abb97cca55beea9c73d5456a3d4be25386b3 ==
== git HEAD = 8f8caf25e737e3720d045fcb8c8a61b136384d9a ==
== pairs.txt: tong=89 vetted=89 chua_vet=0 ==
```

30 dòng `funnel.minute` cuối: xem `baocao/evidence/phaseB_60min_paperrun.txt`
dòng 17-46 (dán nguyên văn, không cắt). Mẫu 3 dòng đại diện:
```json
{"decode_fail":239,"no_pool":1,"not_pancake_router":8013,"rpc_error":1,"seen":8354,"simulated":0,"unprofitable":2,"venue_v2":23,"venue_v3":0,"ts":"...13:55:37+00:00"}
{"decode_fail":313,"no_pool":5,"not_pancake_router":12800,"rpc_error":4,"seen":13612,"simulated":0,"unprofitable":81,"venue_v2":118,"venue_v3":20,"ts":"...14:20:42+00:00"}
{"decode_fail":164,"no_pool":2,"not_pancake_router":10622,"rpc_error":0,"seen":11447,"simulated":0,"unprofitable":77,"venue_v2":100,"venue_v3":10,"ts":"...14:27:11+00:00"}
```

`/api/skips` (tích luỹ 60 phút):
```json
{"below_min":0,"decode_fail":14444,"gas_cap":0,"honeypot_or_tax":0,"hooks_unread":0,"no_pool":171,"nonce_future":0,"nonce_stale":0,"not_in_list":5661,"not_pancake_router":0,"not_quote_pair":1547,"not_wbnb_pair":0,"rpc_error":186,"sell_direction":9980,"sim_error":0,"thin_liq":0,"unprofitable":2611,"venue_unpinned":554,"victim_would_revert":8}
```
`honeypot_or_tax=0` xuyên suốt 60 phút (A2 vẫn đúng ở quy mô lớn).

`/api/pairs`: `{"count":89,"error_lines":0,...}` — A1 vẫn đúng sau 60 phút.

`/api/econ` (đầy đủ, dán nguyên văn từ output thật):
```json
{"best_net_bnb":0.08930373962339766,
 "buckets_bnb":[
   {"bucket":"<0.01","count":30031,"gross_pos":0,"net_pos":0,"median_gas_cost_bnb":0.000217,"best_net_bnb":null},
   {"bucket":"0.01-0.05","count":575,"gross_pos":0,"net_pos":0,"median_gas_cost_bnb":0.000217,"best_net_bnb":null},
   {"bucket":"0.05-0.2","count":574,"gross_pos":0,"net_pos":0,"median_gas_cost_bnb":0.000217,"best_net_bnb":null},
   {"bucket":"0.2-1","count":573,"gross_pos":0,"net_pos":0,"median_gas_cost_bnb":0.000217,"best_net_bnb":null},
   {"bucket":">=1","count":151,"gross_pos":5,"net_pos":5,"median_gas_cost_bnb":0.000217,"best_net_bnb":0.08930373962339766}
 ],
 "by_quote":{"usdt":3279,"wbnb":31904},
 "candidate":35183,
 "decode_fail_by_router":{"SmartRouter":957,"UR Infinity":13106,"V2 Router":381},
 "latency_ms":{"p50":0.012793,"p95":1779.040308,"samples":35183},
 "lines_scanned":85231,
 "net_pos_total":5,
 "nonce_stale_pct_of_candidate":0.0,
 "summary_line":"candidate=35183 net_pos=5 best_net_bnb=0.089304 p50_ms=0.01 p95_ms=1779.04 stale_pct=0.00 decode_fail_smartrouter=957",
 "top_tokens":[{"count":2936,"token":"0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d"},{"count":399,"token":"0x000008d2175f9aeaddb2430c26f8a6f73c5a0000"},{"count":373,"token":"0x7cc151c2ee1016d7b586fb818759b6ae07719999"}]}
```
**Chỉ `buckets_bnb`/`by_quote.wbnb` — `by_quote.usdt` (3279 candidate) KHÔNG
được đưa vào `buckets_bnb` (giới hạn hiện có của `/api/econ`, phát hiện
phiên này, chưa sửa — xem CÒN NỢ).**

`/api/validate`: `{"total":0,...}` — ĐÚNG THIẾT KẾ (validator chỉ chạy khi
`sim_engine="evm"`, paper run này dùng `sim_engine="v2"` ship, giống
BAOCAO38).

Halt sạch: `halt.triggered count = 1`, `tx.seen sau halt.triggered = 0`,
`tx.build=0 simulated(paper build)=0 build.refused=5`.

### **PHÁT HIỆN LỚN NHẤT: 14 candidate `Simulated` THẬT (5 WBNB + 9 USDT),
TẤT CẢ lãi dương SAU gas thật — LẦN ĐẦU TIÊN trên đường nóng `sim_engine="v2"`**

`min_profit_bnb=0`/`min_profit_usdt=0` (paper_run ép ngưỡng về 0, đúng thiết
kế "chỉ hạ ngưỡng để quan sát") nên MỌI `profit_wei>0` đều qua được —
KHÔNG có nghĩa các candidate này thắng được trên chain thật (chưa tính
cạnh tranh với bot MEV khác, chưa tính slippage/relay/latency thực thi —
đúng kết luận F1 của audit, chưa đổi). Toàn bộ 14 dòng `sim.result` thật
(hash, profit_gross/net, gas thật) — xem
`baocao/evidence/phaseB_60min_paperrun.txt` (grep `sim.result` trên
`logs/bot.jsonl` từ dòng 215141) để đối chiếu đầy đủ. `build.refused=5`
(reason `self_address_zero`) xác nhận: dù có lãi, bot **KHÔNG** build tx
thật vì chưa có signer/địa chỉ ví thật — đúng an toàn dry-run.

**5 case gần hòa nhất** (profit_net_wei nhỏ nhất trong 14 dòng, đơn vị wei —
4 case đầu cùng pool WBNB `0x76c42dda...` token BORT, 1 case USDT KHÔNG lọt
top-5-nhỏ-nhất vì tất cả case USDT có margin lớn hơn hẳn case WBNB):

| # | ts | quote | pair | gas_cost_wei | profit_gross_wei | profit_net_wei | profit_net (BNB) |
|---|---|---|---|---|---|---|---|
| 1 | 13:34:50 | wbnb | 0x76c42dda... | 217000000000000 | 11235093893341294 | 11018093893341294 | 0.011018 |
| 2 | 13:36:20 | wbnb | 0x76c42dda... | 217000000000000 | 12111454995675052 | 11894454995675052 | 0.011894 |
| 3 | 13:56:16 | wbnb | 0x76c42dda... | 217000000000000 | 13964048115366414 | 13747048115366414 | 0.013747 |
| 4 | 14:06:16 | wbnb | 0x76c42dda... | 217000000000000 | 15294738278903199 | 15077738278903199 | 0.015078 |
| 5 | 14:22:19 | wbnb | 0x76c42dda... | 217000000000000 | 89520739623397657 | 89303739623397657 | **0.089304 (best_net_bnb)** |

(9 case USDT còn lại profit_net từ ~2.9 đến ~17.6 USDT-equivalent-wei lớn
hơn 5 case trên — không thuộc "gần hòa nhất"; đầy đủ trong evidence file.)

### Trước/sau `decode_fail_by_router` (cùng khuôn khổ 60 phút, KHÔNG phải A/B
có kiểm soát — mempool BSC thật khác giờ khác lưu lượng, chỉ so được xu
hướng, không so được % tuyệt đối chính xác)

| Router | BAOCAO38 (TRƯỚC B2/B3) | BAOCAO39 (SAU B2/B3) |
|---|---|---|
| UR Infinity | 22382 | 13106 |
| SmartRouter | 1958 | 957 |
| V2 Router | 364 | 381 |
| SwapRouter | 82 | *(0, không xuất hiện)* |
| UR v3 (cũ) | 93 | *(0, không xuất hiện — tách riêng "UR Infinity" ở BAOCAO39, gộp region `universal_router` chung)* |
| **Tổng decode_fail** | **24879** | **14444** |

**Số liệu code-attributable rõ ràng nhất (không phụ thuộc lưu lượng thị
trường)** — đếm TRỰC TIẾP trong chính 60 phút này, router SmartRouter/UR,
theo `reason` (KHÔNG so với BAOCAO38, so trong-run):
- SmartRouter: 1283 candidate tổng, `decode_fail`=957 (74.6%), phần
  CÒN LẠI 326 (25.4%) decode THÀNH CÔNG (`sell_direction`=282,
  `not_in_list`=20, `no_pool`=17, `not_quote_pair`=3, `venue_unpinned`=3,
  `rpc_error`=1) — trước B3, `multicall`/biến thể không-deadline này
  KHÔNG THỂ decode dưới bất kỳ hình thức nào (100% decode_fail).
- UR (Infinity+v3-cũ gộp): 14918 candidate tổng, `decode_fail`=13106
  (87.9%), CÒN LẠI 1812 (12.1%) decode thành công (`venue_unpinned`=427,
  `not_in_list`=511, `not_quote_pair`=492, `sell_direction`=307,
  `no_pool`=51, `rpc_error`=12, `unprofitable`=12).
- `venue_unpinned` (đếm gộp, cụm mới): **554** trong 60 phút — TRƯỚC B2/B3,
  giá trị này về mặt CẤU TRÚC không thể vượt quá số lượng call đơn-command
  V3 cũ (code chưa hỗ trợ multi-command/multicall nào cho tới B2/B3) — đây
  là candidate MỚI được phân loại đúng (không còn lẫn vào `decode_fail`).

### `git status --short` (SAU commit cuối `0b90200`, TRƯỚC khi thêm chính BAOCAO này) + `git log -1`

```
(rỗng)
```
```
0b90200d6de439f2c4d008360ca6b26fb2b2258e 2026-09-15 21:37:02 +0700
```
Binary sha256 SAU commit `0b90200` (rebuild lại để xác nhận): `eafc4ca74e6395c2ae47d2397b60abb97cca55beea9c73d5456a3d4be25386b3`
(giống hệt sha256 của commit `8f8caf2` — đúng, `README.md` không ảnh hưởng
biên dịch).

## 6. CHAIN — `0x38`
- Cả 2 lần paper run (5 phút + 60 phút) xác nhận `chain_id=56` qua
  `connect_and_verify` (assert cứng, không log lỗi chain mismatch).
- `eth_getTransactionByHash` (B1, lấy mẫu calldata thật) qua RPC công khai
  đầu tiên trong `BSC_HTTP` — đã verify `eth_blockNumber` trả block thật
  (`0x7461505`) trước khi tin dữ liệu.
- Không đụng `DEX_REGISTRY.md`/pin venue nào phiên này.

## 7. REGISTRY
Không đổi pin. `DEX_REGISTRY.md` giữ nguyên — decoder mở rộng chỉ thêm khả
năng GIẢI MÃ calldata cho router ĐÃ pin, không thêm router mới.

## 8. KHÔNG LÀM
- Không bật live/`bot_armed`/`dry_run=false`, không ký/gửi tx.
- Không sửa `.env`/dòng token `pairs.txt`/`config.toml` giá trị.
- Không sửa `sim_v2.rs`/`sim_evm.rs` logic.
- Không giao việc ghi/sửa file cho subagent/fork (luật #4).
- Không wire đầy đủ nonce gate (F-13) vào đường nóng `sim_engine="v2"` —
  chỉ 2/3 mục nợ BAOCAO38 làm (gas_units_boot_task race, log amount_in
  USDT) — nonce gate đầy đủ (prefetch cache + `nonce_unknown`) CHƯA làm,
  ghi CÒN NỢ.
- Không decode multihop (2+ hop trong 1 `execute()` hoặc qua nhiều
  `multicall`), không thêm `*_SWAP_EXACT_OUT` làm command chính UR, không
  map recipient sentinel `MSG_SENDER`/`ADDRESS_THIS` → `tx.from`.

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

### Ô 10 bằng số (theo đúng yêu cầu lệnh)

- **`decode_fail` giảm bao nhiêu**: KHÔNG có 1 con số % sạch (không phải A/B
  có kiểm soát — 2 lần chạy ở giờ khác nhau, lưu lượng mempool thật khác
  nhau). Số liệu trong-run rõ ràng nhất (không phụ thuộc lưu lượng): trong
  1283 candidate SmartRouter phiên này, 326 (25.4%) decode THÀNH CÔNG mà
  TRƯỚC B3 chắc chắn 100% `decode_fail` (multicall/không-deadline chưa được
  hỗ trợ dưới bất kỳ hình thức nào); trong 14918 candidate UR, 1812 (12.1%)
  decode thành công tương tự. `venue_unpinned` (đếm gộp) = **554** trong 60
  phút — về CẤU TRÚC, giá trị này = 0 trước cụm này với mọi multi-command
  UR/multicall (code chưa tồn tại). Raw before/after theo router (không
  kiểm soát lưu lượng): UR Infinity 22382→13106, SmartRouter 1958→957,
  tổng decode_fail 24879→14444.
- **`candidate V2 tăng bao nhiêu lần`**: KHÔNG tính được 1 hệ số sạch — không
  có baseline "trước" cùng điều kiện thị trường (venue_v2 không bị ảnh hưởng
  trực tiếp bởi B2/B3, vốn chỉ sửa nhánh V3/UR/SmartRouter). Số thật phiên
  này: `venue_v2` = 5376 (tổng 60 phút, cộng dồn từ `funnel.minute`), `venue_v3`
  = 1108 (tổng 60 phút) — không so được với BAOCAO38 vì BAOCAO38 không công
  bố 2 số tổng-60-phút này ở dạng cộng dồn.
- **`best_net_bnb` + bucket**: **0.08930373962339766 BNB**, bucket **`>=1`**
  (victim mua ≥1 BNB), `gross_pos=5 net_pos=5` trong bucket đó (5/151
  candidate ≥1 BNB có lãi dương SAU gas — LẦN ĐẦU TIÊN số này >0 trên
  `sim_engine="v2"` mode 2, BAOCAO38 luôn 0). Các bucket nhỏ hơn
  (`<0.01`/`0.01-0.05`/`0.05-0.2`/`0.2-1`) đều `gross_pos=net_pos=0` — chưa
  thấy lãi dương ở size nhỏ.
- **5 case gần hòa nhất**: xem bảng ở mục 5 (5 case WBNB nhỏ nhất trong 14
  `sim.result` thật, profit_net từ 0.011018 đến 0.089304 BNB — cùng pool
  `0x76c42dda...`/token BORT).
- **p50 latency**: `/api/econ` = **0.012793ms** (A-DoD 5 phút: 0.015ms) —
  giữ vững cải thiện triệt để từ A4 (`known_pair`/`ReserveCache`) xuyên suốt
  60 phút, KHÔNG chỉ là hiệu ứng ngắn hạn lúc mới boot. p95 = **1779.04ms**
  (giảm từ A-DoD 5 phút 3692ms, nhưng VẪN cao hơn mục tiêu p95<500ms — CHƯA
  đạt, ghi thật không hạ mục tiêu).

### Phát hiện phụ (KHÔNG bịa, ghi rõ vì không giải thích được hết trong phiên)

- **Lệch `funnel.minute` (cộng dồn `simulated`=27) vs số dòng log
  `sim.result` thật (=14, xác nhận 3 cách: `grep -c`, `awk NR>=`, lọc theo
  khung giờ `ts`, VÀ 14 hash DUY NHẤT — không phải trùng lặp)**: đã kiểm tra
  `record_funnel_terminal`/`log_outcome_v2` là 2 câu lệnh liên tiếp, CÙNG
  biến `outcome`, KHÔNG có `.await` xen giữa, CHỈ 1 nơi gọi
  `record_simulated()` trong toàn bộ code (`main.rs:830`) — không tìm ra
  nguyên nhân trong thời gian phiên này. Không ảnh hưởng tới bảng "5 case
  gần hòa nhất"/`best_net_bnb` (lấy trực tiếp từ 14 dòng `sim.result` thật,
  không lấy từ `funnel.minute`). Ghi CÒN NỢ, cần điều tra thêm nếu Chủ/Grok
  cần con số `simulated` chính xác tuyệt đối.

### Còn nợ khác (không đổi từ nhận định trước khi chạy 60 phút)

- **Nonce gate (F-13)** chưa wire vào đường nóng v2 — CÒN NỢ từ BAOCAO38,
  chưa làm phiên này (chỉ làm 2/3 mục nhỏ khác của nợ đó).
- **Multihop** (2+ hop UR trong 1 `execute()`, hoặc 2+ sub-call swap trong 1
  `multicall`) — `decode_fail` có chủ đích, chưa có model.
- **`*_SWAP_EXACT_OUT` làm command chính trong UR** — chưa thêm (khác
  `exactOutputSingle`/`exactOutput` của SmartRouter multicall, đã hỗ trợ).
- **Recipient sentinel `MSG_SENDER`/`ADDRESS_THIS` → `tx.from`** — chưa map
  (cần thêm tham số `from` xuyên `decode_swap_calldata`, rủi ro sửa nhiều
  call site cho 1 field không ảnh hưởng quyết định sandwich).
- **`/api/econ` `buckets_bnb` không gồm candidate quote USDT** (3279 candidate
  `by_quote.usdt` phiên này — không xuất hiện trong bất kỳ bucket nào của
  `buckets_bnb`, phát hiện MỚI phiên này, CHƯA sửa) — cần cụm riêng nếu Chủ
  muốn thấy `net_pos`/`best_net` cho USDT (hiện tại: biết có 9 case USDT lãi
  dương từ `sim.result` thô, nhưng KHÔNG hiện trong bucket UI).
- **p95 latency** (~1779ms ở 60 phút, ~3692ms ở A-DoD 5 phút) vẫn CHƯA đạt
  mục tiêu p95<500ms dù p50 đã cải thiện mạnh — cần điều tra thêm (nghi vấn:
  GasOracle/token chưa-trong-pairs.txt vẫn tốn RPC đầy đủ, hoặc outlier hiếm
  kéo p95 lên) — ghi CÒN NỢ, không hạ mục tiêu.
- **Quyết định kinh tế tổng thể**: 14 candidate lãi dương thật (SAU gas) lần
  đầu xuất hiện trên `sim_engine="v2"` mode 2 — tín hiệu TÍCH CỰC hơn hẳn
  BAOCAO38 (luôn 0), nhưng KHÔNG kết luận thay Chủ/Grok đây có phải "đáng
  làm tiếp `strategy-exec`" hay không — cần thêm dữ liệu (14 case trong 60
  phút, tất cả cùng 2 pool `BORT`/`BNC`, có thể là đặc thù của 2 pool đó chứ
  không đại diện toàn bộ 89 pool).

Commit chính: `9275cb856003c23204a1e06d28b13992643c878f` (Phần A),
`8f8caf25e737e3720d045fcb8c8a61b136384d9a` (Phần B).
