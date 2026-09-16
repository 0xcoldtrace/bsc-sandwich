# CLAUDE.md — SANDWICH BOT BSC

Bạn là Claude Code. **Mỗi phiên là trắng.** Không nhớ chat cũ. Không nói “như lát trước”. Chỉ tin file repo + khối lệnh lần này.

## Ai làm gì

| Ai | Việc |
|---|---|
| **Điều hành** (Grok, hoặc Claude trong chat của Chủ — gọi chung là "Grok" trong file này) | Người điều hành duy nhất. Ra khối lệnh tự chứa, đọc BAOCAO, ĐẠT/FAIL, ra lệnh tiếp. Không đụng code. |
| **Chủ** | Copy Grok → Code. Copy BAOCAO / lỗi → Grok. Điền `victims.txt`, `.env`, bật cờ live, deploy VPS. Không tự ĐẠT. |
| **Claude Code** | Thợ. Làm hết cụm trong lệnh; **được kéo thêm việc dính liền trong cùng phiên** để khỏi nợ lát. Một file BAOCAO. Không điều hành. Không sửa file này trừ khi lệnh bảo sửa. |

Không có Claude Desktop. Chủ đưa báo cáo cho Grok là đủ.

Máy: **dev = WSL `~/bsc-sandwich`** (Claude Code chạy ở đây, ext4 native, không qua `/mnt/c`). **Production = VPS** — chỉ nhận binary/commit đã qua paper run trên WSL. Hai máy phải cùng git commit; lệch = MISSING.

## Phiên Code trắng

Khối lệnh phải có: cụm GĐ, file được/cấm, output cần dán, số BAOCAO, stack.

Cấm lệnh: “tiếp tục”, “như cũ”, “ông biết rồi”.

Hết phiên không có `baocao/BAOCAO{NN}.md` = FAIL.

Ưu tiên **một phiên = một cụm dính nhau**, không cắt vụn để nợ vòng copy.

---

## Sản phẩm

- BSC `56`. Dry-run mặc định.
- Stack **Rust** + tokio. RPC: **alloy hoặc ethers-rs, đúng 1** — ghi `docs/STATE.md` khi khởi tạo. Cấm npm app / viem / ethers.js / Python runtime.
- `victims.txt`: `0xAbc...,0.01` (`address,min_swap_bnb`) — **mode 1, TẮT mặc định** (`wallet_scan_enabled=false`, xem "Chiến lược đã chốt").
- `pairs.txt` là nguồn candidate DUY NHẤT đang bật (**mode 2, pair-mode**) — token do Chủ vet tay, xem "Chiến lược đã chốt" + định dạng dòng bên dưới.
- Pair: token/WBNB hoặc token/USDT — quote asset xác định THEO GIAO DỊCH
  NẠN NHÂN: victim mua bằng WBNB/BNB → front-run bằng WBNB/BNB; victim mua
  bằng USDT → front-run bằng USDT (không trộn quote trong 1 path). USDT
  `0x55d398326f99059fF775485246999027B3197955` — pin + getCode khi làm
  registry, ngang hàng WBNB.
- Venue: **mọi phiên bản PancakeSwap AMM đang live trên BSC** — V2, V3, V4/Infinity, **và bản mới hơn nếu đã deploy** (docs chính thức + `eth_getCode > 0` trên chain 56). Đây là mục tiêu sản phẩm, không phải optional.
- Chưa pin một version → **không trade version đó**, vẫn làm các version khác. Cấm bỏ V3/V4/latest ra khỏi roadmap vì “làm V2 trước cho xong”.
- Registry một cụm phải liệt kê đủ family Pancake trên BSC; thiếu family nào ghi `DISABLED + lý do + source đã tìm`, không im lặng cắt.
- WBNB `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c` — pin + getCode khi làm registry.
- 1 signer. Cấm bịa address / ABI / profit / chữ ĐẠT.

### victims.txt

```
0xAbc...,0.01
0xDef...,0.5
# comment OK
```

Min của **đúng ví** → wei (`0.01` = `10^16`). Dòng lỗi log + bỏ. Trùng address: dòng sau thắng. Hot-reload theo config. Code không bịa ví.

### pairs.txt (cụm `strategy-lock-mode2` — nguồn candidate DUY NHẤT đang bật)

Định dạng 1 dòng:

```
0xToken # SYMBOL | vetted YYYY-MM-DD | tax b/s | owner renounced|active | note
```

Dòng KHÔNG có `vetted YYYY-MM-DD` hợp lệ (thiếu hẳn, hoặc có chữ `vetted`
nhưng không kèm ngày) → bot coi là **CHƯA VET**, không thành candidate
(`pairs_require_vetted=true` mặc định). Chủ tự điền ngày sau khi vet tay
(`scripts/vet_goplus.sh` lọc thô + tự soát bằng mắt). Địa chỉ/format phần
trước `#`: `0xAddress` (quote ngầm định WBNB) hoặc `0xToken,0xQuote` — cột 2
PHẢI là WBNB hoặc USDT đã pin (cụm `hotpath-fix-then-decoder-ur`, A1 — trước
đó cột 2 chỉ nhận WBNB, khiến mọi dòng quote USDT rơi vào `error_lines`), địa
chỉ khác → lỗi dòng, không sim.

