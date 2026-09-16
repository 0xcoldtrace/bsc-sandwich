# BAOCAO48 — cụm `planB-B4-multivenue-tool`

## 1. LÁT

`planB-B4-multivenue-tool` — TOOL + LIST + pin Uniswap V3 BSC. Máy WSL.
Không sim, không đo cơ hội, không contract, không live, không đụng `pairs.txt`.

HEAD THẬT lúc mở phiên: `bf87b5de288fb3b445f10750ec19a907d53af3b3`.

Chủ giữa phiên: chạy `--hours 2` trước; thông rồi mới 24 h; sau đó bảo
báo cáo (không đợi 24 h). `--hours 2` đã xong. `--hours 24` chưa xong.

## 2. LỆNH NHẬN

Khối lệnh `planB-B4-multivenue-tool` mục 1→6 + lệnh giữa phiên
`--hours 2` rồi 24 h + “quét 20 phút xong rồi báo cáo”.
Cấm `.env`, `pairs.txt`, sendRaw, contract, bot VPS, paper/đo cơ hội.
Không subagent ghi file.

## 3. FILE ĐỔI

Commit nội dung: `0a1884c6d2efcd81cd6250b381aa73dcc69300c1`.
Commit cuối phiên (điền hash vào ô này + dòng cuối): xem dòng `Commit:` cuối file.

| File | Trạng thái | Nội dung |
|---|---|---|
| `DEX_REGISTRY.md` | sửa | pin Uniswap V3 BSC Factory/QuoterV2/SwapRouter02 + getCode |
| `AGENTS.md` | sửa | điểm 7 Kế hoạch B + dòng Uniswap + 4 field config |
| `src/venues.rs` | sửa | hằng Uniswap V3 BSC + family trong `registry_snapshot` |
| `src/discover_mv.rs` | MỚI | quy tắc V2+V3 cùng quote, ngưỡng, Uniswap không thay |
| `src/bin/discover_multivenue.rs` | MỚI | tool volume Swap log + probe depth + xuất list |
| `src/pool.rs` | sửa | `get_factory`/`erc20_symbol`/`resolve_v3_pools_for_quote_tiers` |
| `src/multivenue.rs` | sửa | field mới `v2_ok`/`v3_ok`/`both_ok`/`uni_v3_pools`/vol |
| `src/config.rs` `config.toml` `src/pipeline.rs` | sửa | 4 field `multivenue_*` (thiếu = fail load) |
| `src/lib.rs` `Cargo.toml` | sửa | mod + bin |
| `docs/STATE.md` `docs/TASKS.md` `docs/DOC_MAP.md` | sửa | cụm B4 tool |
| `baocao/BAOCAO48.md` | MỚI | file này |
| `baocao/evidence/baocao48_*` | MỚI | probe-ten, log 2 h, log 24 h abort, candidates |

**KHÔNG đụng**: `.env`, `pairs.txt`, `victims.txt`, cờ live, bot/config VPS.
`state/multi_venue.json` (gitignored) = bản `--hours 2`.

## 4. LỆNH CHẠY

```
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
cargo test --lib discover_mv:: --offline
./target/release/discover_multivenue --probe-ten --skip-volume
./target/release/discover_multivenue --hours 2 --top 500 --min-v2-bnb 50 \
  --min-v3-impact-pct 2 --probe-bnb 1 --out state/multi_venue.json
# --hours 24: CHƯA CHẠY XONG (abort)
```

## 5. OUTPUT THẬT

**Máy: WSL.** `rustc` từ phiên. Binary tool
`sha256sum target/release/discover_multivenue` =
`15bd222101f0bf9ba2b5840d42e4a8b21673609c4958430482b71f7b5f19c93a`
(git HEAD lúc chạy 2 h = `bf87b5d` + working tree cụm này, chưa commit).

```
--- cargo test --lib ---
test result: ok. 441 passed; 0 failed; 19 ignored; finished in 0.27s
--- cargo test --bin bsc_sandwich ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.21s
--- cargo test --lib discover_mv:: ---
test result: ok. 16 passed; 0 failed; 1 ignored; finished in 0.00s
```

