# BAOCAO47 — cụm `planB-B0-complete`

## 1. LÁT

`planB-B0-complete` — hoàn tất B0 mà BAOCAO46 dừng dở: đo cơ hội backrun-arb
bằng số, 0 tiền, không contract, không live. Máy WSL (code + paper 63,6 phút)
+ VPS (chỉ ĐỌC log, không dừng bot).

HEAD THẬT lúc mở phiên: `7ad347df67dd90c2bf3cbea1fdbe219a529477ff`
(lệnh ghi `71e3c15` — đó là commit nội dung BAOCAO46; sau đó có
`5f0b661` điền hash + `7ad347d` đổi CLAUDE.md→AGENTS.md).

## 2. LỆNH NHẬN

Khối lệnh `planB-B0-complete` mục 1→8. Cấm `.env`, `live_mode="live"`,
sendRaw, contract, dòng token `pairs.txt`, dừng/đổi bot VPS, đổi
`config.toml` VPS. Không subagent ghi file. VPS: user root, key
`key/bsc_vps_ed25519` — **không ghi IP**.

## 3. FILE ĐỔI

Commit nội dung: `9b5381e773e3aee14d07a9ba59510c4d2aaded6e`.
Commit cuối phiên (điền hash vào ô này + dòng cuối): xem dòng `Commit:` cuối file.

| File | Trạng thái | Nội dung |
|---|---|---|
| `DEX_REGISTRY.md` | sửa | pin Balancer Vault + ProtocolFeesCollector + Aave V3 Pool; Infinity Vault thêm "nguồn flash"; IVault lock/take/sync/settle |
| `src/multivenue.rs` | MỚI | load `multi_venue.json` |
| `src/bin/build_multi_venue.rs` | MỚI | sinh file từ pairs.txt + getPair/getReserves/getPool + Initialize 5000 block |
| `src/bin/arb_measure.rs` | MỚI | replay sim_arb trên log VPS |
| `src/bin/arb_crosscheck.rs` | MỚI | đối chiếu route_out vs revm 2–3 hop |
| `src/flash.rs` | sửa | ETH/BTCB constants |
| `src/sim_arb.rs` | không đổi logic | dùng lại từ BAOCAO46 |
| `src/sim_evm.rs` | sửa | `measure_arb_two_swap_gas` + `simulate_arb_hops_evm` |
| `src/main.rs` | sửa | `flash_source_task` (chụp ngay khi có provider) + `handle_backrun_tx` |
| `src/pipeline.rs` | sửa | `decode_for_backrun` (mua+bán), skip `arb_no_*` |
| `src/web.rs` | sửa | `GET /api/flash`, field snapshot |
| `src/pool.rs` | sửa | `scan_infinity_initializes_window`, `resolve_v3_pools_for_quote` |
| `src/venues.rs` / `src/config.rs` / `src/lib.rs` / `Cargo.toml` | sửa | const manager, skip reasons, 3 bin, helpers |
| `config.toml` | sửa | `gas_units_arb_v2flash=380000` (đo); comment bin `build_multi_venue` |
| `web/index.html` `web/app.js` | sửa | khối flash |
| `docs/CONTRACT_DESIGN.md` | sửa | mục ArbExecutor (chỉ thiết kế) |
| `docs/STATE.md` `docs/TASKS.md` `docs/DOC_MAP.md` | sửa | 4 kịch bản rủi ro, bảng B0, file mới |
| `tests/contracts/ArbGasProbe.sol` | MỚI | probe đo gas, KHÔNG deploy |
| `baocao/BAOCAO47.md` | MỚI | file này |
| `baocao/evidence/baocao47_*` | MỚI | số liệu |

**KHÔNG đụng**: `.env`, `pairs.txt`, `victims.txt`, `AGENTS.md`, cờ live,
bot/config VPS.

## 4. LỆNH CHẠY

```
cargo test --lib --offline          # 420 passed
cargo test --bin bsc_sandwich --offline
./target/release/build_multi_venue
./target/release/arb_measure --jsonl /tmp/large_swaps_b0.jsonl
./target/release/arb_crosscheck
scripts/paper_run.sh --minutes 60 --port 8797
ssh -i key/bsc_vps_ed25519 -p 22 root@<host>   # CHI DOC log, khong dung bot
```

## 5. OUTPUT THẬT

**Máy: WSL.** `rustc 1.97.1`. Paper binary
`sha256sum target/release/bsc_sandwich` =
`f966bfe4518042bd4901f06ec20f2c12607f8de47cf67ac442cddc7c387fc7f9`
(git HEAD lúc paper = `7ad347d`). VPS chỉ đọc log, binary VPS vẫn
`edb4461` (lệch WSL — đúng, phiên này không deploy).

