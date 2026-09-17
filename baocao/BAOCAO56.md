# BAOCAO56 — cụm `planB-B8c-explain-v3-gap`

## 1. LÁT

`planB-B8c-explain-v3-gap` — giải thích paper dương / revm âm trên
CASE_CAKE và CASE_LINK (18 simulated BAOCAO53, replay archive BAOCAO55).
Máy WSL. Không contract, không live, không nới list, không Go B1.

HEAD lúc mở: `006e3cf4ff244709e6ebca9042d47fd8a2c6e757` (BAOCAO55 điền hash).

## 2. LỆNH NHẬN

Khối `planB-B8c-explain-v3-gap`. Đọc AGENTS → DOC_MAP → STATE (B5–B8b) →
TASKS → BAOCAO53/54/55 + TSV/OUT → `sim_arb.rs` → `arb_replay_18.rs` →
`CONTRACT_DESIGN.md` (chỉ đọc). Mổ CASE_CAKE + 1 hash CASE_LINK. Ô 5 đủ
C1–C6. Không sửa sim_arb cho ra số đẹp. Chỉ sửa code khi chỉ ra bug cụ
thể + test fixture CASE_CAKE/LINK: đường nóng không Simulated nếu revm
net < 0. Test `--offline`. Docs + evidence. Commit. Push PAT một lần
(không lưu remote). Ô 9 không ĐẠT. Ô 10 cấm Go B1, cấm “đã sửa xong lãi”.

## 3. FILE ĐỔI

Commit nội dung: `b2e04691ff359dfa363ea636541314d6a26993bd`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/sim_arb.rs` | sửa | `net_from_final_out` / `size_quote_allows_simulated`; search bỏ V3 `ok=false`; test CASE_CAKE/LINK |
| `src/sim_v3.rs` | sửa | `quote_mixed_hops_at` (QuoterV2 đúng chiều hop, đúng cỡ vay) |
| `src/multivenue.rs` | sửa | `arb_mixed_venues` bỏ V3/Uni `ok=false`; test CASE_CAKE/LINK |
| `src/main.rs` | sửa | trước Simulated: quoter tuần tự; net ≤ 0 → `unprofitable`; lỗi → `sim_error` |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8c |
| `docs/TASKS.md` | sửa | hàng B8c |
| `baocao/BAOCAO56.md` | MỚI | file này |
| `baocao/evidence/baocao56_*` | MỚI | C3 TSV, quoter log, fork snippet, recon, diag, cargo test |

**KHÔNG đụng**: `AGENTS.md` (dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
# diagnostic (không commit URL): eth_getTransactionReceipt +
# QuoterV2 quoteExactInputSingle tại parent và block victim
# + tái hiện fit 1 chiều + đảo CPMM (probe từ multi_venue bridge)
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`git HEAD` lúc test = `006e3cf4ff244709e6ebca9042d47fd8a2c6e757`.
Số liệu = `cargo test` trực tiếp + git HEAD (không rebuild
`target/release/bsc_sandwich` cụm này).

```
--- cargo test --lib --offline ---
test result: ok. 468 passed; 0 failed; 19 ignored; finished in 0.24s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

468 = 463 BAOCAO55 + 5 test B8c. Archive quoter: host
`bsc-mainnet.nodereal.io` PASS `eth_getStorageAt` Cake slot 0 block
`0x74beff9` (dòng SIM trùng key trong `.env`; không in URL). GetBlock
không dùng.

---

### CASE_CAKE — C1 Định danh

- `tx_hash` = `0xefabc7bfd29e81ed7897df19d18defaa89840f780a7307f5aec021c8d50ff779`
- `block_number` = **122418792**, `tx_index` = **36**, `status=0x1`
- `tx.to` = `0xd9c500dff816a1da21a48a732d3498bf09dc9aeb`
- token Cake `0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82`, quote USDT
  `0x55d398326f99059ff775485246999027b3197955`, `route_kind=v3_v3`