### Decode được phép

- **Gate đầu tiên, trước decode:** `tx.to` phải là 1 trong 5 router Pancake đã pin (V2 Router, V3 SwapRouter, SmartRouter, UR v3, UR Infinity — `venues::PANCAKE_ROUTERS`). Khác → `not_pancake_router`, 0 RPC. `tx.to = None` chỉ được qua khi nguồn là `inject`. Venue suy từ `tx.to` phải khớp venue suy từ selector; lệch → `decode_fail{venue_mismatch}`.
- V2 Router: `swapExactETHForTokens` / `swapExactTokensForETH` / `swapExactTokensForTokens` **và 3 biến thể `*SupportingFeeOnTransferTokens`** (đa số tx thật dùng nhóm này), path đúng 2 token có quote.
- V3 exactInput / exactInputSingle (bản có deadline, VÀ bản KHÔNG deadline của SmartRouter — cụm `decoder-coverage` B2/B3) khi đã pin; V3/exactOutput* hiện `venue_unpinned` cho tới khi sim V3 nối dây.
- **SmartRouter / Universal Router** (cụm `decoder-coverage` B2/B3, thay thế giới hạn "đúng 1 command" cũ): UR `execute(bytes,bytes[])`/`execute(bytes,bytes[],uint256)` duyệt TOÀN BỘ chuỗi command, tìm command swap đầu tiên (`V2_SWAP_EXACT_IN`/`V3_SWAP_EXACT_IN`) — `WRAP_ETH`/`PERMIT2_PERMIT` đứng trước, `UNWRAP_WETH`/`SWEEP`/`TRANSFER` đứng sau đều được bỏ qua (không đổi path/hướng). `PERMIT2_PERMIT` trước swap đòi `payerIsUser=true`, sai → `decode_fail`. `amountIn` = sentinel `CONTRACT_BALANCE` (2^255) + có `WRAP_ETH` trước → thay bằng `tx.value`. `SmartRouter.multicall(bytes[])`/`multicall(uint256,bytes[])`: bóc từng sub-call, tìm ĐÚNG 1 sub-call swap (`exactInputSingle`/`exactInput`/`exactOutput*`/V2 functions) — sub-call khác (`refundETH`/`unwrapWETH9`/`sweepToken`) bị bỏ qua; >1 sub-call swap (multihop nhiều lời gọi) → `decode_fail`, không đoán. Đa số `execute()` "decode_fail" cũ (mẫu thật ~81%, xem `tests/fixtures/ur_calldata.jsonl`) là lệnh NFT marketplace (`SEAPORT_V1_5`...) — KHÔNG PHẢI swap bị bỏ sót, vẫn `decode_fail` đúng sau B2.
- V4/Infinity / bản mới hơn: decoder theo docs đã pin (CL/LB/hooks đúng family).
- Chỉ nhận **victim đang mua** (`path[0]` = quote). Victim bán → `sell_direction`. 3+ token → `not_quote_pair`. Quote hợp lệ: WBNB luôn; USDT khi `scan_quote_usdt=true`.

Cấm: Uniswap factory, flashloan, steal approval, honeypot drain. Hook không đọc được → skip **pool đó**, không tắt bot, không bỏ family.

### Math

V2 (0.25%):

```
amountOut = (amountIn * 9975 * reserveOut) / (reserveIn * 10000 + amountIn * 9975)
```

Cụm `strategy-lock-mode2` (chủ chốt 2026-09-15, xem "Chiến lược đã chốt"):
đường nóng (mọi tx đi qua `pairs.txt`) dùng THẲNG công thức đóng V2 ở trên +
gas thật (F-03) để quyết định `Simulated`/lợi nhuận — KHÔNG mở fork EVM mỗi
tx nữa, vì token trong `pairs.txt` đã được Chủ VET TAY (không tax, không
honeypot) trước khi vào danh sách. `sim_engine` ship = `"v2"`. `revm` KHÔNG
biến mất — vẫn giữ đúng 3 việc: (a) vet nền định kỳ cho `pairs.txt`
(`pairs_vet_task`), (b) đo lại token ngay trước khi ký ở live (`7.x`), (c)
validator `validate.victim` (đối chiếu dự đoán vs thật). Quyết định cũ
"EVM THẬT mỗi tx trên đường nóng" (cụm `foundation-fix-then-real-sim`/
`evm-validate-fixed-then-wire`) ĐÃ THAY THẾ bởi quyết định này — không phải
song song. F-03 (gas hiện lấy từ trần cấu hình, cao hơn thực tế) VẪN CHƯA
sửa (cụm `real-economics-mode2` sửa), số `unprofitable`/`profit` trên đường
nóng V2 vẫn chưa dùng để kết luận kinh tế chính xác cho tới khi đó.

V3 / V4 / Infinity / bản mới: `eth_call` quoter/router/pool-manager đã pin. Không đoán tick/hooks.

