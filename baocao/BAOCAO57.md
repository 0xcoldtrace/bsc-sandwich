# BAOCAO57 — cụm `planB-B8d-paper-after-quoter-gate`

## 1. LÁT

`planB-B8d-paper-after-quoter-gate` — paper dry-run ≥ 60 phút SAU cổng
B8c (bỏ V3 `ok=false` + QuoterV2 đúng cỡ vay trước Simulated). Máy WSL.
Không contract, không live, không nới `pairs_arb`, không đổi CPMM.

HEAD lúc mở: `192b428b487c9b65be29889d2556b54a7bce3c46` (BAOCAO56 điền
hash). Nhánh có BAOCAO56 / `b2e0469`. `origin/master` khớp sau `git fetch`.

## 2. LỆNH NHẬN

Khối `planB-B8d-paper-after-quoter-gate`. Đọc AGENTS → STATE → TASKS →
BAOCAO55 → BAOCAO56 ô 5. Xác nhận cổng còn. `scripts/paper_run.sh
--minutes 60`. Đếm `sim.arb`. Nếu simulated > 0: từng hàng token / route /
borrow / net_paper / net_quoter — thiếu cột quoter = FAIL. Test
`--offline`. BAOCAO57 10 ô. Ô 9 không ĐẠT. Ô 10 cấm Go B1. Commit + push
PAT một lần (không lưu remote).

## 3. FILE ĐỔI

Commit nội dung: `761be543092e9c69efe328f8621b2e01d5bdadfb`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/main.rs` | sửa | Simulated log thêm `size_quote_net_wei` / `size_quote_applied`; `sim.arb` `sim_error` khi quoter RPC lỗi |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8d |
| `docs/TASKS.md` | sửa | hàng B8d |
| `baocao/BAOCAO57.md` | MỚI | file này |
| `baocao/evidence/baocao57_*` | MỚI | paper 60', sim.arb jsonl/TSV, skips, cargo test, analyzer |

**KHÔNG đụng**: `AGENTS.md` (dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS.

## 4. LỆNH CHẠY

```
scripts/paper_run.sh --minutes 60 --port 8798
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
python3 baocao/evidence/baocao57_analyze_simarb.py logs/bot.jsonl 327143
```

Cổng xác nhận trước paper: `src/multivenue.rs` `if !p.ok { continue; }`
(V3 + Uni); `src/main.rs` `quote_mixed_hops_at` +
`size_quote_allows_simulated` trước `decision=simulated`.

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`sha256sum target/release/bsc_sandwich` =
`4e793c643d1c16a0855561e20f830157925b91c60a69512a0065fc07551af7a0`
`git HEAD` lúc paper = `192b428b487c9b65be29889d2556b54a7bce3c46`
(working tree có log B8d, binary rebuild từ tree đó).

Boot: `chain_id=56 dry_run=true allow_live=false bot_armed=false`.
Web `127.0.0.1:8798`. `pairs_arb.txt` total=28 vetted=28.
`paper_run.sh` zero `min_profit_*` (boot: `min_profit_bnb=0`) — **không**
gọi Simulated là lãi.

```
== may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
== binary sha256 = 4e793c643d1c16a0855561e20f830157925b91c60a69512a0065fc07551af7a0 ==
== git HEAD = 192b428b487c9b65be29889d2556b54a7bce3c46 ==
== pairs_arb.txt (list A, strategy=backrun): total=28 vetted=28 ==
== chay bot 60 phut, log -> logs/paper_run_1789661530.log (bot.jsonl tu dong 327143) ==
PID=1745696
bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
PAPER_EXIT=0
```

Cửa sổ jsonl start_line=327143, ts `2026-09-17T16:12:10Z`–
`2026-09-17T17:17:44Z` (~65 phút gồm boot; vòng `paper_run.sh` 60 phút).

```
--- cargo test --lib --offline ---
test result: ok. 468 passed; 0 failed; 19 ignored; finished in 0.29s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.22s
```

```
--- /api/skips paper 60 phút (port 8798) ---
{"arb_no_flash_source":0,"arb_no_second_venue":0,"below_min":108,
 "decode_fail":19989,"not_in_list":18472,"not_quote_pair":1484,
 "sanity_reject":1,"unprofitable":68,"sim_error":0,"venue_unpinned":0,
 "competitor_victim":0,"gas_cap":0,"no_pool":0,"rpc_error":0}
