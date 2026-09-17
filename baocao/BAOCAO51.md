# BAOCAO51 — cụm `planB-B5-simarb-v3-measure`

## 1. LÁT

`planB-B5-simarb-v3-measure` — nối chân V3 (PCS QuoterV2 + Uniswap QuoterV2)
vào `sim_arb`, PairBook đọc `pairs_arb.txt` (list A both_ok đã vet), đo cơ
hội backrun-arb. Không contract, không live send. Máy WSL. Paper 60 phút
dừng sớm theo lệnh Chủ (cửa sổ 20.92 phút).

HEAD lúc mở: `af62ac99f9963edd2a77badb264f6e8fd2c7c455` (sau BAOCAO50).

Lệnh ghi `VIẾT: baocao/BAOCAO50.md` — file đó ĐÃ CÓ (cụm `planB-listA-vet`).
AGENTS.md: không đè. Phiên này viết **BAOCAO51**.

## 2. LỆNH NHẬN

Khối `planB-B5-simarb-v3-measure` mục 1→6. List arb = CHỈ both_ok. Bỏ luật
nới "pairs có V3 bất kỳ thì giữ". Không subagent ghi file. Không contract,
không live. Giữa phiên: “task 60 phút paper dừng lấy dữ liệu rồi báo cáo”.

## 3. FILE ĐỔI

