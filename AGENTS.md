# AGENTS.md — BSC MEV BOT (backrun-arb, flash 0 phí)

Bạn là **Thợ** (hiện tại: Grok; trước 2026-09-16 là Claude Code, khi đó file này tên `CLAUDE.md` — các BAOCAO cũ nhắc `CLAUDE.md` là nhắc file này). **Mỗi phiên là trắng.** Không nhớ chat cũ. Không nói “như lát trước”. Chỉ tin file repo + khối lệnh lần này. Luật trong file này không đổi theo người làm Thợ.

## Ai làm gì

| Ai | Việc |
|---|---|
| **Điều hành** = **Claude** (trong chat của Chủ) | Người điều hành duy nhất. Ra khối lệnh tự chứa, đọc BAOCAO, ĐẠT/FAIL, ra lệnh tiếp. Không đụng code. Trong file này mọi chỗ ghi "Điều hành" là Claude. |
| **Chủ** | Copy lệnh Claude → Thợ. Copy BAOCAO / lỗi → Claude. Điền `.env`, `pairs.txt`, khóa ví, cờ live, deploy VPS, chạy tool vet. Không tự ĐẠT. |
| **Thợ** = **Grok** (agent code, phiên trắng) | Làm hết cụm trong lệnh; **được kéo thêm việc dính liền trong cùng phiên** để khỏi nợ lát. Một file BAOCAO. Không điều hành. Không sửa file này trừ khi lệnh bảo sửa. Không tự ĐẠT. |

Chủ đưa BAOCAO cho Claude là đủ. Thợ đọc repo, không đọc chat của Chủ.

Máy: **dev = WSL `~/bsc-sandwich`** (Thợ chạy ở đây, ext4 native, không qua `/mnt/c`). **Production = VPS** (host + user do Chủ dán trong khối lệnh; key `key/bsc_vps_ed25519`; **KHÔNG ghi IP vào file/BAOCAO**). VPS chỉ nhận commit đã qua paper run trên WSL; hai máy phải cùng git commit, lệch = MISSING. `config.toml` trên VPS là file RIÊNG — binary mới có field mới thì phải cập nhật `config.toml` VPS CÙNG LÚC, nếu không bot VPS fail load (bài học BAOCAO46).

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
- Pair: token/WBNB hoặc token/USDT. Chiến lược hiện tại là **backrun-arb** (xem
  "Chiến lược đã chốt"): bot KHÔNG đứng trước victim, chỉ đứng ngay SAU swap
  lớn và cân bằng giá giữa ≥2 pool của cùng token. Đường sandwich cũ giữ code,
  tắt bằng `strategy="backrun"`. USDT
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

Uniswap V3 BSC = venue arb giai đoạn 2, pin trước, sim sau. Cấm: steal approval, honeypot drain. Flashloan CHỈ từ nguồn đã pin trong `DEX_REGISTRY.md` (xem "Chiến lược đã chốt"), trả trong cùng tx, không lãi thì revert. Hook không đọc được → skip **pool đó**, không tắt bot, không bỏ family.

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
`bugfix-presign-and-contract-plan` A2/A3, ghi vào AGENTS.md ở cụm
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

## Chiến lược đã chốt

### 2026-09-16 — KẾ HOẠCH B: BACKRUN-ARB bằng flash loan 0 phí (đang hiệu lực)

Căn cứ (BAOCAO43–45, số thật trên VPS): sandwich victim người-thật ≈ 0 lãi
(victim đặt slippage 1–5 % → `front_in` bị ép, lãi ~0,00002 BNB/case); 96,8 %
lãi mô phỏng là kẹp ví burner của cụm bot `0xB406…` — cần chen trước victim,
3 000 USDT vốn xoay, cửa sổ ngắn. Chủ CHỐT bỏ sandwich.

1. **Chiến lược = backrun-arb nguyên tử.** Sau MỌI swap lớn (≥ 0,5 BNB quy
   đổi, kể cả của cụm `0xB406`) trên token có **≥ 2 venue**: vay flash → mua
   pool rẻ → bán pool đắt (+ chân USDT↔WBNB nếu khác quote) → trả nợ → giữ
   chênh, tất cả trong 1 tx; không lãi → revert (mất gas, không mất bribe nếu
   bundle không được chọn). Không có `victim_ok`, không cần vốn xoay.
