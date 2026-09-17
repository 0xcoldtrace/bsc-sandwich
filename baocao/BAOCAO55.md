# BAOCAO55 — cụm `planB-B8b-replay-archive`

## 1. LÁT

`planB-B8b-replay-archive` — replay 18 case B8 (`sim.arb simulated`,
BAOCAO53/54) trên node archive. Máy WSL. Không contract, không live,
không nới list, không B1.

HEAD lúc mở: `e572f0bfede037e54125d8b43fad6098b31fe51c` (BAOCAO54 điền hash).
`origin/master` lúc mở còn BAOCAO53 (`e3ae727`); local ahead 2 commit B8
(`2d00d0d` / `e572f0b`).

## 2. LỆNH NHẬN

Khối `planB-B8b-replay-archive`. Probe `RPC_LIST` (Chủ dán, không in URL
đầy đủ). URL đầu PASS `eth_getStorageAt` block `0x74beff9` →
`BSC_HTTP_SIM` process-only. Chạy 18 hàng BAOCAO54, cùng borrow / route /
flash. Test `--offline`. Docs + evidence. Commit 2 lần. Push PAT một lần
(không lưu remote). Ô 10 cấm Go B1.

## 3. FILE ĐỔI

Commit nội dung: `COMMIT_NOI_DUNG`.

| File | Trạng thái | Nội dung |
|---|---|---|
| `src/bin/arb_replay_18.rs` | sửa | retry `-32005`, receipt failover, panic/hàng → MISSING |
| `docs/STATE.md` | sửa | mục hiện tại + cụm B8b |
| `docs/TASKS.md` | sửa | hàng B8b |
| `baocao/BAOCAO55.md` | MỚI | file này |
| `baocao/evidence/baocao55_*` | MỚI | cargo test, replay out, TSV 18 hàng, run1 rate-limit |

**KHÔNG đụng**: `AGENTS.md` (còn dirty local, không stage), `.env`,
`pairs.txt`, `victims.txt`, `pairs_arb.txt`, cờ live, unit VPS.

## 4. LỆNH CHẠY

