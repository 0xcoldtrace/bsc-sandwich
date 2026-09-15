# BAOCAO33 — cụm `evm-validate-fixed-then-wire`

## 1. LÁT
`evm-validate-fixed-then-wire` — B4'' (validate lại, sửa 3 thứ, giữ ngưỡng) →
B3 (nối `sim_evm` vào pipeline + validator nhúng) → C (đo tax tự động bằng
revm) → D1/D2/D3. Làm nốt test/docs/web dính trong cùng phiên.

## 2. LỆNH NHẬN
Khối lệnh Grok `evm-validate-fixed-then-wire` (B4''.1..5, B3.1..5, C1..3,
D1/D2/D3). RPC validate: `BSC_HTTP_VALIDATE=https://bsc-rpc.publicnode.com`
(+ dự phòng blockrazor/defibit). KHÔNG hạ ngưỡng B4'' tự ý, KHÔNG live/ký.

## 3. FILE ĐỔI
- `src/decoder.rs` — **SỬA BUG THẬT (chặn B4''.2/3)**: thêm 3 selector
  `*SupportingFeeOnTransferTokens` (`0xb6f9de95`/`0x791ac947`/`0x5c11d795`) —
  toàn bộ V2-router buy trong mempool BSC thật dùng nhóm này, decoder cũ trả
  `decode_fail` 100% nên `poll_wbnb_v2_candidates` gom 0 candidate.
- `src/sim_evm.rs` — B4''.1 `is_meaningful_candidate`, B4''.2
  `decode_v2_swap_amount_out`/`predict_victim_swap_out`/`swap_topic0`,
  `collect_mined_v2_buy_candidates`, `RpcErrorStats`, `validate_rpc_urls`,
  B3.2 `BlockForkCache`/`refine_front_in_on_fork`, C1 `measure_tax_evm`/
  `measure_tax_on_fork`/`EvmTaxMeasurement`, B3.5 `SimEvmError::Revert`; viết
  lại 3 test `real_rpc_*` (B4''.2/B4''.3 tail-mode/B4''.4) + thêm test
  `real_rpc_sim_evm_block_cache_and_tax` + 6 test thuần.
- `src/pipeline.rs` — B3.2 `decide_with_evm`/`log_sim_evm`/`EvmDecision`,
  skip `SimError`; guard cổng tax công-thức-đóng khi `sim_engine="evm"`.
- `src/tax.rs` — C2 `TaxKey (token,quote)` + TTL `get_fresh_ttl`,
  `TaxMeasurement{buy_bps,sell_bps,honeypot}` + `manual`/`from_evm`.
- `src/config.rs` — B3.1 `sim_engine` (fail-load nếu ≠ "evm"/"v2"),
  `tax_cache_ttl_sec`, `front_slippage_bps`/`back_slippage_bps` (D1).
- `src/venues.rs` — C3 `ZERO_TAX_ALLOWLIST` (8 token verify on-chain) +
  `is_zero_tax_allowlisted`, `wbnb_addr`/`usdt_addr`.
- `src/main.rs` — wire `run_evm_decision` (C3+B3.2) + `spawn_victim_validator`
  (B3.4) vào `handle_paper_tx`; funnel `record_sim_error`.
- `src/web.rs` — `ValidateStats` + `GET /api/validate`, `/api/tax` thêm
  quote/buy/sell/honeypot/TTL; funnel `sim_error`.
- `src/calldata.rs` — D2 `encode_front_buy_usdt`/`encode_back_sell_usdt` +
  roundtrip test.
- `config.toml` — D1 (`min_profit_bnb=0.002`, `front/back_slippage_bps`,
  `tax_cache_ttl_sec=600`, `sim_engine="evm"`).
- `scripts/vps_paper_run.sh` + `docs/VPS_RUN.md` — D3.
- `web/index.html`+`web/app.js` — bảng tax mở rộng + validate.
- `CLAUDE.md` — mục Config (thêm 4 field) + Skip (`sim_error`).

