# BAOCAO60 — cụm `planB-B8g-paper-after-live-bridge`

## 1. LÁT

`planB-B8g-paper-after-live-bridge` — paper dry-run ≥ 60 phút SAU cổng
B8f (hop V2 + chân bridge = `getAmountsOut` cùng block, không snapshot
`multi_venue`). Máy WSL. Không contract, không live, không nới
`pairs_arb`, không replay revm (simulated=0).

HEAD lúc mở: `f8df735121e101dbff52a248edc0439846ac4f1c` (BAOCAO59 điền
hash). Nhánh có BAOCAO59 / `6918591`. `origin/master` khớp sau `git fetch`.

## 2. LỆNH NHẬN

Khối `planB-B8g-paper-after-live-bridge` BAOCAO60. Đọc AGENTS → STATE B8f
→ TASKS → BAOCAO59 ô 5 (A+B) → `main.rs` cổng `quote_mixed_hops_at`.
Xác nhận cổng còn. `cargo test --lib/--bin --offline`. Rebuild release.
`scripts/paper_run.sh --minutes 60`. Bảng ô 5 bắt buộc. simulated=0 thì
ghi vậy, không bia cửa. Ô 9 không ĐẠT. Ô 10 cấm Go B1, cấm “đã có lãi”.
Commit + push PAT một lần (không lưu remote).

## 3. FILE ĐỔI

Commit nội dung: `075c089574a0a343bafd6e9d16afddcf21f227b8`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8g |
| `docs/TASKS.md` | sửa | hàng B8g |
| `baocao/BAOCAO60.md` | MỚI | file này |
| `baocao/evidence/baocao60_*` | MỚI | paper 60', sim.arb jsonl, skips, cargo, analyzer |