```
--- cargo test --lib ---
test result: ok. 420 passed; 0 failed; 18 ignored; finished in 0.25s
--- cargo test --bin bsc_sandwich ---
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 0.23s
```

Số test lib: **417 → 420** (+3: `decode_for_backrun_nhan_ca_mua_va_ban`,
2 test `multivenue`).

```
--- build_multi_venue (WSL, chain=56 block=122217770) ---
pairs.txt tokens_parsed=129
infinity Initialize logs in 122212771-122217770: 16
WROTE state/multi_venue.json tokens=129 arb_ready=9 infinity_err=None
```

87/128 token có 2 địa chỉ V2 (BAOCAO46) nhưng chỉ **9/129** đủ
`min_reserve` cả hai pool: CAKE, BUSD, ETH, BTCB, USDC, DOGE, KOGE, FIST, SLT.

```
--- flash.source 3 dong THAT (WSL paper, event=flash.source) ---
ts=2026-09-16T12:11:09Z block=122219282
  infinity fee=0 WBNB=185.31 USDT=3.566e7 ETH=73.89 BTCB=2.29
  balancer fee=0 WBNB=0.000435 USDT=5.0e-13
  aave     fee=5 WBNB=116725.03 USDT=1.211e7
ts=2026-09-16T12:16:35Z block=122220011  inf WBNB=188.95
ts=2026-09-16T12:22:06Z block=122220745  inf WBNB=188.69
GET /api/flash: 3 sources, arb_ready=9, block cap nhat 122227401
```

File đầy đủ: `baocao/evidence/baocao47_flash_source_3.txt`.

```
--- kiem cheo sim_arb vs revm (14 case hoan thanh, leftover-fund da tru) ---
token  borrow  pct_diff  gas_hops  ok<=2%
CAKE   0.1/0.5  0.046%    339808    4/4
BUSD   0.1/0.5  0.023%    339836    4/4
ETH    0.1/0.5  0.252%    339812    2/2
BTCB   0.1/0.5  0.222%    339862    2/2
USDC   0.1/0.5  0.267%    347747    2/2
n=14 n_le_2pct=14  (lenh doi >=20: 6 case USDT-borrow / token khac FAIL
 RPC eth_getStorageAt -32000 not supported — ghi MISSING, khong bia)
gas hops p50=339836
gas_units_arb_infinity = 420000  (= 339836 hops + ~80k lock/take/sync/settle
  CHUA chen duoc toan bo lock path 1 tx revm)
gas_units_arb_v2flash  = 380000  (= hops + ~40k pancakeCall)
```

Bảng đủ: `baocao/evidence/baocao47_crosscheck_corrected.tsv`.

```
--- (a) VPS log 18.85 h (chi DOC, unit bsc-sandwich-paper active, live_mode=off)
     large swap >=0.5 BNB, token arb_ready, sim_arb ---
n_in_arb_ready=14  n_opp=1  p50=24.27 USDT  p90=24.27  sum=24.27
borrow_p80=2336 USDT  flash=infinity_vault  cluster_pct=0.0
quy /ngay = 1 / 18.85 * 24 = 1.27 co hoi/ngay
on-chain Swap cung pool o vi tri +1..+3 sau victim: 4/14 = 28.6%
```

```
--- (b) WSL 63.6 phut live-mempool strategy=backrun ---
t0=2026-09-16T12:11:03Z  t1=13:14:37Z  hours=1.060
sim.arb=7795  simulated=0  unprofitable=7795  (7794/7795 = USDC)
arb_no_second_venue=885  below_min=8682  cluster_pct=0
gross_wei p50=-1  n_pos_gross=0
quy /ngay = 0
```

`paper_run.sh` in `BOT DA CHET sau 59 phut` vì `halt.lock` sau khi Chủ báo
đủ 60 phút; cửa sổ log thật **63,6 phút**.

## 6. CHAIN

`eth_chainId=0x38` (build_multi_venue + flash task + crosscheck). getCode
nguồn flash lấy từ BAOCAO46 (len 8347 / 24512 / 2880 / 1933) — pin registry
phiên này, không đo lại getCode (dữ liệu đã có, lệnh cho phép).

## 7. REGISTRY

Đã ghi `DEX_REGISTRY.md` mục "Nguồn flash loan":