## 4. LỆNH CHẠY
```
cargo test --lib                       # 246 passed
cargo build --release                  # 0 warning
BSC_HTTP_VALIDATE=... cargo test --lib real_rpc_victim_prediction_matches_onchain -- --ignored --nocapture
BSC_HTTP_VALIDATE=... cargo test --lib real_rpc_replay_real_sandwich_triplets -- --ignored --nocapture
BSC_HTTP_VALIDATE=... cargo test --lib real_rpc_ternary_search_evm_warm_cache_timing -- --ignored --nocapture
BSC_HTTP_VALIDATE=... cargo test --lib real_rpc_sim_evm_block_cache_and_tax -- --ignored --nocapture
# RUNTIME THẬT: boot binary release, BSC_HTTP/BSC_WS=publicnode, cfg sim_engine=evm, universal, port scratch
```

## 5. OUTPUT THẬT

### `cargo test --lib` (246 passed, 0 failed) + `cargo build --release` (0 warning)
```
test result: ok. 246 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
Finished `release` profile [optimized] target(s) in 25.03s   (0 warning)
```

### B4''.2 — dự đoán victim khớp on-chain (bảng ≥5 hash pred/real/lệch%)
Ngưỡng: ≥5 tx đủ điều kiện, ≥80% lệch ≤1%. **ĐẠT: 9/9 = 100%, lệch 0.000000%.**
```
hash=0x7a8d6916...ac170 pair=0x64fe8f5e... block=121953970 pred=114904244169049729279318 real=114904244169049729279318 lech=0.000000% ok=true
hash=0xb375d7c3...4e689c pair=0xc7b065d1... block=121953969 pred=1033137571733239 real=1033137571733239 lech=0.000000% ok=true
hash=0x81dc6edf...aad461 pair=0xa69574b0... block=121953969 pred=66301890943068718 real=66301890943068718 lech=0.000000% ok=true
hash=0x53f80329...ec7b7 pair=0x30000a40... block=121954035 pred=323312092044441 real=323312092044441 lech=0.000000% ok=true
hash=0xffa07b3f...1f6031 pair=0x755dddef... block=121954028 pred=820295030524663761 real=820295030524663761 lech=0.000000% ok=true
hash=0x6adba793...5d39cf pair=0xbf5c0ff6... block=121954102 pred=133840626403416 real=133840626403416 lech=0.000000% ok=true
hash=0x2a66ecb5...cd9df3 pair=0x2124dc84... block=121954096 pred=299275190541388336661 real=299275190541388336661 lech=0.000000% ok=true
hash=0x1e794c5f...86ed5e pair=0xa69574b0... block=121954149 pred=139990293742816241 real=139990293742816241 lech=0.000000% ok=true
hash=0x55c98a2e...da18835 pair=0x66bd8c3d... block=121954145 pred=21519732229013254475795430 real=... lech=0.000000% ok=true
TONG B4''.2: 9 tx du dieu kien, 9 lech <=1% (100.0%), 1 bi loai vi qua nho
RPC_ERRORS trong lan chay nay: -32005=0 khac=10
```
`khac=10` là lỗi `-32602 Archive requests require a personal token` từ
publicnode cho `eth_getTransactionReceipt`/`balanceOf` block cũ — ĐÃ failover
sang blockrazor/defibit cho receipt, và kiến trúc VÒNG (fork block ngay khi
thấy, không đợi) né được cửa sổ state ~128 block. `-32005=0` → mẫu ít KHÔNG
phải do rate-limit.

### B4''.2 — 3 sửa (B4''.1/.2/.4) + chọn mẫu từ block đã mined
- **B4''.1**: `is_meaningful_candidate` (victim_in ≥0.05 BNB & impact ≥0.02%);
  log rõ candidate bị loại. Đo THẬT `txpool_content`: victim V2-router pending
  trên publicnode cỡ `0.0004`–`0.004` BNB (dưới xa 0.05) → đổi nguồn mẫu sang
  **block đã mined** (lưu lượng lớn hơn nhiều bậc), ngữ nghĩa validate không
  đổi (fork block cha, replay victim thật, kiểm cô lập 1 Swap/block).
- **B4''.2**: `victim_out_real` từ `eth_getTransactionReceipt` (log Swap của
  pair trong receipt), dự đoán bằng `predict_victim_swap_out` (đọc log Swap do
  chính revm sinh) → so apples-to-apples (thay `balanceOf` delta cũ lệch bằng
  tax). Cô lập bằng ĐÚNG 1 `eth_getLogs {block, address=pair, topic0=Swap}`.