Lib 420 (BAOCAO47) → **441** (+ discover_mv 16 + config 2 + venues 1 + pool 2
và test sẵn có khác; không bịa từng test ngoài `discover_mv` 16).

```
--- pin Uniswap V3 BSC (WSL, bsc-dataseed.binance.org, eth_chainId=0x38) ---
Factory      0xdB1d10011AD0Ff90774D0C6Bb92e5C5C8b4461F7  getCode_len=24535
QuoterV2     0x78D78E420Da98ad378D7799bE8f4AF69033EB077  getCode_len=8273
SwapRouter02 0xB971eF87ede563556b2ED4b1C0b0019111Dd85d2  getCode_len=24497
source=https://developers.uniswap.org/docs/protocols/v3/deployments/v3-bnb-deployments
ngay=2026-09-16
```

```
--- probe 10 token RPC (WSL, --probe-ten --skip-volume, 199.2 s) ---
block=122235470 chain=56
blue CAKE  both_ok=true
blue ETH   both_ok=true
blue BTCB  both_ok=true
blue USDC  both_ok=true
blue DOGE  both_ok=true
mid  TWT   both_ok=true
mid  LINK  both_ok=true
mid  DOT   both_ok=true
mid  UNI   both_ok=true
mid  XRP   both_ok=true
10/10 both_ok. Uniswap ghi riêng, khong thay PCS V3.
file: baocao/evidence/baocao48_probe_ten.txt
```

```
--- discover --hours 2 (WSL, Swap log V2+V3, KHONG doc pairs.txt) ---
START 2026-09-16T14:39:21Z
END   2026-09-16T15:16:25Z
elapsed_sec=2224  (tool in elapsed=2035.7s)
block=122239050  blocks=16001  (~0.450 s/block)
volume_method=swap_logs_v2_v3
logs=312096  pcs_pools_seen=1309
scanned_tokens_vol=1288  probed=500
v2_ok=214  v3_ok=37  both_ok=22
tier_dist_pcs_v3_ok={100:16, 500:16, 2500:32, 10000:16}
```

**Top 22 both_ok** (hết list; không đủ 30). Cột vol = volume **cửa sổ 2 h**
(field tên `vol24h_bnb` trong JSON — không phải 24 h):

| # | symbol | vol_2h_bnb | v3_impact_min |
|---|---|---|---|
| 1 | SKYAI | 883.30 | 0.165 |
| 2 | 4 | 225.85 | 0.334 |
| 3 | BabyDoge | 136.94 | 0.0 |
| 4 | USDC | 39.87 | 2e-6 |
| 5 | 币安人生 | 23.30 | 0.225 |
| 6 | TRX | 14.59 | 0.068 |
| 7 | TST | 11.89 | 0.247 |
| 8 | DOT | 11.87 | 0.213 |
| 9 | BTCB | 9.32 | 0.00035 |
| 10 | ETH | 8.22 | 0.000655 |
| 11 | Cake | 4.03 | 0.007 |
| 12 | CAT | 3.96 | 0.310 |
| 13 | INJ | 1.67 | 1.200 |
| 14 | COSA | 1.02 | 0.0 |
| 15 | Cheems | 0.96 | 0.047 |
| 16 | SAFE | 0.88 | 0.083 |
| 17 | ASTER | 0.86 | 0.019 |
| 18 | USD1 | 0.63 | 4.3e-5 |
| 19 | Jager | 0.57 | 0.111 |
| 20 | SFP | 0.55 | 0.169 |
| 21 | Moolah | 0.45 | 0.321 |
| 22 | INKY | 0.36 | 0.003 |

10 dòng đầu `multi_venue_candidates.txt` (`vetted` trống):

