# BAOCAO49 — quét 4 h + bổ sung pairs.txt nếu có V3

## 1. LÁT

Quét lại đa venue **4 giờ** + bổ sung 129 token `pairs.txt` nếu có V3
(PCS V2, PCS V3, Uniswap V3). Máy WSL. Không sim, không contract, không live.
Không sửa `pairs.txt`.

HEAD lúc mở: `233682a8682f6ae51fbe14bde93ce4e2ba67bdce` (sau BAOCAO48).

## 2. LỆNH NHẬN

Chủ: “quét lại 4 tiếng, và cho phép bổ sung thêm 129 token trong pair.txt
nếu có v3 thì lấy, pancakeswap v2 v3, uniswap v3”.

## 3. FILE ĐỔI

Commit nội dung: xem dòng `Commit:` cuối file.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/discover_mv.rs` | sửa | Uniswap V3 tính V3 (impact); pairs.txt keep nếu có pool V3 |
| `src/bin/discover_multivenue.rs` | sửa | `--pairs`, probe 4 h + bổ sung |
| `src/multivenue.rs` | sửa | `from_pairs`, `uni_v3.ok` |
| `AGENTS.md` | sửa | điểm 7: Uniswap venue hợp lệ + bổ sung pairs |
| `docs/STATE.md` `docs/TASKS.md` | sửa | số 4 h |
| `baocao/BAOCAO49.md` | MỚI | file này |
| `baocao/evidence/baocao49_*` | MỚI | log 4 h + candidates |

`pairs.txt` **không** đổi. `state/multi_venue.json` gitignored.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
./target/release/discover_multivenue --hours 4 --top 500 \
  --min-v2-bnb 50 --min-v3-impact-pct 2 --probe-bnb 1 \
  --pairs pairs.txt --out state/multi_venue.json
```

## 5. OUTPUT THẬT

**Máy: WSL.** Binary
`sha256sum target/release/discover_multivenue` =
`e23d3474da3045f6626ff0af088ecabf240292cca80a7f4ee88c10efed27a893`

```
--- cargo test --lib ---
test result: ok. 445 passed; 0 failed; 19 ignored; finished in 0.21s
--- cargo test --lib discover_mv:: ---
test result: ok. 20 passed; 0 failed; 1 ignored
--- cargo test --bin bsc_sandwich ---
test result: ok. 18 passed
```

```
--- discover --hours 4 --pairs pairs.txt ---
START 2026-09-16T15:37:16Z
END   2026-09-16T16:18:13Z
elapsed_sec=2457  (tool in elapsed=2252.7s ≈ 37.5 phút)
chain=56 block=122246764
blocks=32001 (~0.450 s/block, cửa sổ 4 h)
volume_method=swap_logs_v2_v3
logs=638362  pcs_pools_seen=1297
scanned_tokens_vol=1271  probed=611 (500 vol + 111 pairs ngoài top)
pairs.txt unique=129  pairs_with_v3=108
v2_ok=209  v3_ok=49  both_ok=34  keep_in_list=109
tier PCS V3 ok: {100:17, 500:16, 2500:34, 10000:18}
tier Uni V3 ok: {100:5, 500:9, 3000:14, 10000:12}
```

Luật keep: `both_ok` (V2 đủ ngưỡng + V3 PCS/Uni cùng quote impact ≤2 %)
**hoặc** (từ pairs.txt **và** có pool V3 PCS hoặc Uniswap — “nếu có v3 thì lấy”).

Top 10 keep theo vol 4 h:

| # | symbol | vol_4h | both | pairs | uni pools |
|---|---|---|---|---|---|
| 1 | BORT | 166.04 | no | yes | 2 |
| 2 | B | 77.34 | no | yes | 3 |
| 3 | USDC | 65.77 | yes | yes | 8 |
| 4 | DOT | 34.88 | yes | yes | 5 |
| 5 | 人生K线 | 20.98 | no | yes | 2 |
| 6 | 黑马 | 13.42 | no | yes | 2 |
| 7 | bibi | 4.70 | no | yes | 4 |
| 8 | ADA | 3.47 | yes | yes | 5 |
| 9 | COSA | 1.64 | yes | no (vol) | 0 |
| 10 | DOGE | 1.09 | yes | yes | 7 |

10 dòng đầu candidates:

```
# discover_multivenue — CHUA VET. Chu chay vet_bsc_token roi dien vetted YYYY-MM-DD.
0x2a846aaaf896ef393ccb76398c1d96ea97374444 # BORT | vetted  | tax ?/? | owner unknown | discover_multivenue src=pairs vol=166.0357 v2=WBNB v3=USDT@10000 uni=USDT@500+USDT@10000
0x6bdcce4a559076e37755a78ce0c06214e59e4444 # B | vetted  | tax ?/? | owner unknown | discover_multivenue src=pairs vol=77.3356 v2=WBNB v3= uni=WBNB@10000+USDT@3000+USDT@10000
0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d # USDC | vetted  | tax ?/? | owner unknown | discover_multivenue src=pairs vol=65.7732 v2=WBNB+USDT v3=WBNB@100+WBNB@500+USDT@100+USDT@500 uni=...
0x7083609fce4d1d8dc0c979aab8c869ea2c873402 # DOT | vetted  | tax ?/? | owner unknown | ...
0x1a1e69f1e6182e2f8b9e8987e83c016ac9444444 # 人生K线 | vetted  | ...
0xf9c6e80e9a5807a1214a79449009b48104f94444 # 黑马 | vetted  | ...
0x9212cf1f9f4a9c69bb010146ba5b0725169d4444 # bibi | vetted  | ...
0x3ee2200efb3400fabb9aacf31297cbdd1d435d47 # ADA | vetted  | ...
0x5f980533b994c93631a639deda7892fc49995839 # COSA | vetted  | src=vol ...
0xba2ae424d960c26247dd6c32edc70b295c744c43 # DOGE | vetted  | ...
```

109 dòng keep (1 comment + 109 token = 110 dòng file).
Log: `baocao/evidence/baocao49_discover_4h.txt`.

## 6. CHAIN

`eth_chainId=0x38`, block `122246764`, Swap log PCS V2+V3 32001 block.

## 7. REGISTRY

Không pin mới. Uniswap V3 BSC đã pin BAOCAO48; phiên này **dùng** làm venue
lọc (impact) + bổ sung pairs.

## 8. KHÔNG LÀM

Không sim_arb V3, không đo cơ hội, không contract, không sendRaw, không sửa
`pairs.txt` / `.env` / VPS. Không THENA/Biswap.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- `--hours 24` vẫn chưa chạy (Chủ đổi sang 4 h).
- Field JSON vẫn tên `vol24h_bnb` dù cửa sổ là 4 h.
- List 109 token **chưa vet tay** (`vetted` trống) — Chủ chạy `vet_bsc_token`.
- 108/129 pairs có V3; 21 pairs không V3 → không lấy.
- sim_arb V3 / đo cơ hội — cụm sau khi vet.
- VPS `config.toml` 4 field `multivenue_*` trước binary mới.

---

Commit: (điền sau git commit)
