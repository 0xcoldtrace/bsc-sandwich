# BAOCAO54 — cụm `planB-B8-simarb-revm-18`

## 1. LÁT

`planB-B8-simarb-revm-18` — đối chiếu 18 dòng `sim.arb simulated` (BAOCAO53)
với revm, cùng borrow / `route_kind=v3_v3` / flash `infinity_vault`, fork
đúng block victim. Máy WSL. Không contract, không live, không đụng unit VPS.

HEAD lúc mở: `e3ae72777e4b0ed81b0a0a2575350bf54f6df16e` (BAOCAO53 điền hash).
`origin/master` có BAOCAO53 (`e3ae727`) và nội dung B7 (`e4a8b5b`).

## 2. LỆNH NHẬN

Khối `planB-B8-simarb-revm-18`. Replay đúng 18 hash evidence BAOCAO53
(`baocao53_paper60_simarb.jsonl` / `baocao53_paper60_simulated.tsv`).
Thiếu field block trong TSV → lấy `eth_getTransactionReceipt`. Cùng borrow,
cùng route, cùng flash. Lệch > 20% hoặc revm revert → FAIL số. Thiếu
archive / `eth_getStorageAt` `-32000` → ô 6 MISSING, không đoán lãi.
Không sửa `pairs_arb` / `strategy` / live flags. Không paper 6h. Không B1.
Không đụng unit VPS (Chủ không dán host).

## 3. FILE ĐỔI

Commit nội dung: `2d00d0db79d2377875d36e7dcd02d8868147d562`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/bin/arb_replay_18.rs` | MỚI | replay 18 simulated vs revm, failover SIM→HTTP |
| `src/sim_arb.rs` | sửa | `profit_lech_pct` + test 18 hash jsonl↔tsv |
| `Cargo.toml` | sửa | bin `arb_replay_18` |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8 |
| `docs/TASKS.md` | sửa | hàng B8 |
| `docs/DOC_MAP.md` | sửa | bin B8 |
| `baocao/BAOCAO54.md` | MỚI | file này |
| `baocao/evidence/baocao54_*` | MỚI | cargo test, replay out, TSV 18 hàng |

**KHÔNG đụng**: `AGENTS.md` (còn dirty local, không stage), `.env`,
`pairs.txt`, `victims.txt`, `pairs_arb.txt`, cờ live, unit VPS.

## 4. LỆNH CHẠY

```
git fetch && git log -1 --oneline origin/master
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo build --release --bin arb_replay_18 --offline
./target/release/arb_replay_18 \
  --jsonl baocao/evidence/baocao53_paper60_simarb.jsonl \
  --multi-venue state/multi_venue.json
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`git HEAD` lúc chạy = `e3ae72777e4b0ed81b0a0a2575350bf54f6df16e`.
`sha256sum target/release/arb_replay_18` =
`453ba5d742d8b2cb03219cbe8ee2675c17a21031aafeb7c1755cda326b01c3a6`.
Binary `bsc_sandwich` không rebuild cụm này:
`sha256=88d3f7824fded9ba26f6b2d7af46c7ddb95d4149316495ba6d299b2f31ad1496`.