2. **Nguồn flash** (đọc on-chain TRƯỚC mỗi lần dùng, không giả định; pin đủ
   trong `DEX_REGISTRY.md`): (1) PancakeSwap Infinity Vault
   `0x238a358808379702088667322f80aC48bAd5e6c4` — `lock()` → `lockAcquired` →
   `take` → arb → trả token + `sync` + `settle`, delta = 0, phí 0, trần vay =
   `balanceOf(vault)` (KHÔNG phải `reservesOfApp`, BAOCAO46); (2) Aave V3 Pool
   `0x6807dc923806fE8Fd134338EABCA509979a7e0cB`, 5 bps; (3) Pancake V2 flash
   swap `pancakeCall`, ~25,06 bps; (4) Balancer V2 Vault
   `0xBA12222222228d8Ba445958a75a0704d566BF2C8` phí 0 nhưng trên BSC gần
   RỖNG (0,0004 WBNB, BAOCAO46) + wind-down vote 25–29/9/2026 → không chờ.
   `flash::choose_flash_source` chọn nguồn rẻ nhất còn đủ sâu tại block.
3. **Vị trí sau victim lấy qua builder**, không đua latency: 48 Club Puissant
   `eth_sendBundle` (`https://puissant-builder.48.club/`, không auth,
   `backrunTarget` = hash victim, bribe = transfer BNB tới
   `0x4848489f0b2BEdd788c696e2D79b6b69D7484848`); BlockRazor Block Builder
   (`https://virginia.builder.blockrazor.io`, header `Authorization`, bribe =
   transfer BNB tới `0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20`). Bundle =
   `[victim_raw, backrun_tx]`. Bribe chỉ chuyển khi tx thành công.
4. **Đường nóng** giữ nguyên nền mode 2: `pairs.txt` vet tay, decoder, Sync
   cache, presign không fork, V2 math (đã được EVM xác nhận sát trên mẫu lớn
   — BAOCAO45). Thêm: `state/multi_venue.json` (token → các pool), `sim_arb`
   (route 2–3 chân, cỡ vay tối ưu, chi phí = phí pool + phí flash + gas +
   bribe), `flash_source_task`. `strategy = "backrun"` (ship);
   `"sandwich"` giữ code, tắt.
5. **Go/No-Go cho contract (B1)**: ≥ 30 cơ hội/ngày VÀ p50 lãi ≥ 5 USDT sau
   bribe, đo trên ≥ 6 giờ thật (B0). Không đạt → mở venue thứ 2 (Infinity CL /
   V3) trước, không viết contract.
6. **Vốn**: ví tay chỉ giữ BNB cho gas + bribe; lãi về ví kho ngay trong tx;
   contract không giữ vốn.
7. **List đa venue (sửa `planB-B5-simarb-v3-measure`).** List arb = CHỈ
   token **both_ok** (V2 đủ ngưỡng VÀ V3 impact ≤ 2 % cùng quote — PCS V3
   **hoặc** Uniswap V3). File: `pairs_arb.txt` (`config pairs_arb_path`),
   PairBook + vet nền đọc khi `strategy="backrun"`. Nhóm "có V3 nhưng mỏng"
   là list theo dõi (`baocao/evidence/baocao50_listB_watch.txt`), **KHÔNG
   sim, không kết luận**. Bỏ luật nới "pairs.txt có V3 bất kỳ thì giữ".
   Venue: PCS V2 + PCS V3 + Uniswap V3 BSC. KHÔNG THENA, KHÔNG Biswap.
   Ngưỡng V2: `reserve_quote` ≥ 50 BNB (≥ 35.000 USDT). Nguồn volume: top
   500 token Swap log PCS V2+V3 (`getLogs` dải 1000 block). Tool:
   `discover_multivenue --hours N --pairs pairs.txt`. Output
   `state/multi_venue.json` + TSV + `state/multi_venue_candidates.txt`.

### 2026-09-15 — mode 2 (nền, vẫn áp dụng cho phần lọc/vet)

1. Chỉ pair-mode (`pairs.txt`); mode 1/3 tắt bằng cờ, không xoá code.
2. Token do Chủ VET (Tool 1 `discover_v2` → Tool 2 `vet_bsc_token` → điền
   `vetted YYYY-MM-DD`; xem `docs/GHICHU_VET.md`); bot vet lại nền bằng revm
   (`pairs_vet_task`); chưa vet → không sim.