`profit = backWBNB - frontWBNB - gasFront - gasBack`
(pool quote WBNB). **Cụm `real-economics-mode2` (đã sửa F-03) — đường nóng
`sim_engine="v2"`:** `gasFront+gasBack` giờ là `gas_cost_wei` ĐO THẬT
(`eth_gasPrice` qua `GasOracle` × `max(giá đó, gas_price của chính victim)` ×
gas unit đo 1 lần lúc boot bằng revm trên 1 pair đã vet — xem
`docs/STATE.md` mục "real-economics-mode2 cụm B"), KHÔNG còn lấy thẳng từ
trần cấu hình. `front_max_gas_bnb_wei + back_max_gas_bnb_wei` giờ CHỈ là
TRẦN so với `gas_cost_wei` đó — vượt trần, hoặc `eth_gasPrice` đo được vượt
`gas_price_max_gwei`, → skip `gas_cap` (không tính sim). Số `unprofitable`/
`profit` trên đường nóng v2 giờ DÙNG ĐƯỢC để kết luận kinh tế. **CÒN NỢ**:
đường `sim_engine="evm"` (không phải hot path — chỉ dùng cho vet nền/
pre-sign/validator) VẪN dùng trần cấu hình trực tiếp, chưa nối gas thật.

Thứ tự quyết định khi `sim_engine="evm"` (cụm 1+2): EVM quyết định `Simulated`/`unprofitable`/`victim_would_revert`; công thức đóng chỉ ước lượng `front_in`. `tx.build` CHỈ sau khi EVM trả `Simulated`, và CHỈ khi có địa chỉ ví thật (`to != 0x0`) — chưa có signer thì `build_refused`.
Pool quote USDT: profit_usdt = backUSDT - frontUSDT - gas_usdt (gas_usdt =
`gas_cost_wei` (BNB) quy đổi sang USDT qua reserve THẬT của pool WBNB/USDT
tại block hiện tại — KHÔNG price oracle, KHÔNG dùng pool token/USDT đang
xét). **Cụm `real-economics-mode2` (mục 1.d) đã BỎ luật cũ "profit_usdt
không trừ gas"** — gas giờ trừ THẲNG vào profit USDT, cùng cơ chế WBNB. Trần
gas (`gas_cap`) LUÔN so bằng đơn vị BNB (gas trả bằng BNB bất kể quote asset
nào của pool) — gate độc lập với việc quy đổi profit, y hệt cơ chế cũ về
mặt cấu trúc (2 field `gas_reserve_bnb_wei`/`front_max_gas_bnb_wei`/
`back_max_gas_bnb_wei` vẫn là field BNB có sẵn).

`frontIn <= max_front_bnb`. `U256` only.

Config thêm: scan_quote_usdt (bool, ship false), min_profit_usdt,
max_front_usdt, min_reserve_usdt (cùng luật hot-reload/validate như
*_bnb). scan_quote_usdt=false → hành vi WBNB không đổi gì.

victims.txt GIỮ NGUYÊN format, min_swap_bnb chỉ áp cho quote WBNB. Quote
USDT đi qua pair-mode/universal-mode dùng min_reserve_usdt (mức pool),
KHÔNG thêm cột cho wallet-mode ở cụm này.

Nhiều pool WBNB: sim version `scan_*=true` đã pin, chọn **1 profit max**.

Skip: `not_in_list | below_min | decode_fail | not_wbnb_pair | not_quote_pair | sell_direction | not_pancake_router | venue_unpinned | no_pool | rpc_error | thin_liq | deadline | nonce_stale | nonce_future | victim_would_revert | unprofitable | honeypot_or_tax | hooks_unread | sim_error | gas_cap | sanity_reject | competitor_victim`
(`sanity_reject` + `competitor_victim` thêm ở cụm
`bugfix-presign-and-contract-plan` A2/A3, ghi vào CLAUDE.md ở cụm
`decision-data-24h` mục 6 theo lệnh Chủ. `sanity_reject`: kết quả sim vi phạm
1 trong 3 trần vô lý — `front_in > 10%` reserve, `profit > 2%` reserve, hoặc
`victim_in > 100%` reserve (đo thật 30 phút: 106/106 dòng pass, cổng không cắt
cơ hội thật nào). `competitor_victim`: victim là ví của CỤM ĐỐI THỦ MEV đã
trinh sát (`src/competitor.rs` — 3 seed đã verify on-chain + ví burner nhận
Transfer quote từ seed trong block ±1), chỉ skip khi
`allow_competitor_victims=false` (ship) VÀ `live_mode` khác `"off"`. Đo thật
10.92 h trên VPS: 481/497 cơ hội có lãi là ví của cụm này, nên mọi số `net_pos`
phải đọc kèm `net_pos_non_cluster`.)
(`gas_cap` thêm ở cụm `real-economics-mode2` — F-03: `gas_cost_wei` đo thật
vượt trần `front_max_gas_bnb_wei+back_max_gas_bnb_wei`, HOẶC `eth_gasPrice`
đo được vượt `gas_price_max_gwei`.)
(`rpc_error` thêm ở cụm `hotpath-fix-then-decoder-ur` A3 — `eth_call`
`getPair`/`getReserves` LỖI THẬT (timeout/mạng), TÁCH khỏi `no_pool` (Factory
trả `address(0)`, chắc chắn không pool) — trước đó 2 tình huống gộp chung vào
`no_pool`.)
(`nonce_stale`/`nonce_future` thêm ở cụm 1. `deadline` phải có code path sinh ra thật, không chỉ khai báo.)

