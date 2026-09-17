# BAOCAO58 — cụm `planB-B8e-revm-14-after-gate`

## 1. LÁT

`planB-B8e-revm-14-after-gate` — replay 14 hàng `sim.arb simulated` B8d
(BAOCAO57) trên revm, node archive. Máy WSL. Không contract, không live,
không nới `pairs_arb`, không đổi CPMM, không replay 18 hàng B8b.

HEAD lúc mở: `93813c6796a7d76a882a88f25d829f0b8426a38f` (BAOCAO57 điền
hash). Nhánh có BAOCAO57 / `761be54`. `origin/master` khớp sau `git fetch`.

## 2. LỆNH NHẬN

Khối `planB-B8e-revm-14-after-gate`. Đọc AGENTS → DOC_MAP → STATE B8b/B8c/B8d
→ TASKS → BAOCAO55/56/57 + TSV/jsonl B8d → `arb_replay_18` / `sim_evm` /
cổng `main.rs`. Xác nhận cổng còn. Probe archive Cake slot0 tại block
victim hàng Cake. Replay 14 hash, cùng borrow / route / `infinity_vault`.
Bảng ô 5 đủ 14. Test `--offline`. BAOCAO58 10 ô. Ô 9 không ĐẠT. Ô 10 cấm
Go B1, cấm “đã có lãi”. Commit + push PAT một lần (không lưu remote).

## 3. FILE ĐỔI

Commit nội dung: `48058f194337b3b0fb96f953c76a876fd6dfda3a`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/bin/arb_replay_18.rs` | sửa | jsonl B8d, V2+V3, lệch vs quoter (có dấu), đọc mọi `BSC_HTTP_SIM`, retry `-32005`, Cake trước |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8e |
| `docs/TASKS.md` | sửa | hàng B8e |
| `baocao/BAOCAO58.md` | MỚI | file này |
| `baocao/evidence/baocao58_*` | MỚI | probe, cargo, replay out/TSV, missing7 jsonl |

**KHÔNG đụng**: `AGENTS.md` (dirty local, không stage), `.env`, `pairs.txt`,
`victims.txt`, `pairs_arb.txt`, cờ live, unit VPS, `fit_v3_virtual_reserves`.

## 4. LỆNH CHẠY

```
# probe: eth_getTransactionReceipt Cake hash + eth_getStorageAt(Cake, slot0, block)
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo build --release --bin arb_replay_18 --offline
./target/release/arb_replay_18 \
  --jsonl baocao/evidence/baocao57_paper60_simarb.jsonl \
  --multi-venue state/multi_venue.json