- `pool_a` (mua) `0x7f51c8aaa6b0599abd16674e2b17fec7a9f674a1` PCS V3
  fee **2500** (0,25 %), `ok=true`, impact list 0,006642 %,
  liquidity on-chain `8.294785690619085249511004e24`
- `pool_b` (bán) `0x55fe55677e8398ef5c92f01b780403c9a771a1c0` PCS V3
  fee **10000** (1 %), `ok=false`, impact list 32,735 %,
  liquidity on-chain `7.4105773214492950866e16` (~8 bậc nhỏ hơn pool mua)
- Nguồn địa chỉ pool: **`state/multi_venue.json`** (paper `pair_buy` /
  `pair_sell`); on-chain `token0/token1/fee()` khớp. Không compute CREATE2.
- `pair_victim` V2 WBNB `0x0ed7e52944161450477ee417de9cd3a859b14fd0`,
  `amount_in=3e18`, `victim_buys_token=false`.

### CASE_CAKE — C2 Fork revm

- Fork **tại block chứa victim** (`122418792`), **không** parent.
- `AlloyDB` + `BlockId::number(fork_block)` = **post-state** block đó.
- **Không** apply raw victim vào fork.
- Hàm: `arb_replay_18` → `fetch_block` (receipt) →
  `sim_evm::simulate_arb_mixed_hops_evm` → `open_fork`.
- Comment trên `simulate_arb_mixed_hops_evm` ghi “Fork `block-1`, (tuỳ
  chọn) chạy 1 swap victim V2 trước” — **sai so với thân hàm**.
- Đo: `slot0` + `liquidity` 2 pool V3 arb **parent == victim** (bit
  khớp). Victim không đụng 2 pool USDT. Snippet:
  `baocao/evidence/baocao56_fork_snippet.txt`.

```
// src/sim_evm.rs open_fork
let alloy_db = AlloyDB::<...>::new(provider, BlockId::number(fork_block));
// src/bin/arb_replay_18.rs
simulate_arb_mixed_hops_evm(p2, block, token, borrow_quote, sell_quote, borrow, buy, sell)
// không có victim_tx; net = final_out − borrow − flash − paper.gas − paper.bribe
```

### CASE_CAKE — C3 Bảng 2 chân (USDT / Cake, wei)

Hàng đúng thứ tự route: chân1 mua USDT→Cake, chân2 bán Cake→USDT.
`fee_paid = amount_in × fee / 1e6`.

| chân | amount_in | amount_out | fee_paid | nguồn |
|---|---|---|---|---|
| 1 mua PCS 2500 | 195.058899531652779389 USDT | 80.988348976735581076 Cake | 0.487647248829131948 USDT | PAPER = QUOTER (khớp) |
| 2 bán PCS 10000 | 80.988348976735581076 Cake | **207.111570832585772397 USDT** | 0.809883489767355811 Cake | PAPER = đảo CPMM |
| 2 bán PCS 10000 | 80.988348976735581076 Cake | **42.973630202155319675 USDT** | 0.809883489767355811 Cake | QUOTER block 122418791=122418792 |
| 2 bán | (cùng hop1) | **42.973630531652767101 USDT** | (cùng) | REVM implied `final_out` |

Tái hiện paper (probe 710.111880 USDT từ `multi_venue` bridge, fit 1
chiều USDT→token, đảo chân bán) net =
`12.042311300932993008` **==** `net_wei` BAOCAO53.

### CASE_CAKE — C4 Flash Infinity Vault

- `borrow_wei` = `195058899531652779389` (195.058899531652779389 USDT)
- `premium_wei` = 0 (`flash_fee_bps=0`, `flash_fee_wei=0`)
- `repay_wei` = borrow + 0
- Paper có trừ premium: có, số = 0. Revm có trừ: có, số = 0.
- `profit_paper` (`net_wei`) = **sau gas + bribe**.
  `gross_wei` = trước gas/bribe, sau flash.
- `profit_revm` BAOCAO55 = **sau gas + bribe paper** (replay không dùng
  gas revm đo được).

