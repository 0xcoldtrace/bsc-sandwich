# BAOCAO59 — cụm `planB-B8f-audit-then-fix-sim`

## 1. LÁT

`planB-B8f-audit-then-fix-sim` — audit độc lập 2 case (CASE_V2V3 token-4
+ CASE_CAKE B8d) rồi sửa **một lần** cổng Simulated. Máy WSL. Không
contract, không live, không nới list, không đổi dấu `profit_paper`, không
xóa test B6 / CASE_CAKE cổng B8c, không paper 60'.

## 2. LỆNH NHẬN

Khối `planB-B8f-audit-then-fix-sim` BAOCAO59. Đọc AGENTS → STATE B8b–B8e
→ TASKS → BAOCAO56/57/58 ô 5 + TSV B8e → `sim_arb` / `sim_v3` / `sim_evm`
/ `arb_replay_18` / cổng quoter `main.rs` / `multivenue`. Audit xong 2
case **rồi mới** sửa `src`. Cổng B8c giữ (`ok=false`, quoter tuần tự).
Test `--offline` + fixture 2 hash. Replay đúng 2 hash. Ô 9 không ĐẠT.
Ô 10 cấm Go B1, cấm “đã có lãi”.

## 3. FILE ĐỔI

Commit nội dung: `69185915bcd6e89fa3fb1f90ec8914bc347db534`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/sim_v3.rs` | sửa | `quote_v2_path_at` + hop V2/bridge = `getAmountsOut` cùng block |
| `src/venues.rs` | sửa | `v2_router()` |
| `src/main.rs` | sửa | comment cổng B8f (không nới; không tháo `ok=false`) |
| `src/sim_arb.rs` | sửa | fixture 2 hash B8f (không xóa B6 / CASE_CAKE B8c) |
| `src/bin/arb_replay_18.rs` | sửa | `--hashes`, in hop + `net_quoter_now` |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8f |
| `docs/TASKS.md` | sửa | hàng B8f |
| `baocao/BAOCAO59.md` | MỚI | file này |
| `baocao/evidence/baocao59_*` | MỚI | audit, cargo, replay 2 hash |

**KHÔNG đụng**: `AGENTS.md` (dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS, `fit_v3_virtual_reserves`,
`if !p.ok { continue; }`.

## 4. LỆNH CHẠY

```
python3 baocao/evidence/baocao59_audit_hops.py
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo build --release --bin arb_replay_18 --offline
./target/release/arb_replay_18 --jsonl baocao/evidence/baocao57_paper60_simarb.jsonl \
  --hashes 0x657218fc…990f53,0x7ba67696…28cc1d