- **B4''.4 failover**: `-32005`→backoff 2s + đổi URL; receipt qua danh sách
  failover (publicnode từ chối receipt).

### B4''.3 — replay sandwich THẬT (quét Swap event, không cần BscScan)
Lần quét dải-2000-block (log đầy đủ, `-32005=0`): **TÌM ĐƯỢC 8 bộ sandwich
THẬT** thoả đúng định nghĩa lệnh (i,i+1,i+2 liên tiếp cùng pair, i&i+2 cùng
`from`, i+1 khác), có hash chứng minh tồn tại:
```
36629 log Swap doc duoc / 1970 block, 5101 bo 3-swap-lien-tiep cung pair, 8 bo THOA dinh nghia sandwich, -32005=0
SANDWICH block=121954730 pair=0x74f71ac7... attacker=0x9999b0cd... front=0x05929cee... victim=0xd57275a9... back=0xd91df4f6...
SANDWICH block=121953614 pair=0x5248613d... attacker=0x9999b0cd... front=0x03011ece... victim=0x36b8a00d... back=0x94887 25b...
... (tong 8, danh sach day du trong log)
```
**Phát hiện hạ tầng THẬT (không phải giả định)**:
- `bsc-rpc.publicnode.com` TỪ CHỐI `eth_getLogs` không lọc address
  (`-32701: Please specify an address`); `bsc.blockrazor.xyz` CHO PHÉP nhưng
  cap 25 block/query (`-32000: log query range must not exceed 25 blocks`).
  → "1 lời gọi cho 2000 block" bất khả thi trên mọi endpoint Chủ cấp; thay
  bằng chia CHUNK 25 block phủ đúng dải (cùng kết quả logic, ghi rõ trong test).
- Cả 8 bộ tìm được đều cách block hiện tại 1400–3160 block (>128 block state
  window) nên KHÔNG replay được state. Sửa: **chế độ TAIL trực tiếp** (quét
  block mới nhất, replay NGAY khi state còn).

**Kết quả TAIL (chạy 1219s, log đầy đủ trong task output)**:
```
block da quet: 2397, log Swap doc duoc: 43784, bo 3-swap-lien-tiep cung pair: 6994
bo THOA dinh nghia sandwich (i&i+2 cung from): 28   (danh sach 28 hash day du trong log)
==== BANG REPLAY (1 dong) ====
block=121962035 pair=0x45bc7b64... front=0x2d16c4aa... victim=0x5aa60524... back=0xac55da17...
  profit_evm=0 profit_real=0 lech=0.0000% victim_success_evm=true
TONG B4''.3: 1 bo replay duoc, 1 lech <=2%.  RPC_ERRORS: -32005=0 khac=27
```
- **28 bộ sandwich THẬT tìm được** (gấp 3.5× lần quét dải), hash chứng minh,
  `-32005=0`. Cơ chế phát hiện ĐÚNG và dữ liệu DỒI DÀO.
- **KHÔNG đạt ≥3 replay lệch ≤2%** — 2 giới hạn THẬT (không phải lỗi code):
  1. **Cửa sổ state ~128 block**: dù tail, quét 25-block-chunk trên RPC công
     khai (cap 25 block/query + sleep chống rate-limit) CHẬM HƠN tốc độ ra
     block BSC (~0.75s/block), nên phần lớn bộ phát hiện xong đã > 100 block
     tuổi → không còn state để replay.
  2. **Bot MEV thật route qua CONTRACT RIÊNG** (không phải EOA): `profit_real`
     đo bằng delta số dư native của `from` (EOA) ra `0` vì lợi nhuận vào
     CONTRACT, không vào EOA — nên bộ replay được duy nhất cho `0 vs 0`
     (khớp hình thức nhưng KHÔNG có ý nghĩa validate). Đo đúng lợi nhuận của
     bot khác cần decode luồng token trong contract của họ (ngoài phạm vi).