### CASE_CAKE — C5 Gas

- Paper: `gas_units_arb_v3=360000`; log `gas_wei=360000000000000`
  (0,00036 — victim quote WBNB nên không quy đổi USDT). `bribe_wei=0.01e18`.
- Revm: `_gas` discard; net dùng gas/bribe paper.
- `lãi_swap_revm` (trước gas) = `42.973630531652767101 − 195.058899531652779389`
  = **−152.085269 USDT** (đã âm).
- `lãi_net_revm` (sau gas+bribe) = **−152.095629 USDT** (TSV).
- Lỗi **không phải gas**.

### CASE_CAKE — C6 Nhãn

**QUOTER_KHAC_SWAP**

Số: hop1 khớp 80.988349 Cake; hop2 paper 207.111571 vs quoter/revm
42.973630 (lệch ~164 USDT). Paper đảo CPMM từ 2 quote USDT→token trên
pool 1% `ok=false` (liq mỏng) → bịa giá bán. Quoter đúng chiều
Cake→USDT tại đúng cỡ vay = revm = lỗ 152 USDT. Không chọn
`FORK_SAI_THỜI_ĐIỂM` (slot0 parent=victim). Không `THIEU_PHI_FLASH`
(premium 0). Không `GAS_TRON_LAI` (swap đã âm). Không `SAI_DECIMALS`
(USDT/Cake 18). `SAI_POOL_HOAC_FEE`: pool 1% `ok=false` là điều kiện
đi kèm, không phải nhãn chính của lệch số.

---

### CASE_LINK — C1 Định danh

Hash chọn: `0x0f9be5357e3c820ae8a9decebc786a7fd2c660008334f977bac347d5c148408a`
(cùng borrow 46.372956 / paper 4.902730 / revm −1.566826 với
`0x41370019…` và `0xdf3c6638…`).

- `block_number` = **122417145**, `tx_index` = **64**, `status=0x1`
- `tx.to` = `0x10ed43c718714eb63d5aa57b78b54704e256024e` (PCS V2 Router)
- token `0x924fa68a0fc644485b8df8abfa0a41c2e7744444` (multi_venue
  symbol `币安人生`; BAOCAO54 ghi LINK), quote USDT, `v3_v3`
- `pool_a` (mua) `0x75c5fbf77c1cd517544487aca4cc41e1ad95aced` **Uni V3**
  fee **3000**, `ok=false`, impact list 52,393932 %
- `pool_b` (bán) `0xa1ff9406219ffa6bcc3d89c2719dd91d231d4cee` PCS V3
  fee **10000**, `ok=true`, impact 0,224565 %
- Nguồn: **`multi_venue.json`**. On-chain fee khớp. Parent slot0 == victim.

### CASE_LINK — C2 Fork revm

Cùng cơ chế CASE_CAKE: fork post-state block 122417145, không apply
victim. Victim là V2 Router, không đụng 2 pool V3 USDT. slot0 parent ==
victim.

### CASE_LINK — C3 Bảng 2 chân

| chân | amount_in | amount_out | fee_paid | nguồn |
|---|---|---|---|---|
| 1 mua Uni 3000 | 46.372956426206550302 USDT | 109.205556823185435639 token | 0.139118869278619651 USDT | PAPER |
| 1 mua Uni 3000 | 46.372956426206550302 USDT | 95.427676590205946324 token | 0.139118869278619651 USDT | QUOTER |
| 2 bán PCS 10000 | 109.205556823185435639 token | **51.286046446566114970 USDT** | 1.092055568231854356 token | PAPER |
| 2 bán PCS 10000 | 95.427676590205946324 token | **44.816490241708360292 USDT** | 0.954276765902059463 token | QUOTER |
| 2 bán | (quoter hop1) | **44.816490426206550302 USDT** | (cùng) | REVM implied |

Paper recon net `4.902730020359564668` == BAOCAO53.

### CASE_LINK — C4 Flash Infinity Vault