---

## Chiến lược đã chốt (2026-09-15)

Chủ CHỐT chiến lược (cụm `strategy-lock-mode2`), áp dụng cho mọi phiên sau:

1. **CHỈ mode 2 (pair-mode, `pairs.txt`).** Mode 1 (`victims.txt`) và mode 3
   (universal) TẮT bằng cờ (`wallet_scan_enabled=false`,
   `pair_scan_universal=false`), KHÔNG xoá code — có thể bật lại sau bằng
   cờ nếu Chủ đổi ý.
2. **Token trong `pairs.txt` do CHỦ VET TAY**: verified, không tax, không
   honeypot, không blacklist, không cooldown/anti-MEV, không pausable,
   không rebase, có pool V2 với WBNB hoặc USDT reserve ≥ 20 BNB (hoặc
   ≥ 10.000 USDT). Bước lọc thô: `scripts/vet_goplus.sh` (GoPlus Security
   API, lọc nhanh trước khi Chủ tự soát tay). Bước xác nhận: bot đo
   `measure_tax_evm` NỀN (không chặn đường nóng). Token CHƯA VET → bot
   KHÔNG được sim (`pairs_require_vetted=true`).
3. **Đường nóng KHÔNG đo tax/honeypot mỗi tx.** Sim đường nóng =
   lãi/lỗ theo công thức V2 (phí pool 0.25%) + gas thật. `sim_engine`
   ship = `"v2"` (thay `"evm"` — đường nóng không mở fork EVM mỗi tx nữa,
   vì token đã qua vet ở mục 2).
4. **`revm` giữ lại đúng 3 việc**: (a) vet NỀN định kỳ cho `pairs.txt`
   (`pairs_vet_task`, mỗi `pairs_vet_interval_sec`), (b) đo lại token NGAY
   TRƯỚC KHI KÝ ở live (`7.x`, chưa làm ở cụm này), (c) validator
   `validate.victim` (so dự đoán vs thật). KHÔNG fork per-tx trong đường
   nóng — đây là điểm khác biệt cốt lõi so với chiến lược cũ
   (`evm-validate-fixed-then-wire`/`foundation-fix-then-real-sim`).

---

## 0.ANTI

Không chắc → `MISSING` | `CHƯA ĐỌC` | `FAIL`.

Cấm bịa pin; cấm “test pass” không dán output; cấm tự ĐẠT; cấm sendRaw khi `dry_run=true`.

Pin = `DEX_REGISTRY.md` + source_url + ngày + `eth_getCode > 0` trong BAOCAO.

### 3 luật bổ sung (chủ ra lệnh 2026-09-15)

1. **Mỗi cụm kết thúc bằng git commit.** Làm xong cụm trong lệnh (kể cả
   việc kéo thêm dính liền cùng phiên) → `git commit` trước khi đóng
   phiên. BAOCAO ô 3 (FILE ĐỔI) phải ghi kèm hash commit đó
   (`git log -1 --format=%H`). Không commit = phiên chưa xong, ghi
   `CHƯA XONG`, không được ghi `CHỜ GROK`.
2. **Mọi số liệu runtime phải ghi rõ chạy ở đâu + hash binary.** Bất kỳ
   output thật nào dán vào BAOCAO (`cargo test`, paper run, `real_rpc_*`,
   `/api/*`, log jsonl...) phải kèm: (a) máy chạy — `WSL` hay `VPS` (ghi
   rõ, không được để trống), (b) `sha256sum` của binary đã build lúc chạy
   (`target/release/bsc_sandwich`) hoặc git HEAD hash nếu chạy qua `cargo
   run`/`cargo test` trực tiếp. Số liệu không ghi được 2 mục này coi như
   `MISSING`, không được tính là bằng chứng ĐẠT.
3. **`real_rpc_*` không được "pass rỗng".** Test nhóm `real_rpc_*`
   (`#[ignore]`, gọi RPC thật) chỉ được báo là chạy được khi có output
   THẬT dán kèm (số block, giá trị quote, tx hash...) — không được ghi
   "ignored" rồi coi như đã verify, không được dán dòng `... ok` mà không
   kèm số liệu RPC thật phía trên nó, không được chạy với danh sách URL
   rỗng rồi báo FAIL/SKIP im lặng. Thiếu RPC thật để chạy → ghi `MISSING`
   rõ ràng ở BAOCAO, không tự suy diễn kết quả.

4. **Subagent chỉ được ĐỌC.** Claude Code không giao cho subagent/fork bất
   kỳ việc ghi/xoá/git nào. Subagent chỉ để đọc và tóm tắt; mọi thay đổi
   file do phiên chính làm và ghi vào BAOCAO.

---

## BAOCAO — một phiên một file mới

`baocao/BAOCAO01.md`, `02`, … không đè.

