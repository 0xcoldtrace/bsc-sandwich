# BAOCAO53 — cụm `planB-B7-paper-after-cap`

## 1. LÁT

`planB-B7-paper-after-cap` — đo lại mật độ `sim.arb` SAU B6 (search đã kẹp
`arb_max_borrow_bnb=20` / `arb_max_borrow_usdt=12000`). Máy WSL. Paper 60
phút, `dry_run=true`. Không contract, không live, không đụng unit `arc`.

HEAD lúc mở: `feac61ad66770cbb74aecad4e33f87bc8c850fe0` (BAOCAO52 điền hash).

## 2. LỆNH NHẬN

Khối `planB-B7-paper-after-cap`. Xác nhận sanity: `borrow > cap` →
`PipelineSkip::SanityReject`, không Simulated. Paper WSL ≥60 phút. Ghi
sha256 binary + git HEAD + `sim.arb` simulated / sanity_reject /
arb_no_second_venue / below_min + phân bố borrow (max ≤ cap) + profit sau
gas. SSH: không đụng unit `arc` cho tới khi Chủ xác nhận đã stop+disable.
Không xoá log `bot.jsonl.24h_*`. Không quét token mới vào `pairs_arb.txt`.
Cấm contract / sendRaw / sửa AGENTS chiến lược / `pairs.txt` / `victims.txt`
/ bật `bot_armed`/`allow_live`.

## 3. FILE ĐỔI

Commit nội dung: *(điền sau `git commit`)*

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/sim_arb.rs` | sửa | Test V2↔V3 / V3↔V3 / mixed quote siết `expect` quote (không `if let` nuốt None) |
| `src/main.rs` | sửa | `sim.arb` nhánh `unprofitable` thêm `borrow_quote` + `profit_before_bribe_wei` |
| `docs/STATE.md` | sửa | mục cụm B7 (không viết lại lịch sử) |
| `docs/TASKS.md` | sửa | hàng B7 trong bảng Kế hoạch B |
| `baocao/BAOCAO53.md` | MỚI | file này |
| `baocao/evidence/baocao53_*` | MỚI | cargo test, paper 60', skips, sim.arb, TSV 18 simulated |

**KHÔNG đụng**: `AGENTS.md` (còn dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
scripts/paper_run.sh --minutes 60 --port 8798
```

Sanity unit: `search_v2_v3_khong_vuot_tran_20_bnb`,
`search_v3_v3_khong_vuot_tran_20_bnb`,
`search_mixed_quote_kep_tran_dung_don_vi`,
`arb_borrow_40_bnb_over_cap_20_is_sanity_reject_not_simulated` — đều pass.
Không chạy 15 case revm (nợ RPC archive, ghi MISSING).

## 5. OUTPUT THẬT

**Máy: WSL.** `cargo test` + paper 60 phút (12:48:33Z–13:54:26Z).

Binary paper `sha256sum target/release/bsc_sandwich` =
`88d3f7824fded9ba26f6b2d7af46c7ddb95d4149316495ba6d299b2f31ad1496`
(git HEAD lúc paper = `feac61ad66770cbb74aecad4e33f87bc8c850fe0`, working
tree cụm này).

```
--- cargo test --lib --offline ---
test result: ok. 461 passed; 0 failed; 19 ignored; finished in 0.23s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

Fixture pipeline: vay 40 BNB / trần 20 → `PipelineSkip::SanityReject`, không
Simulated. Mixed V3 search `expect` quote + `borrow <= cap` pass.

```
== may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
== binary sha256 = 88d3f7824fded9ba26f6b2d7af46c7ddb95d4149316495ba6d299b2f31ad1496 ==
== git HEAD = feac61ad66770cbb74aecad4e33f87bc8c850fe0 ==
== pairs_arb.txt (list A, strategy=backrun): total=28 vetted=28 ==
== chay bot 60 phut, log -> logs/paper_run_1789649313.log (bot.jsonl tu dong 257026) ==
PID=1614548
```

Bot `/api/health` lúc chạy: `WATCHING`, `dry_run=true`, `allow_live=false`,
`bot_armed=false`, `live_mode=off`, `chain_id=56`.

```
--- /api/skips paper 60 phút (port 8798) ---
{"arb_no_flash_source":0,"arb_no_second_venue":1,"below_min":100,
 "decode_fail":12746,"not_in_list":15850,"not_quote_pair":1139,
 "sanity_reject":1,"unprofitable":136,"venue_unpinned":0,
 "competitor_victim":0,"gas_cap":0,"no_pool":0,"rpc_error":0}