- `borrow_wei` = `46372956426206550302`
- `premium_wei` = 0; paper trừ 0; revm trừ 0; repay = borrow
- `profit_paper` / `profit_revm` = sau gas+bribe (cùng C4 Cake)

### CASE_LINK — C5 Gas

Cùng `gas_wei=0.00036e18`, `bribe=0.01e18`.
`lãi_swap_revm` = `44.816490426 − 46.372956426` = **−1.556466 USDT**
(đã âm). `lãi_net_revm` = **−1.566826 USDT**. Không phải gas.

### CASE_LINK — C6 Nhãn

**QUOTER_KHAC_SWAP**

Số: paper net +4.902730 vs quoter/revm −1.556466 / −1.566826
(lệch 131,96 %). Hop1 paper 109.21 vs quoter 95.43; hop2 paper 51.29
vs 44.82. Cùng cơ chế đảo CPMM. Pool mua Uni 0,3 % `ok=false` (impact
52 %) là điều kiện đi kèm, không đổi nhãn chính.

File C3/quoter/fork: `baocao/evidence/baocao56_c3.tsv`,
`baocao56_quoter.log`, `baocao56_fork_snippet.txt`,
`baocao56_paper_recon.json`, `baocao56_diag.json`.

### Bug cụ thể + cổng (không đổi dấu paper)

1. `src/multivenue.rs` `arb_mixed_venues` — comment “V3/Uni `ok` = impact
   ≤ 2 %” nhưng vòng V3/Uni **không** `continue` khi `!p.ok`. CASE_CAKE
   bán 1 % và CASE_LINK mua Uni 0,3 % lọt search. Sửa: bỏ `ok=false`.
2. `src/sim_v3.rs` `fit_arb_v3_pool` + `sim_arb::hop_out` — fit 1 chiều
   quote→token, chân bán đảo CPMM. Tái hiện đúng số paper; quoter đúng
   chiều đúng cỡ = revm âm. Sửa: trước Simulated, `quote_mixed_hops_at`
   + `size_quote_allows_simulated` (net ≤ 0 → không Simulated). Không
   sửa `fit_v3_virtual_reserves` cho ra số đẹp. Không xóa test trần B6.

## 6. CHAIN

`eth_getStorageAt` Cake slot 0 block `0x74beff9` PASS (host
`bsc-mainnet.nodereal.io`, không in URL). Receipt + QuoterV2
`eth_call` tại 122417144/145 và 122418791/792: số ô 5. `token0/token1/fee`
2 pool khớp list. Không pin mới.

SSH VPS: **MISSING** (không host trong lệnh).

## 7. REGISTRY

Không pin mới. QuoterV2 PCS + Uni, Infinity Vault fee 0 — đã pin.

## 8. KHÔNG LÀM

Không Solidity / ArbExecutor. Không sendRaw. Không `discover_multivenue`.
Không sửa `pairs_arb.txt` / `pairs.txt` / `victims.txt` / cờ live /
AGENTS.md. Không paper 6h. Không đụng unit VPS. Không mở B9. Không
commit `.env` / PAT / URL có token. Không đổi dấu `profit_paper`.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- **Cấm Go B1.** 18/18 paper+ / revm− (BAOCAO55) vẫn là sự thật đã đo.
  56 USDT giấy không phải lãi.
- Cấm “đã sửa xong lãi”. Cổng chỉ **không Simulated** khi quoter tuần
  tự net ≤ 0 hoặc pool `ok=false`. Công thức paper CPMM vẫn ra cùng số
  dương — không tuyên bố fit V3 đã đúng.
- Chưa chạy paper 60' sau cổng (ngoài phạm vi cụm). V2-only không qua
  cổng quoter (đã đối chiếu EVM cụm cũ).
- VPS: không host. PAT GitHub dùng một lần lúc push, không ghi git
  config. Push: xem ô này sau khi chạy; không PAT trong lệnh Thợ gốc
  thì MISSING — lệnh này **có** PAT ở cuối khối.

---

Commit: `b2e04691ff359dfa363ea636541314d6a26993bd`