```
1. LÁT: cụm đã làm (vd 0.1+0.2+0.3)
2. LỆNH NHẬN:
3. FILE ĐỔI:
4. LỆNH CHẠY:
5. OUTPUT THẬT: ≥ 15 dòng cuối nếu có test
6. CHAIN: 0x38 + getCode/eth_call rút gọn hoặc MISSING
7. REGISTRY:
8. KHÔNG LÀM:
9. CHỮ: CHƯA XONG | FAIL | CHỜ GROK
10. CÒN NỢ / LÁT SAU: chỉ việc thật sự chưa làm được
```

Grok ĐẠT khi có ô 5. Code không viết ĐẠT.

---

## Config — thiếu field = fail load

`chain_id dry_run allow_live bot_armed scan_v2 scan_v3 scan_v4 live_v2 live_v3 live_v4 min_profit_bnb max_front_bnb min_reserve_wbnb victims_path victims_reload_sec config_reload_sec pending_poll_ms pending_txpool_max_per_poll gas_reserve_bnb_wei front_max_gas_bnb_wei back_max_gas_bnb_wei tx_timeout_sec ws_silence_sec max_consecutive_loss max_exposure_bnb web_bind web_port max_roundtrip_tax tax_cache_blocks allow_tax_inject executor_deadline_buffer_sec pairs_path pairs_reload_sec pairs_min_swap_bnb pair_scan_universal wallet_scan_enabled pair_scan_enabled scan_quote_usdt min_profit_usdt max_front_usdt min_reserve_usdt sim_engine tax_cache_ttl_sec front_slippage_bps back_slippage_bps pairs_vet_interval_sec pairs_require_vetted gas_units_front gas_units_back gas_price_max_gwei`

Cụm `real-economics-mode2` thêm 3 field: `gas_units_front`/`gas_units_back`
(số gas UNIT fallback cho front-buy/back-sell khi chưa đo được thật bằng
revm lúc boot trên 1 pair đã vet, ship `160000`/`140000`) và
`gas_price_max_gwei` (trần `eth_gasPrice` gwei nguyên, ship `10` — vượt thì
`gas_cap`). `front_max_gas_bnb_wei`/`back_max_gas_bnb_wei` ĐỔI Ý NGHĨA: chỉ
còn là TRẦN so với gas thật đo được (không còn dùng thẳng làm chi phí gas
trừ vào profit).

Đã bỏ (cụm 1): `executor_slippage_bps` → còn trong file = fail load với thông báo "đã đổi tên thành front_slippage_bps/back_slippage_bps". `tax_cache_blocks` giữ để không fail load nhưng KHÔNG dùng (TTL theo `tax_cache_ttl_sec`).

`.env` `BSC_HTTP`/`BSC_WS` cho phép nhiều URL (đa URL `_2`..`_16`, `_LIST` phẩy, hoặc chuỗi phẩy ngay trong biến gốc) — HTTP/WSS đều failover sang URL kế trong danh sách khi 1 node chết, không halt bot.

Ship (khớp `config.toml` trong repo — file đó là nguồn sự thật, mục này chỉ nêu các cờ quan trọng): `chain_id=56`, `dry_run=true`, `allow_live=false`, `bot_armed=false`, **`scan_v2=true scan_v3=true scan_v4=true`** (v4 = Infinity + bucket bản mới hơn), mọi `live_*=false`, `sim_engine="v2"` (cụm `strategy-lock-mode2` — đường nóng KHÔNG mở fork EVM mỗi tx, xem "Chiến lược đã chốt"), `min_profit_bnb=0.002` (paper), `max_front_bnb=5`, `max_exposure_bnb=5`, `min_reserve_wbnb=20`, `max_roundtrip_tax=0.005`, `tax_cache_ttl_sec=600`, `front_slippage_bps=10`, `back_slippage_bps=50`, `allow_tax_inject=true`, **`wallet_scan_enabled=false`** (mode 1 TẮT), `pair_scan_enabled=true` (mode 2 BẬT — nguồn candidate duy nhất), `pair_scan_universal=false` (mode 3 TẮT), `scan_quote_usdt=false`, `pairs_vet_interval_sec=600`, `pairs_require_vetted=true` (token chưa có `vetted` trong `pairs.txt` → không sim). Paper run (`scripts/paper_run.sh`) chỉ override NGƯỠNG về 0 + đổi port trong config TẠM (KHÔNG còn ép `pair_scan_universal=true`/`scan_quote_usdt=true`/`sim_engine="evm"` — 3 field đó giữ nguyên giá trị ship, đúng chiến lược mode 2 only), không sửa file ship. Zero-tax only: chủ đặt `max_roundtrip_tax=0`.

`min_profit_bnb max_front_bnb min_reserve_wbnb max_roundtrip_tax max_exposure_bnb` là ngưỡng chủ chỉnh tự do trong `config.toml`, KHÔNG hardcode trong Rust — sửa file, đợi tối đa `config_reload_sec` giây (hot-reload giống `victims_reload_sec`) là bot dùng số mới, không cần build/restart. Fail load CHỈ khi: thiếu field, `chain_id != 56`, 1 trong 5 field trên là số âm hoặc không hữu hạn (NaN/Infinity), hoặc parse lỗi — `min_profit_bnb=0`/`max_roundtrip_tax=0` và `max_front_bnb` rất lớn đều hợp lệ, không bị chặn biên trên.