3. Đường nóng không đo tax mỗi tx; `sim_engine="v2"`.
4. `revm` giữ 3 việc: vet nền, kiểm tra trước ký, validator; không fork per-tx.

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

4. **Subagent chỉ được ĐỌC.** Thợ không giao cho subagent/fork bất kỳ việc
   ghi/xoá/git nào. Subagent chỉ để đọc và tóm tắt; mọi thay đổi file do
   phiên chính làm và ghi vào BAOCAO.
5. **Ngừng an toàn khi Chủ yêu cầu.** Nhận lệnh "dừng an toàn" → commit
   phần đã làm, viết BAOCAO chữ `CHƯA XONG`, ô 10 liệt kê ĐỦ mục chưa làm
   (không bỏ im lặng mục nào), không để file dở giữa chừng.

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

Claude (Điều hành) ĐẠT khi có ô 5. Thợ không viết ĐẠT.

---

## Config — thiếu field = fail load

`chain_id dry_run allow_live bot_armed scan_v2 scan_v3 scan_v4 live_v2 live_v3 live_v4 min_profit_bnb max_front_bnb min_reserve_wbnb victims_path victims_reload_sec config_reload_sec pending_poll_ms pending_txpool_max_per_poll gas_reserve_bnb_wei front_max_gas_bnb_wei back_max_gas_bnb_wei tx_timeout_sec ws_silence_sec max_consecutive_loss max_exposure_bnb web_bind web_port max_roundtrip_tax tax_cache_blocks allow_tax_inject executor_deadline_buffer_sec pairs_path pairs_arb_path pairs_reload_sec pairs_min_swap_bnb pair_scan_universal wallet_scan_enabled pair_scan_enabled scan_quote_usdt min_profit_usdt max_front_usdt min_reserve_usdt sim_engine tax_cache_ttl_sec front_slippage_bps back_slippage_bps pairs_vet_interval_sec pairs_require_vetted gas_units_front gas_units_back gas_price_max_gwei bribe_pct_of_profit bribe_min_bnb bribe_max_bnb bribe_mode live_mode allow_competitor_victims strategy arb_max_borrow_bnb arb_max_borrow_usdt flash_source_interval_sec multi_venue_path gas_units_arb_infinity gas_units_arb_v2flash gas_units_arb_v3 multivenue_min_v2_bnb multivenue_min_v2_usdt multivenue_min_v3_impact_pct multivenue_probe_bnb`