Commit nội dung: điền hash ở dòng `Commit:` cuối file (luật #1).

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/sim_arb.rs` | sửa | `ArbVenue`/`MixedRoute`, `get_amount_out_v3`, fit 2-quote, search V2↔V3 / V3↔V3 |
| `src/sim_v3.rs` | sửa | `quote_exact_input_single_at` + `fit_arb_v3_pool` (≤2 eth_call / pool) |
| `src/sim_evm.rs` | sửa | hop V3 SwapRouter / SwapRouter02; nap dung `borrow` (không ×4) |
| `src/multivenue.rs` | sửa | `arb_mixed_venues` (≥2 venue, không đòi ≥2 V2) |
| `src/discover_mv.rs` | sửa | `keep_in_list = both_ok` only |
| `src/pairbook` via `src/main.rs` `src/config.rs` | sửa | `pairs_arb_path`; backrun đọc list A; vet nền theo PairBook |
| `src/venues.rs` | sửa | `V3_SWAP_ROUTER_ADDRESS`, `is_backrun_router` (+ Uni SwapRouter02) |
| `src/bin/arb_measure.rs` `arb_crosscheck.rs` | sửa | đo list A + kiểm chéo V3 |
| `src/bin/build_multi_venue.rs` | sửa | field `ok`/`both_ok`/… cho compile |
| `config.toml` `src/pipeline.rs` | sửa | `pairs_arb_path`, `gas_units_arb_v3=360000` |
| `AGENTS.md` | sửa | điểm 7 both_ok only + 2 field config |
| `docs/CONTRACT_DESIGN.md` | sửa | chân V3 SwapRouter / SwapRouter02 |
| `docs/STATE.md` `TASKS.md` `DOC_MAP.md` `RUN.md` | sửa | cụm B5; VPS 4 field `multivenue_*` + 2 field mới (chưa deploy) |
| `scripts/paper_run.sh` | sửa | in thống kê `pairs_arb.txt` |
| `baocao/BAOCAO51.md` | MỚI | file này |
| `baocao/evidence/baocao51_*` | MỚI | test, measure, crosscheck, paper 5'/20.9' |

**KHÔNG đụng**: `.env`, `pairs.txt`, `victims.txt`, cờ live, bot/config VPS.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
./target/release/arb_measure --jsonl logs/bot.jsonl.1 --pairs-arb pairs_arb.txt
./target/release/arb_crosscheck
scripts/paper_run.sh --minutes 5 --port 8795
scripts/paper_run.sh --minutes 60 --port 8796   # dung som ~21 phut
```

## 5. OUTPUT THẬT

**Máy: WSL.** `cargo test` + paper.

Binary paper `sha256sum target/release/bsc_sandwich` =
`6c4abc694925201d2643cd2bbce66d0a3e7bfdef8c217f00b211444ee4d6af8c`
(git HEAD lúc paper = `af62ac99`, working tree cụm này).

`arb_measure` =
`694c4c1b221fab6556a01c4c20ab317c4966b3049984d3f8f50dd636419b75e4`

`arb_crosscheck` (sau sửa nap borrow) =
`ebd59bf53509f74a3d1a65147f493c0878ba806dd29057b3a04809a28fef8b33`

```
--- cargo test --lib ---
test result: ok. 453 passed; 0 failed; 19 ignored; finished in 0.27s
--- cargo test --bin bsc_sandwich ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.22s
```

Lib 445 (BAOCAO49) → **453** (+ sim_arb V3, discover_mv keep, config 2 field,
multivenue mixed, venues backrun router).

```
--- /api/pairs paper 5 phut + 20.9 phut (strategy=backrun) ---
count=28 error_lines=0  (dung pairs_arb.txt, vetted 2026-09-17)
venue_unpinned=0  (V3 khong con skip VenueUnpinned)
```

### Kiểm chéo revm (mục 3)

7 case **hoàn thành** (math vs revm, nap dung `borrow` không ×4), lệch ≤2%:

| token | kind | pct_diff | gas | ok |
|---|---|---|---|---|
| CAT `0x6894cde3…` | v2_v3 | 0.9471 | 254470 | true |
| CAT | v3_v2 | 0.9565 | 253773 | true |
| BUSD `0xe9e7cea3…` | v2_v3 | 1.7500 | 338500 | true |
| BUSD | v3_v2 | 1.7241 | 366211 | true |
| Cheems `0x0df05872…` | v2_v3 | 0.4781 | 275105 | true |
| Cheems | v3_v2 | 0.4804 | 274124 | true |
| XVS `0xcf6bb538…` | v2_v3 | 1.0150 | 287678 | true |

`n_le_2pct=7/7` completed. `gas hops p50=275105`.
`gas_units_arb_v3=360000` (= 275k + ~80k overhead).

**Không đủ 15 case:** lần chạy sau `eth_getStorageAt` `-32000 not supported`
(cùng giới hạn RPC BAOCAO47). Token khác 4.7–7.5% (CPMM ảo vs CL thật) hoặc
FAIL RPC. Ghi MISSING phần còn lại, không bịa 15. File:
`baocao/evidence/baocao51_arb_crosscheck_7ok.txt`.

### Đo mục 4

**(a) log ≥18 h — máy: WSL, file `logs/bot.jsonl.1`**
cửa sổ `2026-09-15T06:06:14Z` → `2026-09-16T05:48:58Z` = **23.71 h**.
Lệnh không có host VPS → không SSH; dùng bản local ≥18 h.

V3 quote tại **block hiện tại** (RPC public không giữ 18 h state) — số dưới
là chênh V2/V3 lúc đo + victim amount lịch sử, không phải lãi backrun đúng
khối cũ.

```
FIT tokens=28 block=122351643 list_a=28
SUMMARY n_in_listA=28 n_opp=26 p50_usdt=26.8988 p90_usdt=189.4366
        sum_usdt=1508.37 borrow_p80=396.41 cluster_pct=0.0
        flash=infinity_vault
quy /ngay = 26 / 23.71 * 24 = 26.3 co hoi/ngay
```

On-chain +1..+3 sau victim (12 hash mẫu, `eth_getTransactionByHash` +
`eth_getBlockByNumber` full): **1/12 = 8.3%** tx kế tiếp là V2 Router.
Chi tiết trong phiên (không in URL RPC).

Bảng token × route × giờ (rút):

| token | route | giờ (UTC) | n | p50 USDT | sum |
|---|---|---|---|---|---|
| ETH | v2_v3 | 15T13 | 7 | 26.90 | 185.84 |
| ETH | v2_v3 | 15T14 | 4 | 26.95 | 107.92 |
| ETH | v2_v3 | 15T16 | 7 | 26.90 | 186.70 |
| ETH | v2_v3 | 15T17–18 | 3 | 26.90 | 79.97 |
| USDT | v3_v2 | 15T12/16/18 + 16T03 | 5 | 189.4 | 948.0 |

ETH 21/26 dòng lãi ~26.9 USDT gần như trùng — dấu standing arb V2/V3 tại
block quote, không phải impact victim.

**(b) WSL live mempool `strategy=backrun` list A**

Thử 5 phút trước (bắt buộc): `04:37:53Z`–`04:43:28Z`, port 8795.
`/api/pairs` 28. skips: `unprofitable=1` `below_min=2` `venue_unpinned=0`.
`sim.arb`: 1 simulated (token `4` v3_v3) + 1 unprofitable (USDT).

60 phút: Chủ ra lệnh dừng. Cửa sổ **20.92 phút**
`2026-09-17T04:44:07Z`–`05:05:02Z`, port 8796, `pending_source=ws`,
`last_block=122354373`, `halt.triggered` `05:04:59Z`.

```
/api/skips (truoc halt):
  decode_fail=4683  not_in_list=3893  not_quote_pair=313
  below_min=14  unprofitable=8  sanity_reject=1  venue_unpinned=0
sim.arb cua so 20.92 phut: n=11  simulated=3  unprofitable=8
flash infinity: WBNB=199.7  USDT=3.62e7  fee=0  (block 122353850)
```

3 dòng `simulated` (không dùng cho Go — cỡ vay > trần 20 BNB):

| ts | token | route | borrow BNB | net BNB |
|---|---|---|---|---|
| 05:02:27Z | 币安人生 | uni_v3→pcs_v3 | 46.47 | 4.92 |
| 05:03:43Z | 4 | pcs_v3→pcs_v3 | 40.19 | 2.66 |
| 05:04:16Z | 4 | pcs_v3→pcs_v3 | 40.19 | 2.66 |

`arb_max_borrow_bnb=20` mà borrow 40–46 → fit virtual reserve / search chưa
kẹp trần trên chân V3. `sanity_reject=1` không bắt hết. **Không kết luận lãi.**

8 dòng unprofitable = USDT-as-token v2_v3, net ≈ −0.00045 BNB (gas).

quy /ngày từ 3 simulated (nếu tin) = 3/20.92×24×60 ≈ 206 — **không tin**.
Nếu loại: **0 cơ hội/ngày** trên cửa sổ live.

File: `baocao/evidence/baocao51_paper5.out`, `baocao51_paper60_*`.

## 6. CHAIN

`eth_chainId=0x38` (paper `last_block=122354373`, measure fit block
`122351643`, crosscheck fork `122352274`). getCode venue V3 không đo lại
(đã pin BAOCAO02/48).

Flash snapshot paper (WSL): Infinity fee=0, WBNB≈199.7, USDT≈3.62e7.

## 7. REGISTRY

Không pin mới. Dùng: PCS QuoterV2 / SwapRouter, Uni QuoterV2 / SwapRouter02
(đã pin). `is_backrun_router` thêm Uni SwapRouter02; sandwich vẫn 5 Pancake.

## 8. KHÔNG LÀM

Không contract Solidity. Không sendRaw. Không deploy VPS. Không đè
`pairs.txt`. Không bật live. Paper 60 phút không chạy hết (lệnh dừng).
Không đủ 15 case revm (RPC `-32000`).

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

**Go/No-Go B1 (ô này, bằng số):** **No-Go.**

Ngưỡng: ≥30 cơ hội/ngày **và** p50 ≥5 USDT sau bribe, đo ≥6 h thật.

| nguồn | giờ | n cơ hội | /ngày | p50 USDT | đạt? |
|---|---|---|---|---|---|
| replay log 23.71 h (V3 quote hiện tại) | 23.71 | 26 | **26.3** | 26.90 | n<30; p50 bẩn standing arb |
| live WSL 20.92 phút | 0.349 | 3 sim không tin (vay>20 BNB) | 0 tin cậy | — | không |
| live 5 phút thử | 0.09 | 1 sim cùng kiểu | — | — | không |

Không đạt. Thiếu: (1) kẹp `arb_max_borrow_*` trên mixed V3; (2) quote V3 đồng
thời với tx (không fit ảo quá sâu); (3) cửa sổ live ≥6 h sau khi (1)(2);
(4) 15 case revm — cần node `getStorageAt` (BSC_HTTP_SIM archive).

Top gần ngưỡng (replay, không phải live tin cậy):

| # | token | route | n | p50 USDT | thiếu gì |
|---|---|---|---|---|---|
| 1 | ETH | v2_v3 | 21 | 26.9 | quote đồng khối; standing vs victim |
| 2 | USDT | v3_v2 | 5 | 189 | USDT-as-token; vay 14202 USDT |
| 3 | 4 | v3_v3 live | 2 | (2.66 BNB) | kẹp trần vay; sanity |
| 4 | 币安人生 | v3_v3 live | 1 | (4.92 BNB) | như trên |
| 5–10 | DOT ADA Cake BTCB LINK XVS | — | 0 live ≥0.5 BNB | — | chưa thấy swap đủ lớn trên list A |

Nợ khác: Infinity CL; 6 proxy FAIL list A; List B 75 theo dõi; VPS chưa
deploy — `config.toml` VPS cần `multivenue_*` (4) + `pairs_arb_path` +
`gas_units_arb_v3` trước binary này.

---

Commit: *(điền sau `git commit`)*