`scan_*=true` mà family chưa pin → skip family đó + `venue_unpinned`, **không** tắt scan các family khác, không crash.

### Live

```
allow_live && !dry_run && bot_armed
&& không halt.lock && chain_id==56
&& live_* version đó && version đã pin && gas cap > 0
```

Chủ bật cờ là đủ. Không cần câu văn bản.  
`PRIVATE_TX_URL` rỗng = public. Cấm bịa relay. **Sandwich chỉ gửi dạng bundle nguyên tử `[front, victim_raw, back]`** qua relay hỗ trợ bundle (48 Club Puissant `eth_sendBundle`, BlockRazor `eth_sendMevBundle`); cấm gửi lẻ front/back qua RPC thường (audit F-01).

`state/halt.lock` **dừng cả paper loop** (không spawn, `halt.triggered` 1 lần, state `STOPPED`), không chỉ khóa cổng live (cụm 1, audit V-06). Xoá file → `halt.cleared`, chạy lại.

`.env`: `PRIVATE_KEY` `BSC_HTTP` `BSC_WS` optional `PRIVATE_TX_URL`. gitignore `.env state/ logs/ target/ key/`. `BSC_WS` phải là WSS có `newPendingTransactions` thật (publicnode); bot phải cảnh báo khi `pending_source != ws`.

---

## State / log

`IDLE → WATCHING → HIT → SIM_LOCK → LOGGED`  
Live: `SENDING_FRONT → SENDING_BACK`  
`STOPPED --reset--> IDLE`

`state/halt.lock` `disarm.req` `reset.req`

`logs/bot.jsonl`: `bot.start victim.reload pair.reload tx.seen tx.skip sim.evm sim.result tx.build build.refused validate.victim funnel.minute venue.pick tx.send tx.abort halt.triggered halt.cleared rpc.* tax.inject`. Mọi `tx.skip`/`sim.evm` phải có `hash to venue selector token quote amount_in` (cụm 2 bổ sung `amount_in`/`pair`/`reserve_quote`).

---

## Web — xem tất cả thông tin (bắt buộc, không phải phụ)

Một dashboard local để chủ + Grok nhìn đủ trạng thái, **không** thay CLI, **không** gửi tx.

- Bind mặc định `127.0.0.1:8787` (config `web_bind`, `web_port`). Không public 0.0.0.0 trừ khi chủ đổi.
- Stack: cùng binary Rust (`axum` + static) hoặc folder `web/` static đọc JSON API do bot serve. Cấm app Node riêng.
- Cấm hiện `PRIVATE_KEY`, URL RPC có token, `.env`. Redact giữa/cuối key.
- Chỉ đọc file + memory bot. Nút Halt/Disarm/Reset = **ghi file `state/*.req`**, không gọi signer.
- Paper và live cùng UI; badge to `DRY_RUN` / `LIVE_BLOCKED` / `LIVE_ARMED`.

### Một trang (scroll), đủ khối

1. **Bot** — state enum, uptime, last block, `chain_id`, dry_run, allow_live, bot_armed, halt.lock có/không.
2. **Cổng live** — từng điều kiện mục Live, tick xanh/đỏ (thiếu cái nào thì đỏ + chữ).
3. **Venue** — V2 / V3 / V4-Infinity / bản mới: pin? scan_? live_? getCode length? DISABLED + nguồn.
4. **Victims** — bảng address rút gọn, `min_swap_bnb`, lúc reload cuối, số dòng lỗi. Không sửa list trên web (sửa file).
5. **Hit sống** — 50 dòng jsonl mới nhất: from, min, amount_in, token, venue, pool, profit, decision, reason. Filter reason.
6. **Đếm skip** — đếm theo enum `not_in_list below_min decode_fail …` phiên hiện tại.
7. **Sim cuối** — front_in, victim_ok, profit_bnb, venue.pick.
8. **Builder** — `PRIVATE_TX_URL_*` có/không (boolean), không in URL. Public mempool nếu tất cả rỗng.
9. **Docs** — link tương đối `DEX_REGISTRY.md`, `docs/STATE.md`, BAOCAO số lớn nhất (tên file thôi).

API tối thiểu (JSON):

```
GET /api/health
GET /api/status      # state + flags + block + gate
GET /api/victims
GET /api/venues
GET /api/hits?limit=50
GET /api/skips
GET /api/pairs
GET /api/tax         # cache tax (token,quote,buy/sell bps,honeypot,TTL) ; POST inject
GET /api/funnel      # delta mỗi phút theo gate order thật
GET /api/validate    # validator nhúng: pred vs real, tách isolated / non-isolated
GET /api/compete      # cụm econ-truth-latency-vps — compete.check: tx liền kề victim có chạm cùng pool + gas so với victim
POST /api/control    # body {action: halt|disarm|reset} → ghi state file
```

