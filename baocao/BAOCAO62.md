# BAOCAO62 — cụm `count-funnel-skip`

## 1. LÁT

`count-funnel-skip` — chỉ đếm. Không sửa `src/`, không paper, không live.
HEAD lúc mở: `172a3e9105216b90b73223a818c837c9618a5235` (BAOCAO61 docs).
Nhánh `main`. Máy WSL.

## 2. LỆNH NHẬN

In số dòng vetted `pairs_arb.txt`; `strategy` + `pairs_arb_path` trong
`config.toml`; nếu có `logs/bot.jsonl` thì đếm `tx.skip` theo `reason`
(top 10); không có log thì MISSING. Viết baocao ngắn. Không kết luận hết
cơ hội. Commit nếu có file baocao mới.

## 3. FILE ĐỔI

| File | Trạng thái | Nội dung |
|---|---|---|
| `baocao/BAOCAO62.md` | MỚI | file này |

**KHÔNG đụng**: `src/`, `config.toml`, `.env`, `pairs.txt`, `pairs_arb.txt`,
cờ live, `CLAUDE.md`, `AGENTS.md`, baocao cũ, `logs/` (chỉ đọc).

## 4. LỆNH CHẠY

Đọc file local. Không `cargo`, không RPC.

```
python3  # đếm dòng 0x vetted trong pairs_arb.txt
python3  # in strategy / pairs_arb_path từ config.toml
python3  # stream logs/bot.jsonl: event=tx.skip group reason
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`). Không rebuild.

`pairs_arb.txt`: **28** dòng `0x` có `vetted 2026-09-17` (28/28 non-comment).
List hẹp, không phải toàn thị trường.

`config.toml` (chỉ đọc):

```
strategy = "backrun"
pairs_arb_path = "pairs_arb.txt"
```

`logs/bot.jsonl`: **có** (size 202854209 bytes, json_fail=0, empty=0,
n_lines=481235). Không MISSING.

File này **không** phải 1 cửa sổ paper: `bot.start` = **10** lần,
`first_ts` = `2026-09-16T08:51:36.909268720+00:00`,
`last_ts` = `2026-09-18T05:43:13.915166928+00:00`.

`tx.seen` = 208454. `tx.skip` = 208362. Mọi `tx.skip` có field `reason`.

**Top 10 `tx.skip` theo `reason` (toàn file):**

| # | reason | count |
|---|---|---|
| 1 | decode_fail | 87961 |
| 2 | not_in_list | 77815 |
| 3 | sell_direction | 10786 |
| 4 | unprofitable | 9650 |
| 5 | below_min | 9314 |
| 6 | not_quote_pair | 7741 |
| 7 | venue_unpinned | 3862 |
| 8 | arb_no_second_venue | 941 |
| 9 | victim_would_revert | 179 |
| 10 | rpc_error | 99 |

Còn lại (ngoài top 10): `sanity_reject=8`, `deadline=3`, `thin_liq=3`.
Tổng reason = 208362.

`sim.arb` trên cùng file: n=8239 (`unprofitable=8203`, `simulated=36`).
**Không** gọi Simulated là lãi. **Không** kết luận hết cơ hội.

universe_pairs: 28 (dòng 0x vetted `pairs_arb.txt`, list hẹp)
swaps_ingested: 208454 (`tx.seen`, toàn `logs/bot.jsonl`, 10 lần `bot.start`)
window_minutes: 2691.62 (span lịch file, KHÔNG phải 1 cửa sổ paper)
status_enum: CHỜ FABLE
skip_top: decode_fail=87961, not_in_list=77815, sell_direction=10786, unprofitable=9650, below_min=9314, not_quote_pair=7741, venue_unpinned=3862, arb_no_second_venue=941, victim_would_revert=179, rpc_error=99
git: 172a3e9105216b90b73223a818c837c9618a5235 (HEAD lúc mở; hash commit = `git log -1` sau commit)
máy: WSL
binary_or_head: không rebuild; HEAD lúc mở `172a3e9105216b90b73223a818c837c9618a5235`

## 6. CHAIN

Không gọi RPC. SSH VPS: **MISSING** (lệnh không có host).

## 7. REGISTRY

Không pin mới.

## 8. KHÔNG LÀM

Sửa `src/`, paper, live, `sendRaw`, `ArbExecutor`, nới list, kết luận hết
cơ hội / đổi dự án, tự ĐẠT.

## 9. CHỮ

CHỜ FABLE

## 10. CÒN NỢ / LÁT SAU

**Cấm Go B1.** Số trên là đếm file log ghép 10 lần boot, không phải mẫu
một cửa sổ. `not_in_list` + `decode_fail` đứng đầu — đó là đếm skip, không
phải “hết cơ hội”. Việc Fable: chọn nhánh `if_zero_hits` nếu cần
(EXPAND_UNIVERSE / LOG_NEAR_MISS / RELAX_FILTER / FIX_INFRA). Commit này
chỉ đếm.