```

jsonl cùng cửa sổ (một vài skip sau lúc curl `/api/skips`, trước halt):
below_min=108, unprofitable=68, sanity_reject=1, decode_fail=19997,
not_in_list=18494, not_quote_pair=1485.

```
--- sim.arb (mọi dòng, cửa sổ 60 phút, start_line=327143) ---
n=82  simulated=14  unprofitable=68  sim_error=0  over_cap=0
route_kind: v2_v3=81  v3_v3=1
borrow WBNB n=82 min≈0 p50≈0 max=20.000000 BNB   (trần 20)
borrow USDT n=0
reason size_quote_net_le_0 = 2  (nằm trong unprofitable)
simulated_missing_quoter=0  simulated_quoter_le_0=0
```

Bảng 14 `simulated` (quote WBNB, flash `infinity_vault`,
`size_quote_applied=true`, 14 unique hash). Đơn vị BNB.
`net_paper` = `net_wei` paper; `net_quoter` = `size_quote_net_wei`.

| token | route | borrow | net_paper | net_quoter | hash |
|---|---|---|---|---|---|
| 币安人生 `0x924fa68a…4444` | v2_v3 | 1.989860 | 0.005752 | 0.005752 | `0xbc42244d…b97013` |
| `4` `0x0a43fc31…4444` | v2_v3 | 2.585473 | 0.013411 | 0.013411 | `0x657218fc…990f53` |
| `4` | v2_v3 | 2.585506 | 0.013412 | 0.013412 | `0x403cc445…b98569` |
| `4` | v2_v3 | 2.454692 | 0.012053 | 0.012053 | `0xce7aafe7…fc8e5a` |
| `4` | v2_v3 | 2.502680 | 0.012558 | 0.012558 | `0xc71f5666…ec859b` |
| 币安人生 | v2_v3 | 1.975102 | 0.005664 | 0.005664 | `0x02896c83…c061d8` |
| `4` | v2_v3 | 2.400094 | 0.011538 | 0.011538 | `0x85d57a7c…25c530` |
| 我踏马来了 `0xc51a9250…4444` | v2_v3 | 2.600822 | 0.010795 | 0.010795 | `0xc0020ff6…6b5c30` |
| `4` | v2_v3 | 2.535789 | 0.012845 | 0.012849 | `0x73a4958a…fd6a15` |
| `4` | v2_v3 | 2.490468 | 0.012371 | 0.012375 | `0x2799839c…1f8481` |
| `4` | v2_v3 | 2.517964 | 0.012597 | 0.012600 | `0xcdb08900…1534c4` |
| `4` | v2_v3 | 2.590105 | 0.013384 | 0.013396 | `0x493cd4d9…5ab5fb` |
| Cake `0x0e09fabb…ce82` | v3_v3 | 20.000000 | 0.319424 | 0.316064 | `0x7ba67696…28cc1d` |
| 币安人生 | v2_v3 | 1.959313 | 0.005570 | 0.005570 | `0x216d3205…47edc6` |

14/14 có cột quoter, net_quoter > 0. **Không FAIL** thiếu cột.
`paper_run.sh` zero `min_profit` — **không** gọi 14 hàng này là lãi.
Ship `min_profit_bnb=0.002`: 14/14 net_paper > 0.002 (vẫn chỉ là số
paper+quoter, chưa revm).

2 hàng cổng quoter chặn (paper+ / quoter− → `unprofitable`):

| token | route | borrow | net_paper | net_quoter | hash |
|---|---|---|---|---|---|
| SKYAI `0x92aa0313…ffb10` | v2_v3 | 0.243269 | 0.001929 | −0.002028 | `0x4a3ff0eb…97fb38` |
| SKYAI | v2_v3 | 0.195856 | 0.001426 | −0.000642 | `0xbe5d0fe6…f1779b` |

`sanity_reject=1`: BabyDoge `0xc7486730…e8de` V2 `amount_in≈0.837 BNB` /
`reserve_quote≈5994 BNB` — cửa reserve (không vượt `arb_max_borrow_*`;
không log `sim.arb`). Hash `0x6952fe64…db5a41`.

Cake Simulated borrow **20.000 BNB** = trần B6, `over_cap=0` (kẹp trong
search). pair_buy `0xafb2da14…`, pair_sell `0x7f51c8aa…` (pool 2500
`ok=true`). Khác CASE_CAKE USDT (bán pool 1% `ok=false`). Không replay
revm hàng này ở cụm này.

`funnel.minute` `simulated=0` = bucket `sim.evm` sandwich, **không** phải
`sim.arb`. `tx.build=0 simulated=0` (paper_run đếm `sim.evm`)
`build.refused=0 halt.triggered=1` `tx.seen` sau halt = 0.

```
tx.build=0 simulated=0 build.refused=0 halt.triggered=1
tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
DONE. may=WSL, binary sha256=4e793c643d1c16a0855561e20f830157925b91c60a69512a0065fc07551af7a0
git HEAD=192b428b487c9b65be29889d2556b54a7bce3c46
PAPER_EXIT=0
```

File: `baocao/evidence/baocao57_paper60.out`,
`baocao57_paper60_api_skips.json`, `baocao57_paper60_skips.json`,
`baocao57_paper60_simarb.jsonl`, `baocao57_paper60_simulated.tsv`,
`baocao57_simarb_summary.txt`, `baocao57_cargo_test.txt`,
`baocao57_analyze_simarb.py`.

## 6. CHAIN

Paper WSL `pending_source` WS publicnode (`bsc-rpc.publicnode.com`, không
in URL). `/api/tax` `current_block=122452019`. Không đo lại getCode.

SSH VPS: **MISSING** (không host trong lệnh). Không start/stop/deploy.

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V2/V3, Uni V3, Infinity Vault flash,
QuoterV2).

## 8. KHÔNG LÀM

Không Solidity / ArbExecutor. Không sendRaw. Không `discover_multivenue`.
Không sửa `pairs_arb.txt` / `pairs.txt` / `victims.txt` / cờ live /
AGENTS.md. Không paper 6h. Không đụng unit VPS. Không sửa
`fit_v3_virtual_reserves`. Không đổi dấu `profit_paper`. Không commit
`.env` / PAT / URL có token.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- **Cấm Go B1.** 14 Simulated có quoter net > 0 **không** phải lãi
  on-chain / bundle được chọn. Cửa Go (≥30 cơ hội/ngày và p50 ≥ 5 USDT
  sau bribe, ≥6 h) không đo ở cụm này. p50 net_paper 14 hàng ≈ 0.0126 BNB
  (cửa sổ 60', `min_profit` paper = 0).
- 18/18 B8b paper+ / revm− vẫn là sự thật đã đo. Cụm này **không** replay
  revm 14 hàng mới.
- Cấm “đã sửa xong lãi”. Cổng chỉ không Simulated khi quoter net ≤ 0
  hoặc pool `ok=false`.
- `AGENTS.md` dirty local — không stage.
- VPS: không host. PAT GitHub dùng một lần lúc push, không ghi git config.

---

Commit: `761be543092e9c69efe328f8621b2e01d5bdadfb`
