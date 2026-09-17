# BAOCAO52 — cụm `planB-B6-cap-borrow-v3`

## 1. LÁT

`planB-B6-cap-borrow-v3` — kẹp search `sim_arb` TRONG trần
`arb_max_borrow_*` (đúng quote chân vay) cho mọi route V2↔V2 / V2↔V3 /
V3↔V3 / mixed quote. Cổng `sanity_reject` thêm 2 cửa trần vay. Máy WSL.
Không contract, không live, không SSH VPS.

HEAD lúc mở: `187f10562f447e0f0b9dac27f73f231445242436` (README sau BAOCAO51).

## 2. LỆNH NHẬN

Khối `planB-B6-cap-borrow-v3` Việc 1–3. Search trong trần, không search xong
rồi mới nhìn. Reason vượt trần: `sanity_reject` (nhất quán). Cấm contract /
sendRaw / deploy / đổi list / nới both_ok / paper 6h. Không đụng `.env` /
`pairs.txt` / `victims.txt` / cờ live / config VPS. AGENTS.md không sửa
(không thêm skip reason mới).

## 3. FILE ĐỔI

Commit: `837a08f5f0359edf7c5ac2ee7311d750844dbdd3`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/sim_arb.rs` | sửa | `clamp_borrow` + `search_borrow_capped`; ternary V2/mixed kẹp trước mỗi mid; `accept_quote` loại `borrow > max`; lưới 5 điểm kẹp; test 4 kiểu route + 40/20 |
| `src/pipeline.rs` | sửa | `sanity_check_arb_borrow` + `arb_borrow_sanity_skip`; 2 cửa trần vay cạnh 3 cửa reserve cũ; fixture 40 BNB / trần 20 → `sanity_reject` |
| `src/main.rs` | sửa | `handle_backrun_tx` gọi `arb_borrow_sanity_skip` trước Simulated |
| `docs/STATE.md` | sửa | mục cụm B6 (không viết lại lịch sử) |
| `docs/TASKS.md` | sửa | hàng B6 trong bảng Kế hoạch B |
| `baocao/BAOCAO52.md` | MỚI | file này |
| `baocao/evidence/baocao52_*` | MỚI | cargo test, paper 5', skips, sim.arb |

**KHÔNG đụng**: `AGENTS.md`, `.env`, `pairs.txt`, `victims.txt`, cờ live, bot/config VPS.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
scripts/paper_run.sh --minutes 5 --port 8797
```

Không chạy 15 case revm (nợ RPC archive, ghi MISSING).

## 5. OUTPUT THẬT

**Máy: WSL.** `cargo test` + paper 5 phút.

Binary paper `sha256sum target/release/bsc_sandwich` =
`83565571c77c69c1f3e7cec90a069fb05e31161ed9a1fea9ecf19f3205bc8406`
(git HEAD lúc paper = `187f10562f447e0f0b9dac27f73f231445242436`, working tree cụm này).

```
--- cargo test --lib --offline ---
test result: ok. 461 passed; 0 failed; 19 ignored; finished in 0.25s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

Lib 453 (BAOCAO51) → **461** (+8: clamp/search 4 route + accept 40/20 +
pipeline 40 BNB/trần 20 + quote lạ).

Fixture pipeline: `arb_borrow_40_bnb_over_cap_20_is_sanity_reject_not_simulated`
pass — vay 40 BNB / trần 20 → `PipelineSkip::SanityReject`, không Simulated.
USDT 40 < 12000 pass; USDT 13000 reject.

```
--- /api/skips paper 5 phút (port 8797, 2026-09-17T05:46Z–05:52Z) ---
{"arb_no_flash_source":0,"arb_no_second_venue":0,"below_min":1,
 "decode_fail":1622,"not_in_list":2977,"not_quote_pair":114,
 "sanity_reject":1,"unprofitable":1,"venue_unpinned":0,
 "competitor_victim":0,"gas_cap":0,"no_pool":0,"rpc_error":0}
```

```
--- sim.arb (mọi dòng, cửa sổ 5 phút) ---
n=1  simulated=0  unprofitable=1  over_cap=0
ts=05:52:00Z token=USDT(as-token) route=v2_v3 borrow=2 wei
  net=-0.00036 BNB  flash=infinity_vault  decision=unprofitable
```

**0 dòng `sim.arb` `simulated` có borrow > trần.** Kỳ vọng đạt.

`sanity_reject=1`: tx V2 token `0x92aa…` `amount_in=200 BNB` /
`reserve_quote≈4741 BNB` — cửa reserve `arb_sanity_ok_mixed` (không phải
vượt `arb_max_borrow_*`; không log `sim.arb`).

File: `baocao/evidence/baocao52_paper5.out`, `baocao52_paper5_skips.json`,
`baocao52_paper5_simarb.jsonl`, `baocao52_cargo_test.txt`.

## 6. CHAIN

Paper WSL `pending_source` từ funnel: `seen` ~13k–15k/phút. Không đo lại
getCode (đã pin). Không SSH VPS.

15 case revm: **MISSING** (RPC `eth_getStorageAt` `-32000`, nợ BAOCAO51).

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V2/V3, Uni V3, Infinity Vault flash).

## 8. KHÔNG LÀM

Không contract Solidity. Không sendRaw. Không deploy VPS. Không đụng
`.env` / `pairs.txt` / `victims.txt` / cờ live. Không nới both_ok. Không
paper 6h. Không sửa AGENTS.md. Không 15 case revm.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- 15 case revm — cần node `getStorageAt` (BSC_HTTP_SIM archive). MISSING.
- Quote V3 đồng khối với tx (fit ảo quá sâu) — ngoài phạm vi B6.
- Go/No-Go B1 sau khi kẹp trần: cần cửa sổ live ≥6 h. Cụm này không đo lại
  cơ hội/ngày (5 phút: 0 simulated).
- VPS chưa deploy binary này; `config.toml` VPS không đổi field mới.

---

Commit: `837a08f5f0359edf7c5ac2ee7311d750844dbdd3`