Danh sách trên là nguồn sự thật THỨ HAI; nguồn thứ nhất là `src/config.rs` — thêm field ở code thì thêm vào đây và vào `config.toml` cả WSL lẫn VPS cùng lúc. Cụm `planB-backrun-opportunity` (BAOCAO46) thêm 7 field; cụm `planB-B4-multivenue-tool` (BAOCAO48) thêm 4 field ngưỡng list đa venue (`multivenue_min_v2_bnb`/`multivenue_min_v2_usdt`/`multivenue_min_v3_impact_pct`/`multivenue_probe_bnb`). `gas_units_arb_*` hiện là ước lượng thô, CHƯA đo revm (nợ B0). VPS phải cập nhật 4 field mới **trước** khi deploy binary BAOCAO48 (thiếu = fail load).

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
`PRIVATE_TX_URL` rỗng = public. Cấm bịa relay. **Mọi tx MEV chỉ gửi dạng bundle** qua builder đã pin: backrun = `[victim_raw, backrun_tx]` (48 Club `backrunTarget`, BlockRazor bundle); sandwich (đã tắt) = `[front, victim_raw, back]`. Bribe = transfer BNB tới EOA builder trong chính tx, chỉ khi thành công. Cấm gửi lẻ qua RPC thường (audit F-01). `live_mode`: `off` | `shadow` (ký, không gửi) | `live`. `.env` thêm `BLOCKRAZOR_AUTH` (optional), `BSC_HTTP_SIM`, `BSC_HTTP_BG`.

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
GET /api/compete     # compete.check + cluster_rate (cụm đối thủ 0xB406)
GET /api/econ        # bucket victim_in, top_pools, net_pos_non_cluster, victim_ok 2 cổng
GET /api/mem         # RSS + kích thước mọi container sống lâu (chống OOM)
GET /api/flash       # (B0) chiều sâu + phí 4 nguồn flash tại block
POST /api/control    # body {action: halt|disarm|reset} → ghi state file
```

Web làm cùng phiên với `0.3` (logger/state) hoặc ngay sau Gói A — **không để nợ** “lát web riêng sau live”. Mock data được khi chưa có WSS; có jsonl thì đọc file thật.

---

## Cây file — phiên đầu ĐƯỢC TẠO nếu thiếu

`AGENTS.md Cargo.toml config.toml vps.json .env.example .gitignore .gitattributes README.md DEX_REGISTRY.md docs/STATE.md docs/TASKS.md docs/DOC_MAP.md docs/RUN.md baocao/ README victims.txt pairs.txt src/ web/ scripts/ key/ (gitignored)`

`victims.example.txt` ĐÃ XOÁ (cụm `docs-cleanup-mode2`, 2026-09-15) — mode 1
(wallet-mode) tắt mặc định, `victims.txt` giữ lại 1 dòng comment ghi rõ mode
1 đang tắt (định dạng cũ vẫn ghi trong comment đó, không cần file ví dụ
riêng).

`vps.json`: `chain_id=56`, RPC placeholder. Boot `eth_chainId==0x38`.

Đọc đầu phiên: AGENTS.md → STATE → TASKS → REGISTRY → config → BAOCAO mới nhất (nếu có).

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

### Sau audit độc lập 2026-09-15 — ĐÃ XONG (BAOCAO34–45)

Cụm 0 → `exec-path-traps` → `strategy-lock-mode2` → `real-economics-mode2` →
`decoder-universal-router` → `econ-truth-latency-vps` → `competitor-recon` →
`presign`/`bugfix` → `decision-data-24h` → `truth-victim-ok-and-memleak` →
`verify-cluster-as-victim`. Kết quả: hạ tầng đo được sự thật; sandwich bị
loại bằng số (xem "Chiến lược đã chốt"). Nợ còn: `/api/econ` O(log), p95 nhóm
đi tới sim 848 ms, `sim_engine="evm"` dùng trần gas, tỉ lệ thắng cuộc đua.

### KẾ HOẠCH B (từ 2026-09-16) — thứ tự bắt buộc, mỗi cụm 1 lệnh = 1 commit = 1 BAOCAO

- **B0 `planB-backrun-opportunity`** — ĐANG LÀM (BAOCAO46 CHƯA XONG: xong
  `flash.rs`, `sim_arb.rs`, 7 field config; CHƯA: `multi_venue.json`,
  `flash_source_task`, kiểm chéo revm ≥20 case, bảng đo ≥6 h, gas_units đo
  thật, registry pin, ArbExecutor design, Go/No-Go).
- **B1 `arb-executor-contract`** — chỉ khi B0 Go. Solidity 0.8, không proxy, 4
  entry flash vào 1 `_route`, `minProfit` revert, bribe khi thành công,
  `onlyExecutor`, owner withdraw/pause, không giữ vốn. Foundry fork test.
  **Audit độc lập vòng 2 trước deploy.**
- **B2 `arb-exec-wiring`** — bot: swap lớn → route → ký 1 tx → bundle
  `[victim_raw, backrun]` → `bundle.result` (API trạng thái 48 Club) →
  RiskGuard. Shadow 24 h.
- **B3 `live-small`** — ví tay 0,2 BNB gas+bribe, không vốn giao dịch. Đo tỉ
  lệ bundle được chọn + lãi thật/ngày, 1 tuần. Đây là con số thật đầu tiên.
- **B4 `venue-infinity-v3`** — mở venue thứ 2/3 (Infinity CL, V3 tier) khi B0
  chỉ ra token thiếu venue; decoder PoolKey/hooks.
- **Deploy VPS**: theo commit, `sha256` binary, unit systemd `Restart=always`
  + `MemoryMax`, logrotate; `config.toml` VPS cập nhật cùng binary.

### Cùng phiên — khỏi nợ

Thợ **được và nên** làm nốt việc dính nếu đang mở đúng module:

- Repo trống → `0.1+0.2+0.3` + web xem info + tạo cây file + example victims trong **một** phiên.
- Registry: `1.1+1.2+1.3` **một phiên**. Phải đụng cả V2, V3, V4/Infinity/mới nhất. Family không có getCode → DISABLED + URL đã mở, không được “cắt cho nhanh”.
- `2.1+2.2+2.3` một phiên.
- `3.1+3.2` một phiên; `3.3` làm luôn nếu đã pin.
- `4.1+5.1` một phiên khi 2–3 đã có trong repo.
- Test, logger, docs thiếu của module đang viết → làm luôn, không để “lát logger riêng”.
- Crate hot path (hashbrown, dashmap, alloy-pubsub) được thêm khi đang viết chỗ đó; ghi ô 3 BAOCAO.

**Cấm tự làm** dù cùng phiên: bật live / `dry_run=false` / `bot_armed`; `7.3` send thật; đổi stack; nhảy cụm (làm cụm 3 khi cụm 2 chưa CHỜ GROK); hạ ngưỡng số trong DoD của lệnh.

Một phiên = một BAOCAO, ô 1 ghi hết cụm (`0.1+0.2+0.3`). Kẹt RPC: làm nốt phần độc lập, phần kẹt ghi MISSING — không giả output.

Điều hành khi ra lệnh: giao cả cụm, đừng cắt 0.2 ra phiên khác nếu 0.1 chưa chạy.

---

## Mẫu lệnh Điều hành (Claude) → Chủ copy sang Thợ (Grok)

```
ĐỌC: AGENTS.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml, baocao mới nhất (nếu có).