**KHÔNG đụng**: `AGENTS.md` (dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS, `src/` (cổng B8f giữ).

## 4. LỆNH CHẠY

```
git fetch && git log -1
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo build --release --bin bsc_sandwich --offline
scripts/paper_run.sh --minutes 60 --port 8798
python3 baocao/evidence/baocao60_analyze_simarb.py logs/bot.jsonl 418121
```

Cổng xác nhận trước paper: `src/sim_v3.rs` `quote_v2_path_at` /
`quote_mixed_hops_at` hop V2 + bridge = `getAmountsOut` cùng block;
`src/multivenue.rs` + `src/main.rs` `if !p.ok { continue; }` còn;
`size_quote_allows_simulated` trước `decision=simulated`.

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`sha256sum target/release/bsc_sandwich` =
`4fd55f8a82065f256b4396319144c3a1340189cb6bc48de9566b5b20fab0f1a1`
`git HEAD` lúc paper = `f8df735121e101dbff52a248edc0439846ac4f1c`

Boot: `chain_id=56 dry_run=true allow_live=false bot_armed=false`.
Web `127.0.0.1:8798`. `pairs_arb.txt` total=28 vetted=28.
`paper_run.sh` zero `min_profit_*` (boot: `min_profit_bnb=0`) — **không**
gọi Simulated là lãi (cửa sổ này simulated=0).

Cổng B8f còn khi mở phiên: `quote_v2_path_at` / hop3 `getAmountsOut`;
`quote_mixed_hops_at` không lấy `bridge_reserve` snapshot để net
Simulated; `if !p.ok { continue; }` còn 3 chỗ.

```
== may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
== binary sha256 = 4fd55f8a82065f256b4396319144c3a1340189cb6bc48de9566b5b20fab0f1a1 ==
== git HEAD = f8df735121e101dbff52a248edc0439846ac4f1c ==
== pairs_arb.txt (list A, strategy=backrun): total=28 vetted=28 ==
== chay bot 60 phut, log -> logs/paper_run_1789706191.log (bot.jsonl tu dong 418121) ==
PID=2188670
bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
halt.triggered count (lan chay nay) = 1
tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
tx.build=0 simulated=0 build.refused=0 halt.triggered=1
PAPER_EXIT=0
```

Cửa sổ jsonl start_line=418121, ts `2026-09-18T04:36:31Z`–
`2026-09-18T05:43:13Z` (~67 phút gồm boot + halt; vòng `paper_run.sh`
60 phút). `funnel.minute simulated` là sandwich `sim.evm`, không phải
`sim.arb` (cả hai = 0).

```
--- cargo test --lib --offline ---
test result: ok. 472 passed; 0 failed; 19 ignored; finished in 0.27s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

```
--- /api/skips paper 60 phút (port 8798, curl trước halt) ---
{"arb_no_flash_source":0,"arb_no_second_venue":0,"below_min":201,
 "decode_fail":11763,"not_in_list":12354,"not_quote_pair":1780,
 "sanity_reject":4,"unprofitable":52,"sim_error":0,"venue_unpinned":0,
 "competitor_victim":0,"gas_cap":0,"no_pool":0,"rpc_error":0}
```

jsonl cùng cửa sổ (vài skip sau lúc curl `/api/skips`, trước halt):
below_min=202, decode_fail=11769, not_in_list=12371, not_quote_pair=1782,
sanity_reject=4, unprofitable=52.

```
--- sim.arb (mọi dòng, cửa sổ 60 phút, start_line=418121) ---
n=52  simulated=0  unprofitable=52  sim_error=0  over_cap=0
route_kind: v2_v2=0  v2_v3=51  v3_v3=1
borrow WBNB n=52 min≈0 p50≈0 max=7.754174 BNB   (trần 20)
borrow USDT n=0
reason size_quote_net_le_0 = 18  (nằm trong unprofitable)
simulated_missing_quoter=0  simulated_quoter_le_0=0
```

**Bảng ô 5 bắt buộc**

| sim.arb | simulated | unprofitable | sim_error | size_quote_net_le_0 | below_min | decode_fail | not_in_list |
|---|---|---|---|---|---|---|---|
| 52 | 0 | 52 | 0 | 18 | 202 | 11769 | 12371 |

Route: `v2_v2=0` / `v2_v3=51` / `v3_v3=1`.

**simulated = 0** — không bia cửa. Không hàng
`token / route / borrow / net_paper / net_quoter_now / hash`. Không
FAIL cổng (thiếu `net_quoter_now` chỉ FAIL khi simulated>0).

18 `unprofitable` `reason=size_quote_net_le_0`: search CPMM snapshot
dương, `quote_mixed_hops_at` chain net≤0 → không Simulated. Token:
`4` 15 hàng, 币安人生 2, 我踏马来了 1; toàn `v2_v3`. TSV
`baocao/evidence/baocao60_gate_le0.tsv`. 34 hàng còn lại paper `net_wei≤0`
trước cổng. 1 hàng `v3_v3` USDT paper đã âm. Không replay revm.

File: `baocao/evidence/baocao60_paper60.out`,
`baocao60_paper60_simarb.jsonl`, `baocao60_simarb_summary.txt`,
`baocao60_paper60_skips.json`, `baocao60_paper60_api_skips.json`,
`baocao60_gate_le0.tsv`, `baocao60_cargo_test.txt`,
`baocao60_analyze_simarb.py`.

## 6. CHAIN

Không gọi RPC lịch sử cụm này (cổng sống mempool). Boot `chain_id=56`.
SSH VPS: **MISSING** (lệnh cấm). Không pin mới.

## 7. REGISTRY

Không pin mới. Cổng dùng V2 Router + QuoterV2 đã pin cụm cũ.

## 8. KHÔNG LÀM

sendRaw, live, ArbExecutor, nới `pairs_arb`, paper 6h, replay revm,
`discover_multivenue`, `fit_v3` cho số dương, tháo `ok=false`, đổi dấu
`profit_paper`, commit `.env` / PAT / `AGENTS.md`, force-push, VPS.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

**Cấm Go B1. Cấm “đã có lãi”.** Paper 60' sau cổng B8f: simulated=0.
18 hàng search dương bị `getAmountsOut` hop3 chặn (`size_quote_net_le_0`).
14 Simulated B8d (SNAP) không phải lãi on-chain. Cổng B8c (`ok=false`)
giữ. Search `best_arb_for_venues` vẫn CPMM snapshot để *tìm* route —
cổng trước Simulated mới quote chain. Không tuyên bố mật độ/lãi.
Không Go B1.

Commit: 075c089574a0a343bafd6e9d16afddcf21f227b8