Web làm cùng phiên với `0.3` (logger/state) hoặc ngay sau Gói A — **không để nợ** “lát web riêng sau live”. Mock data được khi chưa có WSS; có jsonl thì đọc file thật.

---

## Cây file — phiên đầu ĐƯỢC TẠO nếu thiếu

`CLAUDE.md Cargo.toml config.toml vps.json .env.example .gitignore .gitattributes README.md DEX_REGISTRY.md docs/STATE.md docs/TASKS.md docs/DOC_MAP.md docs/RUN.md baocao/ README victims.txt pairs.txt src/ web/ scripts/ key/ (gitignored)`

`victims.example.txt` ĐÃ XOÁ (cụm `docs-cleanup-mode2`, 2026-09-15) — mode 1
(wallet-mode) tắt mặc định, `victims.txt` giữ lại 1 dòng comment ghi rõ mode
1 đang tắt (định dạng cũ vẫn ghi trong comment đó, không cần file ví dụ
riêng).

`vps.json`: `chain_id=56`, RPC placeholder. Boot `eth_chainId==0x38`.

Đọc đầu phiên: CLAUDE.md → STATE → TASKS → REGISTRY → config → BAOCAO mới nhất (nếu có).

---

## Roadmap — được làm cùng lúc

Không nhảy **7.x live send** trước khi paper `4.1/5.1` có output. V4 không chặn V2.

`0.1` khung Rust + load  
`0.2` VictimBook  
`0.3` state + logger + **web dashboard** (`/`, `/api/*`)  
`1.1` pin V2 + WBNB + factory + router  
`1.2` pin V3 (bắt buộc trong cùng cụm registry)  
`1.3` pin V4/Infinity **và** family Pancake mới hơn trên BSC; không có code → DISABLED + nguồn, không cắt khỏi mục tiêu  
`2.1` HTTP/WSS  
`2.2` decoder mọi router Pancake đã pin (V2+V3+V4/latest + SmartRouter/UR path WBNB)  
`2.3` resolve pool token/WBNB mọi family đã pin  
`3.1` V2 math + search front  
`3.2` victim still ok  
`3.3` sim V3 + V4/latest (quoter đã pin); family chưa pin thì skip family, vẫn sim family đã pin  
`4.1` pipeline paper  
`5.1` chạy ngắn, 0 sendRaw  
`7.1` live gate + signer  
`7.2` pin calldata  
`7.3` executor — **CHỈ sau cụm 6 dưới đây**

### Sau audit độc lập 2026-09-15 (`baocao/BAOCAO_AUDIT_2026-09-15.md`) — thứ tự bắt buộc

Mã F-xx/V-xx trỏ tới bảng phát hiện trong file audit. Mỗi cụm = 1 lệnh = 1 commit = 1 BAOCAO.

- **Cụm 0** `wsl-env-rules-paperrun` — XONG (BAOCAO34, commit `e24a834`).
- **Cụm 1** `exec-path-traps` — F-26 tx.build sau EVM, F-06 từ chối `to=0x0`, F-07 back-sell theo balanceOf, F-04 record_result, F-05 version_pinned, F-08 slippage 2 field, F-13 nonce, F-14 deadline, F-15 to=None, F-16 venue cross-check, F-20 checked_add, V-06 halt dừng paper.
- **`strategy-lock-mode2`** — XONG (BAOCAO36) — Chủ CHỐT mode 2 only + vet tay + `sim_engine="v2"`, xem "Chiến lược đã chốt". Đổi tên/thứ tự 2 cụm dưới đây theo quyết định này.
- **Cụm 2** `real-economics-mode2` (ĐỔI TÊN từ `real-economics` — lý do: chiến lược đã chốt là V2 math + gas thật trên `pairs.txt` đã vet, không phải "EVM mỗi tx") — F-03 gas thật (`eth_gasPrice` × gas đo), histogram `victim_in` trên pair-mode, validator (đối chiếu dự đoán V2 vs thật), vet nền định kỳ (`pairs_vet_task`, đã có khung ở `strategy-lock-mode2`, cụm này đo/tinh chỉnh thật). **Kết quả cụm này quyết định chiến lược thực thi (sandwich vs backrun) trước khi làm cụm 6.**
- **Cụm 3** `fork-actor-perf` — **HẠ ƯU TIÊN xuống SAU cụm 6** (đổi từ vị trí cũ ngay sau cụm 2) — lý do: đường nóng mode 2 KHÔNG còn fork EVM mỗi tx (`strategy-lock-mode2` mục 3+4), nên fork-actor/backpressure chỉ còn phục vụ 3 việc nền/live (vet định kỳ, đo lại trước ký, validator) — không còn nghẽn hot path để tối ưu gấp. F-11 fork actor theo block (thread riêng, reset slot-level), F-12 backpressure, RPC riêng cho fork, timeout 500 ms, phân loại `sim_error`. Mục tiêu p50 < 50 ms, p95 < 500 ms, `sim_error` < 5% — áp dụng cho 3 việc nền/live đó, không phải đường nóng.
- **Cụm 4** `decoder-coverage` — GIỮ NGUYÊN vị trí — F-09 multicall, SmartRouter không deadline, UR đa lệnh, sentinel CONTRACT_BALANCE, payerIsUser. Mục tiêu `venue_v3 > 0`, `decode_fail < 2%`.
- **Cụm 5** `test-hygiene` — F-17 `real_rpc_*` không pass rỗng, F-22/F-23/F-25 dead code & doc, F-21 redact subdomain.
- **Cụm 6** `strategy-exec` — F-01 bundle `[front, victim, back]`, F-02 mô hình gas-price/bribe, executor contract nguyên tử (nếu chọn sandwich) HOẶC backrun-only (bỏ front leg). Chỉ bắt đầu sau khi Chủ chốt chiến lược bằng số liệu cụm 2. `fork-actor-perf` (cụm 3 cũ) làm SAU cụm này.
- **Deploy VPS**: theo commit hash, `sha256sum` binary ghi vào BAOCAO; VPS chạy unit systemd thật `Restart=always`, logrotate, SSH key-only, ufw chỉ 22. Chi tiết `docs/RUN.md` (đổi tên từ `docs/VPS_RUN.md`, cụm `strategy-lock-mode2`).