```
# discover_multivenue — CHUA VET. Chu chay vet_bsc_token roi dien vetted YYYY-MM-DD.
0x92aa03137385f18539301349dcfc9ebc923ffb10 # SKYAI | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=883.2953 v2=WBNB v3=WBNB@10000+USDT@100
0x0a43fc31a73013089df59194872ecae4cae14444 # 4 | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=225.8530 v2=WBNB v3=WBNB@10000+USDT@2500
0xc748673057861a797275cd8a068abb95a902e8de # BabyDoge | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=136.9409 v2=WBNB v3=WBNB@500+WBNB@10000
0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d # USDC | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=39.8688 v2=WBNB+USDT v3=WBNB@100+WBNB@500+USDT@100+USDT@500
0x924fa68a0fc644485b8df8abfa0a41c2e7744444 # 币安人生 | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=23.2985 v2=WBNB v3=WBNB@10000+USDT@10000
0xce7de646e7208a4ef112cb6ed5038fa6cc6b12e3,0x55d398326f99059ff775485246999027b3197955 # TRX | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=14.5913 v2=USDT v3=WBNB@2500+WBNB@10000+USDT@2500
0x86bb94ddd16efc8bc58e6b056e8df71d9e666429 # TST | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=11.8911 v2=WBNB v3=WBNB@10000
0x7083609fce4d1d8dc0c979aab8c869ea2c873402 # DOT | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=11.8720 v2=WBNB v3=WBNB@2500
0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c # BTCB | vetted  | tax ?/? | owner unknown | discover_multivenue vol24h_bnb=9.3234 v2=WBNB+USDT v3=WBNB@100+WBNB@500+WBNB@2500+USDT@100+USDT@500
```

Log đầy đủ: `baocao/evidence/baocao48_discover_2h.txt`.

`--hours 24`: abort giữa quét V2 (~20 phút, 212176 log, 633 token vol,
chưa probe). Log: `baocao/evidence/baocao48_discover_24h_aborted.txt`.
Không bịa số 24 h.

## 6. CHAIN

`eth_chainId=0x38` (pin getCode + probe-ten + discover 2 h).
getCode Uniswap: 24535 / 8273 / 24497 (ô 5).

## 7. REGISTRY

Đã ghi `DEX_REGISTRY.md` mục “Uniswap V3 BSC”:

- Factory `0xdB1d…1F7` PINNED getCode 24535
- QuoterV2 `0x78D7…B077` PINNED getCode 8273
- SwapRouter02 `0xB971…85d2` PINNED getCode 24497
- scan/live **tắt** (giai đoạn 2: pin trước, sim sau)
- KHÔNG pin THENA/Biswap

## 8. KHÔNG LÀM

- Không sim_arb V3. Không đo cơ hội. Không contract. Không sendRaw.
- Không đụng `pairs.txt` / `.env` / bot VPS.
- Không chạy xong `--hours 24`.
- Không kết luận thị trường / Go-No-Go B1.

## 9. CHỮ

**CHƯA XONG**

(`--hours 24` còn nợ; list hiện có là cửa sổ **2 h**.)

## 10. CÒN NỢ / LÁT SAU

- **`--hours 24` chưa chạy xong.** Lần abort: V2 ~212k log / 633 token, chưa
  V3 scan hết, chưa probe 500. Cần 1 lần chạy đầy đủ (ước 20–40 phút nếu
  RPC đỡ 429; lần 2 h thật = 37 phút gồm probe).
- Ranking vol trên list hiện tại là **2 h**, không phải 24 h — field JSON
  vẫn tên `vol24h_bnb`.
- Probe 500 token trên cửa sổ 2 h: nhiều token top-vol là meme 1-pool
  (cả hai=false); 22 both_ok gồm blue-chip + vài mid. Chưa vet tay.
- `sim_arb` V3 / đo cơ hội — cụm sau khi Chủ vet candidates.
- VPS `config.toml` phải thêm 4 field `multivenue_*` trước khi nhận binary
  mới (thiếu = fail load). Phiên này không deploy.
- `/api/econ` O(log), p95 sim, `sim_engine="evm"` trần gas — nợ cũ.

---

Commit: `0a1884c6d2efcd81cd6250b381aa73dcc69300c1` (toàn bộ nội dung cụm).
Commit sau đó chỉ điền hash này vào ô 3 + dòng này, không đổi code.