- Infinity Vault `0x238a…5e6c4` — thêm dòng nguồn flash; IVault
  lock/take/sync/settle, source_url pancakeswap/infinity-core, ngày
  2026-09-16. **Balancer BSC ~rỗng** (0,000435 WBNB) ghi rõ trong bảng.
- Balancer V2 Vault + ProtocolFeesCollector — PINNED, getCode 24512 / 2880.
- Aave V3 Pool `0x6807…e0cB` — PINNED, getCode 1933, phí 5 bps.
- Không thêm/xoá pin family Pancake nào khác.

## 8. KHÔNG LÀM

- Không viết/deploy ArbExecutor (chỉ thiết kế). Không sendRaw.
- Không đụng bot VPS (đang `bsc-sandwich-paper.service`, `live_mode=off`).
- Không sửa `pairs.txt` / `.env` / `AGENTS.md` / config VPS.
- Không đủ 20 case revm (14 đạt ≤2%, 6 FAIL RPC) — không bịa thêm.

## 9. CHỮ

**CHỜ GROK**

## 10. CÒN NỢ / LÁT SAU

### Go/No-Go (số)

Điều kiện B1: **≥ 30 cơ hội/ngày VÀ p50 ≥ 5 USDT sau bribe**.

| Nguồn | giờ thật | n cơ hội lãi | /ngày | p50 USDT |
|---|---|---|---|---|
| VPS log 18,85 h | 18,85 | 1 | **1,27** | 24,27 (n=1) |
| WSL mempool 1,06 h | 1,06 | **0** | **0** | — |

**Không đạt** (≥30/ngày). **No-Go B1.** Không viết contract.

Hướng B4 (venue thứ 2): 78 token có 2 địa chỉ V2 nhưng 1 phía mỏng;
107/129 token có pool V3. Top 20 thiếu venue V2 đủ sâu (theo reserve lớn
nhất), V3 tier / Infinity nếu có:

| symbol | v2 (quote, reserve, meets_min) | v3 tiers | inf |
|---|---|---|---|
| CBM | WBNB 0.11 F, USDT 1.89e6 T | WBNB 10000 | — |
| COCO | USDT 7.52e5 T | WBNB 2500, USDT 100/2500 | — |
| XMT | USDT 7.04e5 T | — | — |
| DNDV3 | USDT 3.38e5 T | — | — |
| Virus2027 | USDT 2.09e5 T | — | — |
| AIX | WBNB ~0 F, USDT 1.82e5 T | USDT 10000 | — |
| KXG | USDT 1.30e5 T | WBNB 2500/10000 | — |
| POFD | WBNB ~0 F, USDT 1.25e5 T | WBNB 2500/10000, USDT 10000 | — |
| (果蝇) | USDT 9.00e4 T | — | — |
| NFTC | USDT 7.32e4 T | — | — |
| RGE | USDT 5.74e4 T | USDT 10000 | — |
| SDM | USDT 4.67e4 T | — | — |
| BinanceTown | USDT 4.51e4 T | — | — |
| DBT | WBNB ~0 F, USDT 3.85e4 T | USDT 10000 | — |
| BNC | USDT 3.42e4 T | — | — |
| EVAA | USDT 1.66e4 T | WBNB 2500/10000, USDT 100–10000 | — |
| SHIB | WBNB 37.9 T, USDT 1.09e4 F | WBNB 100–10000, USDT 500–10000 | — |
| BBT | WBNB 142 T, USDT 1.06e4 F | — | — |
| BabyDoge | WBNB 5961 T, USDT 6732 F | WBNB 100–10000, USDT 500–10000 | — |
| (USDT bản thân) | WBNB 53348 T | WBNB 100–10000 | — |  ← đây là pool bridge, không phải token arb |

USDC (arb_ready) có 1 Infinity Bin pool USDT fee=44 trong cửa sổ 5000 block.

### Nợ thật của cụm

- revm ≥20: mới 14 case (RPC `-32000 not supported` cho getStorageAt USDT /
  vài token). Lock path Infinity chưa đo trọn 1 tx (thiếu bytecode callback
  trong fork) — `gas_units_arb_*` = hops đo + cộng lock/pancakeCall.
- `% backrun on-chain` chỉ 14 mẫu VPS (28,6 %), không phải toàn bộ 6549 swap.
- `/api/econ` O(log), p95 sim 848 ms, `sim_engine="evm"` trần gas — nợ cũ,
  không đụng.

---

Commit: `9b5381e773e3aee14d07a9ba59510c4d2aaded6e` (toàn bộ nội dung cụm).
Commit sau đó chỉ điền hash này vào ô 3 + dòng này, không đổi code.