```

Cổng B8c còn khi mở phiên: `src/multivenue.rs` `if !p.ok { continue; }`;
`src/main.rs` `quote_mixed_hops_at` + `size_quote_allows_simulated`.

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
Audit RPC: host `bsc-mainnet.nodereal.io` (không in URL). Probe
`eth_getStorageAt` Cake slot 0 block `122450112`: PASS (`result` len 66).

Sự thật đã chốt (không đo lại Go): B8b 18/18 paper+ revm−; B8c
`QUOTER_KHAC_SWAP` + cổng `ok=false`; B8d 14 Simulated quoter+; B8e
14/14 quoter+ revm− `|lệch|>20%`, 0 revert, 0 missing.

---

### A. AUDIT ĐỘC LẬP (viết trước khi sửa `src`)

14/14 Simulated B8d cùng kiểu: **mua quote WBNB, bán pool V3 USDT, rồi
bridge USDT→WBNB**. `quote_mixed_hops_at` gọi QuoterV2 cho chân V3 nhưng
chân bridge dùng `get_amount_out` với reserve **snapshot**
`state/multi_venue.json` (block **122246764**, ~197k block / ~6,8 ngày
trước victim). `size_quote_net_wei` dương vì snapshot 710,11 USDT/WBNB;
revm swap V2 thật ~727,6 USDT/WBNB → âm. Cổng B8c không tháo.

`multi_venue` snapshot: `bridge_wbnb_usdt_pair=0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae`
`bridge_reserve_wbnb=53507083413731134572126`
`bridge_reserve_usdt=37996015600097767345798691`.

Paper `current_block` = `app_state.last_block` (newHeads / HTTP health) =
**block đã mine mới nhất** lúc thấy pending. **Không** phải `pending`,
**không** phải block victim, **không** phải `latest` lúc audit. Victim
vào block kế → paper quoter ≈ **parent**. Revm:
`open_fork` `AlloyDB` + `BlockId::number(victim receipt block)` =
**post-state block victim**, **không** apply raw victim (B8e).

File đo: `baocao/evidence/baocao59_audit.log`, `baocao59_audit.json`,
`baocao59_audit_hops.py`.

---

#### CASE_V2V3

Hash `0x657218fc2621df92676029346db880a163276d26d4baf15e88caeaf7d5990f53`
token `4` `0x0a43fc31a73013089df59194872ecae4cae14444` route `v2_v3`
borrow `2585473203347248311` (2.585473203347248311 BNB).

##### A1. Block

- receipt: block **122443922**, `tx_index` **104**, `status=0x1`
- `tx.to` = `0x10ed43c718714eb63d5aa57b78b54704e256024e` (PCS V2 Router)
- Quoter paper: `quote_mixed_hops_at(..., Some(current_block))` với
  `current_block = last_block` = **latest mined ≈ parent 122443921**
  (không pending, không victim, không latest lúc audit).
- Revm fork: `BlockId::number(122443922)` post-state, **không** apply
  victim raw.
- `victim_buys_token=false`. `pair_buy=pair_victim=0xf0a949d3d93b833c183a27ee067165b6f2c9625e`
  (V2 token/WBNB). `pair_sell=0xee04b2a82bab9efefcd626f5d66f51cc2b6fa12a`
  PCS V3 fee **2500** token/USDT (`ok=true`). On-chain `token0/token1/fee`
  khớp list. `flash=infinity_vault` premium 0.

USDT/WBNB: snap **710.111880** | parent **727.563943** | victim **727.563893**.

##### A2. Bảng hop (wei + BNB)

Hàng đúng route: hop1 mua V2 WBNB→token, hop2 bán V3 token→USDT fee 2500,
hop3 bridge V2 USDT→WBNB, flash, gas, bribe.

Cột PAPER_CPMM = đường nóng search + cổng B8c (V2 công thức / V3 fit /
bridge **snapshot**). QUOTER_eth_call = `getAmountsOut`/`quoteExactInputSingle`
tại **parent** (block paper). REVM = cùng eth_call tại **victim** (pool
khác nhau → độc lập = hop revm; net khớp TSV B8e).

Hop1 PAPER_CPMM tái từ `getReserves` parent + công thức V2 (jsonl không
lưu `token_out`) — khớp `getAmountsOut` parent.

| hàng | PAPER_CPMM | QUOTER_eth_call (parent) | REVM (victim) |
|---|---|---|---|
| hop1 V2 WBNB→token | 83934500854544042473364 (83934.500854544042473364) | 83934500854544042473364 (83934.500854544042473364) | 83934529616754722608221 (83934.529616754722608221) |
| hop2 PCS 2500 token→USDT | ≈ quoter (fit; jsonl không lưu) | 1856835127939649224385 (1856.835127939649224385) | 1856835760200432861333 (1856.835760200432861333) |
| hop3 bridge USDT→WBNB | **2608184439073185518 (2.608184439073185518) SNAP** | **2545623148891280862 (2.545623148891280862) chain** | **2545624192049708604 (2.545624192049708604) chain** |
| flash premium | 0 | 0 | 0 |
| gas | 360000000000000 (0.00036) | cùng paper | cùng paper (replay trừ gas jsonl) |
| bribe | 8940913652709132 (0.008940913652709132) | cùng | cùng |

`lãi_swap` (trước gas/bribe) = final − borrow − flash:

| | PAPER/cổng | QUOTER chain parent | REVM victim |
|---|---|---|---|
| lãi_swap | +0.022711235725937207 | **−0.039850054456027449** | **−0.039849011297539707** |
| lãi_net | +0.013410322073228075 | **−0.049150968108676581** | **−0.049149924950248839** |

Log B8d: `net_wei` paper +0.013411370479063696,
`size_quote_net_wei` +0.013411364926945322 (= hop1/2 parent + **SNAP** hop3,
sai số fit ~1e-9). TSV B8e `profit_revm` −0.049150 (= hop chain victim,
làm tròn 6 số).

##### A3. Khoản lệch chính

`|quoter_log − revm|` = `62561289877194161` wei (0.062561).
Hop3 SNAP − hop3 chain victim = `62561135078819022` wei = **99,997 %**
của |quoter−revm| (> 50 %).

Nhãn: **KHAC** (chân bridge snapshot, không eth_call cùng block-fork).

Không chọn: `HOP1_SAI` (parent≈victim, lệch ~3e-5 token), `HOP2_SAI`
(quoter khớp, lệch USDT ~6e-7), `FORK_SAI_BLOCK` (parent chain net
−0.049151 vs victim −0.049150 — không đủ 50 %), `QUOTER_SAI_BLOCK`
(hop1/2 đúng block paper; sai là **không quote hop3**), `FLASH_SAI` (0),
`GAS_BRIBE` (0.009301, swap đã âm trên chain), `DECODE_POOL` (pool USDT
đúng list + on-chain).

##### A4

Quoter dương vì chân bridge USDT→WBNB dùng reserve snapshot multi_venue
(710,11 USDT/BNB, block 122246764); revm âm vì `getAmountsOut` V2 thật
tại block victim (727,56 USDT/BNB) — parent chain đã âm.

---

#### CASE_CAKE

Hash `0x7ba67696c2fcd9c49cf921c5dd52097bedf24ce4c62d0c1a0cf854d9c928cc1d`
Cake `0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82` route `v3_v3`
borrow `20000000000000000000` (20 BNB). **Khác** CASE_CAKE USDT B8c
(`0xefabc7bf…`, bán pool 1 % `ok=false`).

##### A1. Block

- receipt: block **122450112**, `tx_index` **82**, `status=0x1`
- `tx.to` = `0x1b81d678ffb9c0263b24a97847620c99d213eb14` (PCS V3 SwapRouter)
- Quoter paper: `Some(current_block)` = **latest mined ≈ parent 122450111**.
- Revm: `BlockId::number(122450112)` post-state, không apply raw.
- `pair_buy=0xafb2da14056725e3ba3a30dd846b6bbbd7886c56` PCS V3 fee **500**
  Cake/WBNB `ok=true`. On-chain token0=Cake token1=WBNB fee=500.
- `pair_sell=0x7f51c8aaa6b0599abd16674e2b17fec7a9f674a1` PCS V3 fee **2500**
  Cake/USDT `ok=true`. On-chain token0=Cake token1=USDT fee=2500.
- `pair_victim=0x0ed7e52944161450477ee417de9cd3a859b14fd0` V2 Cake/WBNB.
  `victim_buys_token=false`. Flash Infinity premium 0.

USDT/WBNB: snap **710.111880** | parent **727.902898** | victim **727.902898**
(bridge parent == victim; victim không đụng pool WBNB/USDT).

##### A2. Bảng hop

Hop1 PAPER_CPMM / hop2 PAPER_CPMM = **MISSING** (jsonl không lưu
`token_out` / virtual reserve lúc paper). Net paper + SNAP hop3 khớp
quoter parent nên hop1+2 paper ≈ quoter parent.

| hàng | PAPER_CPMM | QUOTER_eth_call (parent) | REVM (victim) |
|---|---|---|---|
| hop1 PCS 500 WBNB→Cake | MISSING | 5951422212192607019709 (5951.422212192607019709) | 5951717958786851307760 (5951.717958786851307760) |
| hop2 PCS 2500 Cake→USDT | MISSING | 14475709425851465308056 (14475.709425851465308056) | 14476427969477887440537 (14476.427969477887440537) |
| hop3 bridge USDT→WBNB | **20326423527379767828 (20.326423527379767828) SNAP** | **19829689536224740174 (19.829689536224740174) chain** | **19830673469829505051 (19.830673469829505051) chain** |
| flash premium | 0 | 0 | 0 |
| gas | 360000000000000 (0.00036) | cùng | cùng |
| bribe | 10000000000000000 (0.01) | cùng | cùng |

| | PAPER jsonl | QUOTER parent+SNAP (= `size_quote_net_wei`) | QUOTER parent+chain | REVM victim |
|---|---|---|---|---|
| lãi_swap | +0.329784347243675688 | +0.326423527379767828 | **−0.170310463775259826** | **−0.169326530170494949** |
| lãi_net | +0.319424347243675688 | **+0.316063527379767828** | **−0.180670463775259826** | **−0.179686530170494949** |

`size_quote_net_wei` log = **316063527379767828** = đúng parent hop1/2
QuoterV2 + SNAP hop3. TSV B8e `profit_revm` −0.179687 = chain victim
(làm tròn).

##### A3. Khoản lệch chính

`|quoter_log − revm|` ≈ 0.495750 wei-BNB.
Hop3 SNAP parent − hop3 chain victim = 0.495750057550262777 = **~100 %**
của |quoter−revm| (> 50 %).

Nhãn: **KHAC** (cùng CASE_V2V3: bridge snapshot).

Không chọn: `HOP1_SAI` / `HOP2_SAI` (parent vs victim ~0,001 BNB net,
< 50 %), `FORK_SAI_BLOCK` (parent chain −0.180670 vs victim −0.179687),
`QUOTER_SAI_BLOCK` (hop1/2 đúng parent; sai là hop3 không eth_call),
`FLASH_SAI` (0), `GAS_BRIBE` (0.01036, swap chain đã âm), `DECODE_POOL`
(WBNB 500 + USDT 2500 đúng on-chain; không phải pool 1 % B8c).

##### A4

Quoter dương vì bridge USDT→WBNB dùng snapshot 710,11 USDT/BNB; revm âm
vì `getAmountsOut` thật 727,90 USDT/BNB tại block victim (parent chain
đã âm). Không phải `QUOTER_KHAC_SWAP` đảo CPMM (hop2 quoter khớp).

---

Cả 2 case: **cùng block paper (parent), quote hop3 on-chain → net < 0**.
Cổng B8c (`ok=false` + QuoterV2 hop V3) **không đủ** vì không quote bridge.

### B. FIX (sau A)

Sửa **một lần**: `quote_mixed_hops_at` hop V2 + chân bridge =
`eth_call getAmountsOut` tại **cùng** `block` với QuoterV2. Không dùng
CPMM snapshot `multi_venue`. Không sửa `fit_v3_virtual_reserves`. Không
tháo `if !p.ok { continue; }` (còn 2 chỗ `multivenue.rs`). Không đổi dấu
`profit_paper`. Test B6 `search_v2_v2_khong_vuot_tran_20_bnb` /
`search_v2_v3_khong_vuot_tran_20_bnb` và CASE_CAKE B8c
`case_cake_quoter_tuan_tu_am_khong_simulated` giữ.

Cổng hot path: `main.rs` vẫn `quote_mixed_hops_at` +
`size_quote_allows_simulated` trước Simulated. net≤0 / hop fail →
`unprofitable` / `sim_error`. Revm mỗi tx không gắn (chậm); cổng tối thiểu
= quoter đúng chiều đúng size đúng block + không Simulated khi hop fail
hoặc net≤0. Fixture 2 hash: chain/revm net≤0 → không Simulated.

```
--- cargo test --lib --offline ---
test result: ok. 472 passed; 0 failed; 19 ignored; finished in 0.24s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