- `-32005=0` → đúng điều kiện **B4''.5**: thiếu mẫu vì DỮ LIỆU/hạ tầng (cửa sổ
  state + hình dạng bot), KHÔNG phải "quét không được". Gate (a) [B4''.2] đã
  ĐẠT (9/9, 0%), là cơ chế bot dùng trong sản xuất → **kích hoạt điều khoản
  B4''.5**: (b) chuyển thành **validator nhúng B3.4** (đã làm, `/api/validate`),
  cho phép sang B3/C/D. Mật độ đo: ~1 bộ / 85 block (28 bộ / 2397 block).

### B4''.4 — ternary search EVM warm-cache (ĐO ms/tx)
Slot 8 verify bit-for-bit `eth_getStorageAt` vs `getReserves()`. **Cold lần
đầu 1209ms; 18 lần warm sau ~6ms/lần (~200× nhanh, DƯỚI mốc ≤50ms).**
```
VERIFY slot8: ... unpack(reserve0=59912700000000000000, reserve1=4356691228728133103881789378) vs getReserves() (KHOP CHINH XAC)
attempt#0 front_in=1.0 BNB ... thoi_gian=1209.39ms   (cold, fetch remote)
attempt#1 ... thoi_gian=6.03ms
attempt#2..18 ... ~5.95-6.16ms
TONG B4'.4: 19 lan sim, trung binh 69.36ms/lan (keo len vi 1 lan cold)
```

### B3.2 + C1 + C3 — RUNTIME THẬT (binary release, mempool WS sống)
Boot `./target/release/bsc_sandwich <cfg scratch>` với
`BSC_HTTP/BSC_WS=publicnode`, `sim_engine="evm"`, universal, ngưỡng thấp, port
`18899`, KHÔNG đụng config/victims thật. `GET /api/status`:
`pending_source":"ws"`, `last_block=121959315` (BSC thật), `dry_run=true`.

`GET /api/funnel` sau ~100s (SAU khi sửa deadlock cổng tax):
```
{"seen":9019,"not_pancake_router":8547,"decode_fail":196,"not_wbnb_pair":188,
 "venue_v2":88,"venue_v3":0,"no_pool":0,"below_min":0,"thin_liq":0,
 "honeypot_or_tax":0,"unprofitable":82,"victim_would_revert":7,
 "simulated":0,"sim_error":0}
```
- `venue_v2=88` candidate V2 thật; `unprofitable=82`+`victim_would_revert=7`:
  công thức đóng `sim_v2` lọc trước (ước lượng), candidate hứa hẹn mới tới EVM.
- **`sim_error=0`** — EVM/RPC chạy tốt, KHÔNG rơi về `sim_v2` âm thầm.
- **BUG THẬT đã sửa giữa 2 lần chạy**: lần đầu `honeypot_or_tax=155/157`,
  `sim.evm=0` (chỉ inject) — cổng tax CÔNG-THỨC-ĐÓNG (cache rỗng) chặn TRƯỚC
  EVM → deadlock (cache không đầy vì EVM không chạy vì cache rỗng). Sửa:
  `sim_engine="evm"` thì `evaluate_candidate(_quote)` BỎ cổng tax đó, để EVM
  tự đo. Sau sửa: `honeypot_or_tax=0` (không còn false-gate), EVM chạy thật.

3 dòng `sim.evm` thật (log `logs/bot.jsonl`) — gồm cả candidate EVM đo tax:
```
sim.evm token=0xc1fe9051...ade2 decision=honeypot_or_tax fork_block=121960068 quote=wbnb  (EVM do tax -> gate)
# test real_rpc_sim_evm_block_cache_and_tax:
sim.evm token=0x3db1b406...4444 front_in=2 token_received=17940972 back_out=1 profit_evm=-1 victim_success=true buy_tax_bps=Some(0) sell_tax_bps=Some(0) attempts=3
  tax_evm token=0x3db1b406...4444 buy_bps=0 sell_bps=0 honeypot=false
  tax_evm token=0x9ee92eb6...a1fb buy_bps=0 sell_bps=10000 honeypot=true   (C1 phat hien HONEYPOT that: back-sell revert TRANSFER_FROM_FAILED)
```
`GET /api/tax` (C1/C2 — đo tự động, khoá (token,quote), TTL):
```
{"allow_tax_inject":true,"tax_cache_ttl_sec":600,"entries":[
 {"token":"0x55d398...7955","quote":"0xbb4cdb9c...095c","roundtrip_tax_bps":0,
  "buy_bps":0,"sell_bps":0,"honeypot":false,"fresh":true}]}
```