# run2 (7 hàng -32005): --jsonl baocao/evidence/baocao58_missing7.jsonl
```

Cổng xác nhận còn: `src/multivenue.rs` `if !p.ok { continue; }` (V3 + Uni);
`src/main.rs` `quote_mixed_hops_at` + `size_quote_allows_simulated` trước
`decision=simulated`. Không sửa cổng.

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`git HEAD` lúc chạy = `93813c6796a7d76a882a88f25d829f0b8426a38f`.
`sha256sum target/release/arb_replay_18` (bin cuối, backoff run2) =
`178084f5ed15e313818685a6e1fa6aaab2d1c7424ada1b53c9449a8580dbc494`.
Run1 (14 hàng, 7 MISSING quota): sha256
`41607feae525030b65ec68f9d0a5fbbb2df5fbe37d0d442191c05f9f204c9a1f`.
Binary `bsc_sandwich` không rebuild:
`sha256=4e793c643d1c16a0855561e20f830157925b91c60a69512a0065fc07551af7a0`.

```
--- cargo test --lib --offline ---
test result: ok. 468 passed; 0 failed; 19 ignored; finished in 0.26s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
--- sau retry bin (cùng 2 lệnh) ---
test result: ok. 468 passed; 0 failed; 19 ignored; finished in 0.29s
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.22s
```

Probe archive (không in URL):

```
RECEIPT_OK host=rpc-bsc.48.club block=0x74c70c0 block_dec=122450112 tx_index=0x52 status=0x1
STORAGE_FAIL host=bsc.blockrazor.xyz HTTP 403
STORAGE_FAIL host=rpc-bsc.48.club code=-32000 not supported
STORAGE_FAIL host=bsc-dataseed1.bnbchain.org code=-32000 missing trie node
STORAGE_OK host=bsc-mainnet.nodereal.io result_len=66 result_head=0x00000000
PROBE_VERDICT PASS
```

dotenv first-wins bỏ dòng `BSC_HTTP_SIM` sau (NodeReal). Bin đọc mọi
occurrence. GetBlock không có trong `.env` lần này; không dùng.

Run1 `start_ts=2026-09-18T00:45:21+07:00` `end_ts=2026-09-18T00:52:31+07:00`
(~430 s). Cake + 6 v2_v3 có số; 7 hàng `-32005` → MISSING.
Run2 (7 hash, backoff dài hơn) `start_ts=2026-09-18T00:55:17+07:00`
`end_ts=2026-09-18T01:02:56+07:00`. `n_missing=0`. `REPLAY_EXIT:0`.

```
n_simulated=14 n_unique_hash=14
fork=BlockId::number(victim_receipt_block) post-state apply_victim_raw=no
SUMMARY n=14 n_unique_hash=14 n_ok=0 n_fail_lech=14 n_revert=0 n_missing=0
FAIL_rule: |lech_vs_quoter|>20% n=14 revert n=0 (archive/MISSING n=0 khong doan lai)
```

Bảng 14 hàng (đơn vị BNB). `lệch_pct = (profit_revm - net_quoter) / |net_quoter| * 100`
(so QUOTER, không so paper CPMM). Flash `infinity_vault`. 14 unique hash.

| token | route | borrow_BNB | net_paper | net_quoter | profit_revm | lệch_pct_vs_quoter | revert | tx_hash_full |
|---|---|---|---|---|---|---|---|---|
| 币安人生 `0x924fa68a0fc644485b8df8abfa0a41c2e7744444` | v2_v3 | 1.989860 | 0.005752 | 0.005752 | -0.041972 | -829.6702 | no | `0xbc42244db9572b202239761ec2ac20804b0854eef01139f89bc1aae0aab97013` |
| `4` `0x0a43fc31a73013089df59194872ecae4cae14444` | v2_v3 | 2.585473 | 0.013411 | 0.013411 | -0.049150 | -466.4797 | no | `0x657218fc2621df92676029346db880a163276d26d4baf15e88caeaf7d5990f53` |
| `4` | v2_v3 | 2.585506 | 0.013412 | 0.013412 | -0.049151 | -466.4777 | no | `0x403cc445f991e782235cdfb664430fcc7d07acd4b5b998a8e934a1dde4b98569` |
| `4` | v2_v3 | 2.454692 | 0.012053 | 0.012053 | -0.047952 | -497.8337 | no | `0xce7aafe720bb455a6b2e5a385ab7079023564d97e107ead18361b7fa34fc8e5a` |
| `4` | v2_v3 | 2.502680 | 0.012558 | 0.012558 | -0.048478 | -486.0219 | no | `0xc71f56666fc398f6238b6228ff41c727c99467bc8664791f9a032bf5e3ec859b` |
| 币安人生 | v2_v3 | 1.975102 | 0.005664 | 0.005664 | -0.042356 | -847.8726 | no | `0x02896c83fc327e5185f797ec25a8fb7bb6b23fa0a7b2e644f64eba9070c061d8` |
| `4` | v2_v3 | 2.400094 | 0.011538 | 0.011538 | -0.047363 | -510.4877 | no | `0x85d57a7caf41df976d9f9b9845fbde51b3d27cbd3bfceadf0f84d6b7a325c530` |
| 我踏马来了 `0xc51a9250795c0186a6fb4a7d20a90330651e4444` | v2_v3 | 2.600822 | 0.010795 | 0.010795 | -0.045165 | -518.3751 | no | `0xc0020ff64a7ed162876224265a3147fa5028b82a2085c35a1a03e0cb856b5c30` |
| `4` | v2_v3 | 2.535789 | 0.012845 | 0.012849 | -0.047791 | -471.9311 | no | `0x73a4958a796e7ab9b603ad368fcd9d2f014266a31de3866cd7ea752dd8fd6a15` |
| `4` | v2_v3 | 2.490468 | 0.012371 | 0.012375 | -0.047067 | -480.3272 | no | `0x2799839cb99839c0f5289cdce7213024fc940036ef7ae140bb57c5e4041f8481` |
| `4` | v2_v3 | 2.517964 | 0.012597 | 0.012600 | -0.049490 | -492.7726 | no | `0xcdb08900af6f7ae32b14cacbebc414c7de197c0a01f8bc6851fb18f8a21534c4` |
| `4` | v2_v3 | 2.590105 | 0.013384 | 0.013396 | -0.049418 | -468.8991 | no | `0x493cd4d9043a4686a6dfa3588a389f14b0eeeb5205e3e6421eb0bb3ff75ab5fb` |
| Cake `0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82` | v3_v3 | 20.000000 | 0.319424 | 0.316064 | -0.179687 | -156.8514 | no | `0x7ba67696c2fcd9c49cf921c5dd52097bedf24ce4c62d0c1a0cf854d9c928cc1d` |
| 币安人生 | v2_v3 | 1.959313 | 0.005570 | 0.005570 | -0.042875 | -869.8128 | no | `0x216d32058da248d92a71f5cb58851e1424a6d14ae3e1aa1f1f0505eb3047edc6` |

Fork: block victim (receipt), post-state, **không** apply victim raw.
Cake pair_buy `0xafb2da14…` / pair_sell `0x7f51c8aa…` (PCS 2500 `ok=true`)
— khác CASE_CAKE USDT BAOCAO56.

Đọc số (lệnh):

- n_unique hash = **14**. n_missing = **0**. n_revert = **0**.
- Cake: revm −0.179687 ≤ 0 **và** |lệch| 156.85% > 20% → **FAIL số**.
- Unique v2_v3: token-4 (9 hash) / 币安人生 (3 hash) / 我踏马来了 (1 hash) =
  **3/3** revm < 0 và |lệch| > 20% (≥ 1/3) → **FAIL số**.
- Không “khớp giấy+quoter”. Không Go B1.

TSV: `baocao/evidence/baocao58_revm14.tsv`.
Log: `baocao58_revm14.out` (run1+run2), `baocao58_revm14_run1.out`,
`baocao58_revm14_run2.out`, `baocao58_probe.txt`, `baocao58_cargo_test.txt`.

## 6. CHAIN

`eth_getStorageAt` Cake `0x0e09FaBB…cE82` slot `0x0` block `0x74c70c0`
(= 122450112, block victim hàng Cake): PASS NodeReal (`result` `0x…`
len 66). Public SIM (Blockrazor 403 / 48.club `-32000` / dataseed missing
trie) FAIL. GetBlock không dùng.

Receipt 14/14 hash trên chain 56: block 122443508–122450436 (head lúc chạy
~122455715 / 122457037). Không pin mới, không đo lại getCode venue.

SSH VPS: **MISSING** (khối lệnh không dán host). Không start/stop/deploy.

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V2/V3, Uni V3, Infinity Vault flash,
QuoterV2).

## 8. KHÔNG LÀM

Không Solidity / ArbExecutor. Không sendRaw. Không `discover_multivenue`.
Không sửa `pairs_arb.txt` / `pairs.txt` / `victims.txt` / cờ live /
AGENTS.md. Không paper 6h. Không đụng unit VPS. Không mở B9. Không sửa
`fit_v3_virtual_reserves`. Không đổi dấu `profit_paper`. Không replay 18
hàng B8b. Không commit `.env` / PAT / URL có token.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- **Cấm Go B1.** Cấm “đã có lãi”. 14/14 quoter+ / revm−, |lệch vs quoter|
  > 20%. Cửa Go (≥30 cơ hội/ngày và p50 ≥ 5 USDT sau bribe, ≥6 h) không
  đo ở cụm này. Cửa sổ paper 60', p50 net_paper ~0.0126 BNB, chưa bundle.
- **FAIL số Cake:** revm −0.179687 ≤ 0, |lệch| 156.85% > 20%.
- **FAIL số unique v2_v3:** 3/3 token (4 / 币安人生 / 我踏马来了) revm < 0
  và |lệch| > 20%.
- Không MISSING revm — số đủ để kết luận lệch, không phải thiếu archive.
- 18/18 B8b paper+ / revm− vẫn đứng. Cổng B8c còn; quoter net > 0 không
  khớp hop revm trên 14 hàng mới.
- VPS: không host trong lệnh; không deploy B8e.
- `AGENTS.md` dirty local — không stage.
- PAT GitHub dùng một lần lúc push, không ghi git config / file repo.

---

Commit: `48058f194337b3b0fb96f953c76a876fd6dfda3a`