### Cùng phiên — khỏi nợ

Claude **được và nên** làm nốt việc dính nếu đang mở đúng module:

- Repo trống → `0.1+0.2+0.3` + web xem info + tạo cây file + example victims trong **một** phiên.
- Registry: `1.1+1.2+1.3` **một phiên**. Phải đụng cả V2, V3, V4/Infinity/mới nhất. Family không có getCode → DISABLED + URL đã mở, không được “cắt cho nhanh”.
- `2.1+2.2+2.3` một phiên.
- `3.1+3.2` một phiên; `3.3` làm luôn nếu đã pin.
- `4.1+5.1` một phiên khi 2–3 đã có trong repo.
- Test, logger, docs thiếu của module đang viết → làm luôn, không để “lát logger riêng”.
- Crate hot path (hashbrown, dashmap, alloy-pubsub) được thêm khi đang viết chỗ đó; ghi ô 3 BAOCAO.

**Cấm tự làm** dù cùng phiên: bật live / `dry_run=false` / `bot_armed`; `7.3` send thật; đổi stack; nhảy cụm (làm cụm 3 khi cụm 2 chưa CHỜ GROK); hạ ngưỡng số trong DoD của lệnh.

Một phiên = một BAOCAO, ô 1 ghi hết cụm (`0.1+0.2+0.3`). Kẹt RPC: làm nốt phần độc lập, phần kẹt ghi MISSING — không giả output.

Grok khi ra lệnh: giao cả cụm, đừng cắt 0.2 ra phiên khác nếu 0.1 chưa chạy.

---

## Mẫu lệnh Grok → copy sang Code

```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml, baocao mới nhất (nếu có).

LÁT: [cụm 0.1+0.2+0.3] — làm nốt việc dính trong cùng phiên (test/docs/logger của cụm này).
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/, docs/, baocao/, file khung, Cargo.toml, config.toml, vps.json
CẤM: CLAUDE.md, victims.txt thật, .env, cờ live

LÀM: [gạch]
KHÔNG LÀM: send thật, đổi stack, pair không-WBNB
NỢ: không tách test/logger ra phiên sau.

ĐẠT CẦN DÁN: cargo test/run + ≥15 dòng output; máy chạy (WSL/VPS) + sha256
binary; `git status --short` rỗng + `git log -1 --format="%H %ci"` SAU commit
cuối (luật #1/#2).

VIẾT: baocao/BAOCAO{NN}.md đủ 10 ô + dòng `Commit: <hash>`. Chữ: CHỜ GROK |
FAIL | CHƯA XONG. Cấm chữ ĐẠT.
```

Chủ → Grok:

```
BAOCAO{NN}
[dán]
Review. Đạt → khối cụm sau. Fail → cùng cụm, chỉ ô thiếu. Không viết lại repo.
```

---

## Pin dự kiến (chưa pin cho đến getCode)

- WBNB `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c`
- V2 Factory `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`
- V2 Router `0x10ED43C718714eb63d5aA57B78B54704E256024E` — xác nhận docs; sai = MISSING

V3 / V4 / Infinity / bản mới: pin khi registry có getCode. Không deploy BSC = DISABLED (có nguồn), không xóa khỏi mục tiêu — deploy sau thì pin lại.

---

BSC 56. RUST. GROK ĐIỀU HÀNH. CODE PHIÊN TRẮNG. DEV = WSL, PROD = VPS, CÙNG COMMIT.
CỤM CÙNG PHIÊN, KHÔNG NỢ VỤN. MỖI CỤM 1 COMMIT. SỐ LIỆU PHẢI CÓ MÁY + HASH.
VICTIMS.TXT `0x...,0.01`. TOKEN/WBNB HOẶC TOKEN/USDT. PANCAKE V2+V3+V4+MỚI NHẤT (PIN). UR PATH WBNB OK.
DRY-RUN. KHÔNG BỊA. KHÔNG TỰ LIVE.
MODE 2 ONLY. PAIRS VET TAY. HOT PATH V2 MATH + GAS THẬT. REVM = VET/PRE-SIGN/VALIDATOR.