```
--- cargo test --lib --offline ---
test result: ok. 463 passed; 0 failed; 19 ignored; finished in 0.25s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

(`463` = 461 BAOCAO53 + 2 test B8: `profit_lech_pct_nguong_20`,
`baocao53_18_simulated_hashes_khop_tsv`.)

```
arb_replay_18 n_sim=3 n_http=5 n_url=6 hosts=bsc.blockrazor.xyz,rpc-bsc.48.club,bsc-dataseed1.bnbchain.org,bsc-dataseed1.defibit.io,bsc-rpc.publicnode.com,bsc.rpc.blxrbdn.com
connect_ok host=bsc.blockrazor.xyz head=122428418
connect_ok host=rpc-bsc.48.club head=122428419
connect_ok host=bsc-dataseed1.bnbchain.org head=122428420
connect_ok host=bsc-dataseed1.defibit.io head=122428421
connect_ok host=bsc-rpc.publicnode.com head=122428422
connect_ok host=bsc.rpc.blxrbdn.com head=122428423
n_simulated=18
archive_fail host=bsc.blockrazor.xyz block=122417145 ... code: -32000, message: "not supported"
archive_fail host=rpc-bsc.48.club block=122417145 ... code: -32000, message: "not supported"
archive_fail host=bsc-dataseed1.bnbchain.org block=122417145 ... code: -32000, message: "missing trie node"
archive_fail host=bsc-dataseed1.defibit.io block=122417145 ... code: -32000, message: "missing trie node"
archive_fail host=bsc-rpc.publicnode.com block=122417145 ... code: -32602, message: "Archive requests require a personal token"
archive_fail host=bsc.rpc.blxrbdn.com block=122417145 ... code: -32000, message: "not supported"
SUMMARY n=18 n_ok=0 n_fail_lech=0 n_revert=0 n_missing=18
FAIL_rule: lech>20% n=0 revert n=0 (archive/MISSING n=18 khong doan lai)
```

Bảng 18 hàng (borrow / profit_paper = USDT; profit_revm không đoán):

| token | route | borrow | profit_paper | profit_revm | lệch_pct | revert |
|---|---|---|---|---|---|---|
| `4` | v3_v3 | 32.761030 | 1.813484 | MISSING | MISSING | MISSING |
| LINK | v3_v3 | 46.372956 | 4.902730 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 32.934211 | 1.823740 | MISSING | MISSING | MISSING |
| LINK | v3_v3 | 46.372956 | 4.902730 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 32.325603 | 1.752745 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 35.177890 | 2.058261 | MISSING | MISSING | MISSING |
| LINK | v3_v3 | 46.372956 | 4.902730 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 35.345129 | 2.079217 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 35.345129 | 2.079217 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 35.983304 | 2.160277 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 35.983304 | 2.160277 | MISSING | MISSING | MISSING |
| Cake | v3_v3 | 195.058900 | 12.042311 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.103055 | 2.175682 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.348576 | 2.207458 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.345801 | 2.207098 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.875259 | 2.276497 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.875259 | 2.276497 | MISSING | MISSING | MISSING |
| `4` | v3_v3 | 36.919065 | 2.269609 | MISSING | MISSING | MISSING |

Block victim (từ receipt, TSV không có): 122417145 … 122421171.
Hash + TSV đầy đủ: `baocao/evidence/baocao54_revm18.tsv`.
Log thô ≥15 dòng: `baocao/evidence/baocao54_revm18.out`.

`n_fail_lech=0` `n_revert=0` — **không có số để FAIL theo lệch > 20%**.
18/18 `profit_revm=MISSING`. Không kết luận fit V3 ảo.

## 6. CHAIN

Receipt 18/18 hash trên chain 56: block 122417145–122421171
(head ~122428423 lúc chạy, cách ~7k–11k block). Không đo lại getCode.

`eth_getStorageAt` tại block victim: **MISSING** trên cả 6 host
`BSC_HTTP_SIM`+`BSC_HTTP` (`-32000 not supported` / `missing trie node` /
`-32602 archive personal token`). Latest/`block-1` vẫn đọc được (probe
trước replay) — thiếu là **archive lịch sử**, không phải method cấm hoàn toàn.

SSH VPS: **MISSING** (khối lệnh không dán host). Không start/stop/deploy.

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V3, Uni V3, Infinity Vault flash).

## 8. KHÔNG LÀM

Không Solidity / ArbExecutor. Không sendRaw. Không OOM-fix. Không
`discover_multivenue`. Không sửa AGENTS chiến lược. Không sửa
`pairs_arb.txt` / `pairs.txt` / `victims.txt` / cờ live. Không paper 6h.
Không đụng unit VPS. Không đoán `profit_revm`.

## 9. CHỮ

**CHƯA XONG**

## 10. CÒN NỢ / LÁT SAU

- 18/18 `profit_revm` MISSING — cần node archive (`BSC_HTTP_SIM` giữ
  `eth_getStorageAt` tại block ~122417145). Có archive thì chạy lại
  `arb_replay_18` (không viết lại tool). Lệch > 20% hoặc revert lúc đó
  mới là FAIL số.
- Fit V3 ảo (nợ B5/B6/B7) **chưa đối chiếu được**.
- Go/No-Go B1: không kết luận (thiếu revm).
- VPS: không host trong lệnh; không deploy B8.
- `AGENTS.md` dirty local — không stage.

---

Commit: `2d00d0db79d2377875d36e7dcd02d8868147d562`