472 = 468 BAOCAO58 + 4 test B8f (calldata `getAmountsOut`, CASE_V2V3,
CASE_CAKE WBNB, snapshot vs live reserve). Máy WSL. `git HEAD` lúc test
= `5b99bc40ad59a5df03b7b3cc9e2bdb0563e5d369` (trước commit cụm này).
`sha256sum target/release/arb_replay_18` (sau `cargo build --release
--bin arb_replay_18 --offline`) =
`66ab0c4a0d7d02ee812595be49bcac4bab224bff30d8f04e6e046f5b288c2b45`.
Binary `bsc_sandwich` không rebuild release (test `--offline` qua
`cargo test`).

Replay 2 hash (WSL, host `bsc-mainnet.nodereal.io`, fork victim post-state,
không apply raw). `net_quoter_now` = sequential sau fix (trừ gas+bribe
jsonl, cùng công thức revm).

| token | route | borrow | net_paper | net_quoter_log | net_quoter_now | profit_revm | lệch | gate |
|---|---|---|---|---|---|---|---|---|
| Cake | v3_v3 | 20.000000 | +0.319424 | +0.316064 | **−0.179687** | **−0.179687** | 0.0000% | GATE_NO_SIM |
| `4` | v2_v3 | 2.585473 | +0.013411 | +0.013411 | **−0.049150** | **−0.049150** | 0.0000% | GATE_NO_SIM |