### C3 — 8 token allowlist zero-tax (verify on-chain THẬT)
`eth_getCode`+`symbol()` qua publicnode (không chép trí nhớ):
```
WBNB 3124/WBNB  USDT 4413/USDT  USDC 1596/USDC  BUSD 4413/BUSD
USD1 2104/USD1  CAKE 7285/Cake  BTCB 4413/BTCB  ETH  4413/ETH
```
WBNB(3124)/USDT(4413) khớp `WBNB_GET_CODE_LEN`/`USDT_GET_CODE_LEN` đã pin.

### D2 — calldata USDT roundtrip
```
test calldata::tests::roundtrip_front_buy_usdt_matches_bit_for_bit ... ok
test calldata::tests::roundtrip_back_sell_usdt_matches_bit_for_bit ... ok  (selector 0x38ed1739, ABI decode khop bit-for-bit)
```

## 6. CHAIN — `0x38`
- `eth_chainId` mọi test/runtime = `56` (assert cứng).
- `eth_getCode`/`symbol()` 8 token allowlist (mục 5, C3).
- B4''.2: 9 tx mined THẬT trên BSC, pred==real bit-for-bit.
- Runtime: `last_block=121959315` (BSC thật), `pending_source=ws`.

## 7. REGISTRY
Không đổi pin venue. Thêm `ZERO_TAX_ALLOWLIST` (8 token, verify getCode>0 +
symbol() trên chain 56). USDT/WBNB dùng lại pin cũ.

## 8. KHÔNG LÀM
- Không bật live/`bot_armed`/`dry_run=false`, không ký/sendRaw, không relay.
- Không hạ ngưỡng B4'' (giữ ≥5/≥80% cho B4''.2, ≥3/≤2% cho B4''.3).
- Không chia sẻ fork EVM xuyên tx (revm `!Send` — ghi CÒN NỢ).
- Không đổi stack / hạ version alloy/revm.

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU
- **B4''.3 replay ≥3 lệch ≤2% — KHÔNG đạt số (1/3)**, nhưng đã kích hoạt đúng
  điều khoản B4''.5 (xem mục 5): 28 bộ sandwich THẬT tìm được (hash-proven),
  `-32005=0` → giới hạn là (1) cửa sổ state ~128 block của RPC công khai +
  (2) bot MEV route qua contract nên `profit_real` đo bằng EOA-balance-delta
  vô nghĩa (`0 vs 0`). Gate (a) [B4''.2] ĐẠT 9/9 0% → (b) chuyển thành
  validator nhúng B3.4. Để đạt số B4''.3 gốc cần: RPC archive riêng (fork sâu
  hơn 128 block) HOẶC decoder luồng token trong contract bot khác (đo đúng
  lợi nhuận của họ) — cần lệnh Grok nếu muốn theo đuổi thay vì dựa B3.4.
- **Fork EVM dùng chung xuyên tx**: revm `Evm` `!Send` → không đặt trong
  `AppState`/không giữ qua `.await` trong task spawn. Hiện mở fork/tx tới bước
  sim (ít sau gate); warm-reuse chỉ trong 1 lời gọi. Muốn chia sẻ theo block
  cần worker-thread actor riêng — CHƯA làm.
- **Warm-cache trong live loop**: `refine_front_in_on_fork` chỉ đánh giá ĐÚNG
  điểm ước lượng `sim_v2` (1 attempt) vì `BlockForkCache::reset` wholesale xoá
  account fetch (~3.6s/lần). Quét nhiều `front_in` warm (~6ms) chỉ ở đường
  B4''.4 (cần token0/pair/reserve để reset slot-level).
- **B3.4 validator + quote USDT**: `predict_victim_swap_out`/validator hiện
  chỉ pool V2 WBNB; USDT chưa (cần đường riêng). `/api/validate` sống nhưng
  chưa gom mẫu trong lần chạy ngắn (candidate tới EVM đều bị tax-gate,
  `evm=None` nên không spawn validator).
- **VPS 30 phút**: `scripts/vps_paper_run.sh`+`docs/VPS_RUN.md` sẵn sàng —
  Chủ tự chạy trên VPS (cần `.env` thật + `cargo build --release`).