```

```
--- sim.arb (mọi dòng, cửa sổ 60 phút, start_line=257026) ---
n=154  simulated=18  unprofitable=136  over_cap=0
route_kind: v2_v3=131  v3_v3=23
borrow WBNB n=136 min≈0 p50≈0 max=0.084906 BNB   (trần 20)
borrow USDT n=18  min=32.33 p50=36.10 max=195.06 USDT (trần 12000)
profit simulated (net sau gas+bribe, KHÔNG dòng over_cap):
  n=18 toàn USDT  sum=56.090562  p50=2.207098  max=12.042311
```

18 `simulated` toàn `v3_v3` quote USDT, flash `infinity_vault`,
`victim_in_competitor_cluster=false`: token `4` 14, LINK 3, Cake 1.
Borrow max 195 USDT << 12000. **0 dòng `sim.arb` `simulated` có borrow >
trần.**

`paper_run.sh` zero `min_profit_*`. Ship `min_profit_usdt=3.0` thì 14/18
(token `4`, net 1,75–2,28) sẽ `unprofitable`; còn 4/18 (3 LINK net 4,90 +
1 Cake net 12,04).

`sanity_reject=1`: BabyDoge V2 `amount_in≈1,16 BNB` /
`reserve_quote≈5978 BNB` — cửa reserve `arb_sanity_ok_mixed` (không phải
vượt `arb_max_borrow_*`; không log `sim.arb`).

```
tx.build=0 simulated=0 build.refused=0 halt.triggered=1
tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
DONE. may=WSL, binary sha256=88d3f7824fded9ba26f6b2d7af46c7ddb95d4149316495ba6d299b2f31ad1496
git HEAD=feac61ad66770cbb74aecad4e33f87bc8c850fe0
PAPER_EXIT=0
```

File: `baocao/evidence/baocao53_paper60.out`,
`baocao53_paper60_skips.json`, `baocao53_paper60_simarb.jsonl`,
`baocao53_paper60_simulated.tsv`, `baocao53_simarb_summary.txt`,
`baocao53_cargo_test.txt`.

## 6. CHAIN

Paper WSL `pending_source` WS publicnode; `funnel.minute` `seen` ~10k–25k
tx/phút. Không đo lại getCode (đã pin).

SSH VPS (Chủ dán host): **chỉ đọc**. Unit `arc.service` **không tồn tại** —
không start/stop/disable bất kỳ unit nào. Unit `bsc-sandwich-paper` đang
`active`/`enabled` nhưng cwd trên disk đã xóa (process còn sống từ inode
deleted). Không xoá `bot.jsonl.24h_*` (không thấy file đó). Không deploy
binary cụm này. Không ghi host/IP.

15 case revm: **MISSING** (RPC `eth_getStorageAt` `-32000`, nợ BAOCAO51).

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V2/V3, Uni V3, Infinity Vault flash).

## 8. KHÔNG LÀM

Không contract Solidity. Không sendRaw. Không deploy VPS. Không đụng unit
`arc` / `bsc-sandwich-paper`. Không đụng `.env` / `pairs.txt` /
`victims.txt` / `pairs_arb.txt` / cờ live. Không nới both_ok. Không sửa
AGENTS.md. Không 15 case revm. Không chạy `discover_multivenue` /
`vet_goplus` (không quét token mới).

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- Go/No-Go B1: cửa sổ 60 phút, p50 net 2,21 USDT < 5 USDT; kể cả lọc ship
  `min_profit_usdt=3` còn 4 case (p50 4,90). Fit V3 ảo (nợ B5/B6) chưa
  đối chiếu revm. **Không kết luận Go.** Cần ≥6 h nếu Điều hành muốn số
  cơ hội/ngày.
- 15 case revm — cần node `getStorageAt` (BSC_HTTP_SIM archive). MISSING.
- VPS: cwd paper đã xóa trên disk; unit `arc` không có; chưa deploy
  binary B7. Chờ Chủ xác nhận stop+disable trước khi đụng unit.
- `AGENTS.md` dirty local (đổi tên Điều hành Claude→Grok chat) — không
  stage trong cụm này.

---

Commit: *(điền sau git commit)*