Hop victim (wei) khớp audit ô 5 A:

- Cake: hop1 5951717958786851307760 Cake, hop2 14476427969477887440537 USDT,
  hop3 19830673469829505051 WBNB
- V2V3: hop1 83934529616754722608221 token, hop2 1856835760200432861333 USDT,
  hop3 2545624192049708604 WBNB

`SUMMARY n=2 n_unique_hash=2 n_ok=2 n_fail_lech=0 n_revert=0 n_missing=0`
`REPLAY_EXIT:0`. Revm **vẫn âm** — cổng chặn Simulated (test + `GATE_NO_SIM`).
Không còn Simulated giả trên 2 hash này. Không tuyên bố lãi.

File: `baocao/evidence/baocao59_audit.log`, `baocao59_audit.json`,
`baocao59_audit_hops.py`, `baocao59_cargo_test.txt`,
`baocao59_replay2.out`, `baocao59_replay2.tsv`.

## 6. CHAIN

`eth_getStorageAt` Cake slot 0 block `0x74c70c0` (122450112) PASS
NodeReal. Receipt 2 hash + `token0/token1/fee` 4 pool + QuoterV2 +
`getAmountsOut` parent/victim: số ô 5. Không pin mới. SSH VPS: **MISSING**.

## 7. REGISTRY