LÁT: [cụm 0.1+0.2+0.3] — làm nốt việc dính trong cùng phiên (test/docs/logger của cụm này).
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/, docs/, baocao/, file khung, Cargo.toml, config.toml, vps.json
CẤM: AGENTS.md, victims.txt thật, .env, cờ live

LÀM: [gạch]
KHÔNG LÀM: send thật, đổi stack, contract khi B0 chưa Go
NỢ: không tách test/logger ra phiên sau.

ĐẠT CẦN DÁN: cargo test/run + ≥15 dòng output; máy chạy (WSL/VPS) + sha256
binary; `git status --short` rỗng + `git log -1 --format="%H %ci"` SAU commit
cuối (luật #1/#2).

VIẾT: baocao/BAOCAO{NN}.md đủ 10 ô + dòng `Commit: <hash>`. Chữ: CHỜ GROK |
FAIL | CHƯA XONG. Cấm chữ ĐẠT.
```

Chủ → Claude:

```
BAOCAO{NN}
[dán]
Review. Đạt → khối cụm sau. Fail → cùng cụm, chỉ ô thiếu. Không viết lại repo.
```

Lưu ý cho Thợ là Grok: đọc `docs/DOC_MAP.md` trước để biết thứ tự file; mọi
lệnh shell chạy trong WSL `~/bsc-sandwich`; VPS chỉ qua `ssh -i key/bsc_vps_ed25519 root@<host Chủ đưa>`; không có host trong lệnh → phần VPS ghi MISSING.

---

## Pin dự kiến (chưa pin cho đến getCode)

- WBNB `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c`
- V2 Factory `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`
- V2 Router `0x10ED43C718714eb63d5aA57B78B54704E256024E` — xác nhận docs; sai = MISSING

V3 / V4 / Infinity / bản mới: pin khi registry có getCode. Không deploy BSC = DISABLED (có nguồn), không xóa khỏi mục tiêu — deploy sau thì pin lại.

---

BSC 56. RUST. CLAUDE ĐIỀU HÀNH, GROK LÀ THỢ, PHIÊN TRẮNG. DEV = WSL, PROD = VPS, CÙNG COMMIT.
CỤM CÙNG PHIÊN, KHÔNG NỢ VỤN. MỖI CỤM 1 COMMIT. SỐ LIỆU PHẢI CÓ MÁY + HASH.
KẾ HOẠCH B: BACKRUN-ARB, FLASH 0 PHÍ (INFINITY VAULT), BUNDLE QUA BUILDER, KHÔNG SANDWICH.
PAIRS VET TAY. HOT PATH V2 MATH + GAS THẬT. REVM = VET/KIỂM/VALIDATOR. KHÔNG CONTRACT TRƯỚC KHI B0 GO.
DRY-RUN. KHÔNG BỊA. KHÔNG TỰ LIVE.