```
# probe RPC_LIST: eth_blockNumber + eth_getStorageAt(Cake, 0x0, 0x74beff9)
# BSC_HTTP_SIM = URL đầu PASS (process); BSC_HTTP_SIM_2 = URL 2 PASS
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo build --release --bin arb_replay_18 --offline   # sau sửa retry
./target/release/arb_replay_18 \
  --jsonl baocao/evidence/baocao53_paper60_simarb.jsonl \
  --multi-venue state/multi_venue.json
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
`git HEAD` lúc chạy replay = `e572f0bfede037e54125d8b43fad6098b31fe51c`.
`sha256sum target/release/arb_replay_18` (sau patch retry) =
`1b5d06a983e8ae7487b74bccf2eda46494bd4bc91ebf0ef3c49c9752e858d27d`.
Binary `bsc_sandwich` không rebuild cụm này:
`sha256=88d3f7824fded9ba26f6b2d7af46c7ddb95d4149316495ba6d299b2f31ad1496`.

```
--- cargo test --lib --offline ---
test result: ok. 463 passed; 0 failed; 19 ignored; finished in 0.31s
--- cargo test --bin bsc_sandwich --offline ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
--- sau patch retry (cùng 2 lệnh) ---
test result: ok. 463 passed; 0 failed; 19 ignored; finished in 0.29s
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
```

Probe `RPC_LIST` (host + 4 ký tự cuối path, không in token):

| # | host | eth_blockNumber | getStorageAt 0x74beff9 | verdict |
|---|---|---|---|---|
| 1 | `bsc-mainnet.nodereal.io/***12d3` | OK `0x74c2b77` | OK `result` `0x` len=66 | **PASS** |
| 2 | `bsc-mainnet.nodereal.io/***2bc5` | OK `0x74c2b79` | OK `result` `0x` len=66 | PASS |
| 3 | `shared.us-east-1.getblock.io/***54dc` | OK `0x74c2b7c` | `-32000` historical state is not available | FAIL |

URL đầu PASS → `BSC_HTTP_SIM` process. URL 2 PASS → `BSC_HTTP_SIM_2`.
GetBlock không dùng. Không sửa 6 host public B8.

Run 1 (binary cũ, chỉ SIM, không retry): hàng 1 có số revm rồi `-32005`
quota public → 17 MISSING. Log:
`baocao/evidence/baocao55_revm18_run1_ratelimit.out`.

Run 2 (bin patch, SIM + HTTP public cho receipt, retry `-32005`):
`start_ts=2026-09-17T21:58:21+07:00` `end_ts=2026-09-17T22:16:13+07:00`
(~1072 s). `REPLAY_EXIT:0`.

```
arb_replay_18 n_sim=2 n_http=5 n_url=7 hosts=bsc-mainnet.nodereal.io,bsc-mainnet.nodereal.io,rpc-bsc.48.club,bsc-dataseed1.bnbchain.org,bsc-dataseed1.defibit.io,bsc-rpc.publicnode.com,bsc.rpc.blxrbdn.com
connect_ok host=bsc-mainnet.nodereal.io#0 head=122433455
n_simulated=18
SUMMARY n=18 n_ok=0 n_fail_lech=18 n_revert=0 n_missing=0
FAIL_rule: lech>20% n=18 revert n=0 (archive/MISSING n=0 khong doan lai)
```

Bảng 18 hàng (borrow / profit = USDT). Symbol file `币安人生` = địa chỉ LINK
BAOCAO54 `0x924fa68a…4444`. `lệch_pct = |revm-paper|/|paper|*100`.

| token | route | borrow_USDT | profit_paper | profit_revm | lệch_pct | revert | tx_hash |
|---|---|---|---|---|---|---|---|
| `4` | v3_v3 | 32.761030 | 1.813484 | -2.839823 | 256.5948 | no | `0x487c33fe…0709f7` |
| LINK | v3_v3 | 46.372956 | 4.902730 | -1.566826 | 131.9582 | no | `0x0f9be535…48408a` |
| `4` | v3_v3 | 32.934211 | 1.823740 | -2.908739 | 259.4931 | no | `0x63bb39da…37c0ca` |
| LINK | v3_v3 | 46.372956 | 4.902730 | -1.566826 | 131.9582 | no | `0x41370019…2990a2` |
| `4` | v3_v3 | 32.325603 | 1.752745 | -2.866739 | 263.5571 | no | `0x85a80380…b314f1` |
| `4` | v3_v3 | 35.177890 | 2.058261 | -3.217150 | 256.3043 | no | `0xc0045f36…ce094a` |
| LINK | v3_v3 | 46.372956 | 4.902730 | -1.566826 | 131.9582 | no | `0xdf3c6638…70e778` |
| `4` | v3_v3 | 35.345129 | 2.079217 | -3.237160 | 255.6913 | no | `0x729e51f5…5c3209d` |
| `4` | v3_v3 | 35.345129 | 2.079217 | -3.226399 | 255.1738 | no | `0xe9724c30…781b79` |
| `4` | v3_v3 | 35.983304 | 2.160277 | -3.274166 | 251.5623 | no | `0x31b657b6…02443a` |
| `4` | v3_v3 | 35.983304 | 2.160277 | -3.276390 | 251.6652 | no | `0xf2f32b1e…38fd05` |
| Cake | v3_v3 | 195.058900 | 12.042311 | -152.095629 | 1363.0103 | no | `0xefabc7bf…50ff779` |
| `4` | v3_v3 | 36.103055 | 2.175682 | -3.303939 | 251.8576 | no | `0x1b335a6a…769ae6` |
| `4` | v3_v3 | 36.348576 | 2.207458 | -3.341331 | 251.3655 | no | `0x91f718b7…dd2c6e` |
| `4` | v3_v3 | 36.345801 | 2.207098 | -3.344307 | 251.5251 | no | `0x1d80be68…ff7a1e` |
| `4` | v3_v3 | 36.875259 | 2.276497 | -3.383028 | 248.6067 | no | `0x11a99c35…8ea5db` |
| `4` | v3_v3 | 36.875259 | 2.276497 | -3.372920 | 248.1627 | no | `0x3f966e9f…7fc3a4` |
| `4` | v3_v3 | 36.919065 | 2.269609 | -3.478118 | 253.2475 | no | `0x6a457323…9d443f` |

LINK lặp 3 dòng cùng borrow `46.372956`: **3 unique `tx_hash`**
(`0x0f9be535…`, `0x41370019…`, `0xdf3c6638…`). Cake 1 unique.

Đọc số (lệnh): unique LINK+Cake = 4. |lệch| > 20% = **4/4**. revert = 0/4.
MISSING revm = 0. → **FAIL số**. Không “khớp giấy”. Không Go B1.

TSV đầy đủ: `baocao/evidence/baocao55_revm18.tsv`.
Log thô: `baocao/evidence/baocao55_revm18.out`.

## 6. CHAIN

`eth_getStorageAt` Cake `0x0e09FaBB…cE82` slot `0x0` block `0x74beff9`
(= 122417145, block victim hàng 1): PASS trên 2 NodeReal (`result` `0x…`
len 66). GetBlock `-32000` historical state — FAIL.

Receipt 18/18 hash trên chain 56: block 122417145–122421171
(head lúc chạy ~122433455). Không pin mới, không đo lại getCode venue.

SSH VPS: **MISSING** (khối lệnh không dán host). Không start/stop/deploy.

## 7. REGISTRY

Không pin mới. Dùng venue đã pin (PCS V3, Uni V3, Infinity Vault flash).

## 8. KHÔNG LÀM

Không Solidity / ArbExecutor. Không sendRaw. Không `discover_multivenue`.
Không sửa `pairs_arb.txt` / `pairs.txt` / `victims.txt` / cờ live.
Không paper 6h. Không đụng unit VPS. Không mở B9. Không commit `.env` /
PAT / URL có token. Không lưu PAT vào `git remote`.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

- **Cấm Go B1.** Unique LINK+Cake 4/4 |lệch| > 20% (FAIL số). Fit V3 ảo
  (quoter paper) không khớp hop revm trên archive: paper dương, revm âm
  mọi hàng (Cake lệch 1363%).
- Không MISSING revm — số đủ để kết luận lệch, không phải thiếu archive.
- VPS: không host trong lệnh; không deploy B8b.
- `AGENTS.md` dirty local — không stage.
- PAT GitHub dùng một lần lúc push, không ghi git config / file repo.

---

Commit: `COMMIT_NOI_DUNG`