Không pin mới. QuoterV2 PCS `0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997`,
V2 Router `0x10ED43C718714eb63d5aA57B78B54704E256024E`, V3 SwapRouter
`0x1b81D678ffb9C0263b24A97847620C99d213eB14`, bridge V2 WBNB/USDT
`0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae` — đã pin, getCode cụm cũ.

## 8. KHÔNG LÀM

sendRaw, live, ArbExecutor, nới `pairs_arb`, paper 60', replay 18 hàng
B8b làm mẫu lãi, đổi dấu `profit_paper`, xóa test B6 / CASE_CAKE B8c,
tháo `if !p.ok`, `fit_v3_virtual_reserves` cho số dương, commit `.env` /
PAT / `AGENTS.md`, force-push, VPS.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

**Cấm Go B1. Cấm “đã có lãi”.** 14 Simulated B8d (paper+ SNAP) không phải
lãi on-chain. Sau B8f, 2 hash audit quoter chain = revm âm → không
Simulated; 12 hàng còn lại cùng kiểu bridge snapshot, chưa replay lại.
Cổng B8c (`ok=false`) giữ. Không paper 60' cụm này. Không tuyên bố mật
độ/lãi. Search `best_arb_for_venues` vẫn dùng CPMM snapshot để *tìm*
route — cổng trước Simulated mới quote chain; có thể còn log
`unprofitable` `size_quote_net_le_0`.

Commit: 69185915bcd6e89fa3fb1f90ec8914bc347db534
