# BAOCAO42 — cụm `bugfix-presign-and-contract-plan`

## 1. LÁT

`bugfix-presign-and-contract-plan` — **PHẦN A** (A1–A8): sửa hết bug đã biết
để đường ký chạy được. **PHẦN B**: `docs/CONTRACT_DESIGN.md` — tài liệu thiết
kế contract executor + kiến trúc ví (CHƯA viết Solidity, CHƯA deploy, đúng
lệnh). Bao gồm **2 lần BỔ SUNG GIỮA PHIÊN** (Chủ dán docs chính thức
BlockRazor rồi 48 Club) — phát hiện mô hình bribe cũ SAI và đã sửa.

## 2. LỆNH NHẬN

Khối lệnh Grok `bugfix-presign-and-contract-plan`. Máy: WSL. HEAD bắt đầu
`63c2d43`. Không subagent ghi file (luật #4) — toàn bộ code/RPC call/phân tích
do phiên chính tự làm. VPS (paper 24h port 18910) KHÔNG đụng. CẤM: `.env`,
`PRIVATE_KEY`, `live_mode="live"`, gửi bundle thật, `pairs.txt`, VPS,
viết/deploy contract.

Giữa phiên Chủ gửi 2 lần bổ sung:

1. **BlockRazor** — endpoint Block Builder + `eth_sendBundle` + header
   `Authorization: $BLOCKRAZOR_AUTH`; bribe = transfer BNB tới builder EOA
   `0x1266C6bE…dD9c5F20` (KHÔNG phải `block.coinbase`); `bribe_mode` đổi
   thành `"builder_transfer"`; gas ≥ 0.05 gwei; `eth_callBundle` chỉ có ở gói
   trả phí.
2. **48 Club** — `https://puissant-builder.48.club/`, không auth; bribe tới
   Builder Control EOA `0x4848489f…D7484848`; xếp hạng = `0.9 × gas fee tx
   unique + BNB tới EOA` ⇒ ưu tiên transfer hơn gas; `backrunTarget` = hash
   victim; `revertingTxHashes` để rỗng; bảng `relay → builder_eoa` pin trong
   `DEX_REGISTRY.md`; contract (B2/B3) phải có hàm back nhận
   `(bribeTo, bribeAmount)` và chuyển ở CUỐI sau khi kiểm lãi.

## 3. FILE ĐỔI

Commit `63e11b5696bb487138956a7ef43388bd148bca87`.

- `src/transport.rs` — **A1**: `ReserveCache` khoá `(pair, **quote**, block)`
  (trước: `(pair, block)` — nguyên nhân gốc, xem ô 5). **A4**: `MinedTxIndex`
  (hash tx 3 block gần nhất) + `SelfNonceCache` + `GasOracle::cached_price_any_block`
  (đọc CHỈ cache). **A5**: `is_unsupported_method_error` nhận thêm
  `-32602 archive/personal token`.
- `src/pipeline.rs` — **A2**: `sanity_check` (thuần) + `PipelineSkip::SanityReject`
  + gate ở CẢ 2 nhánh quote. **A3**: `PipelineSkip::CompetitorVictim`,
  `TxLogMeta.victim_in_competitor_cluster`. Fixture mới
  `fixture_reserves_sanity_ok()`/`VICTIM_1_BNB_WEI`, `usdt_deep_reserves()`
  20.000 → 60.000 USDT.
- `src/competitor.rs` (**MỚI**, ~190 dòng) — **A3**: `ClusterIndex` (3 seed
  tĩnh + ví burner động theo block), `transfer_topic0`, `address_from_topic`.
- `src/shadow.rs` — **A4**: `pre_sign_revet_fast` (THUẦN, 0 RPC, 0 fork) thay
  `pre_sign_revet` cũ; `PreSignRevetResult` đổi hẳn nội dung (5 lý do abort cụ
  thể); `build_and_log_shadow_bundle` nhận thêm `presign_ms`.
- `src/main.rs` — `bg_provider`/`bg_pool_health_check`/`bg_provider_or_hot`
  (**A5**), `mined_and_nonce_prefetch_task` (**A4**),
  `subscribe_competitor_funding` (**A3**), `spawn_shadow_sign_task` VIẾT LẠI
  (0 RPC, bấm giờ từng bước), `spawn_shadow_bundle_sim` (**A7**),
  `pairs_vet_task` chu kỳ 300 s cho pool nóng, gate `competitor_victim`,
  `candidate_seen`.
- `src/web.rs` — **A6**: `buckets_front_in_bnb`, `capital_for_80pct_profit`,
  `competitor`, `top_pools[].competitor_touched`, `top_pools_by_net`,
  `rate_inverted_rejected`/`rate_unavailable`; `/api/shadow` đếm abort theo
  TỪNG lý do + `sign_rate_pct`; 3 field `AppStateInner` mới + `candidate_seen`;
  funnel `sanity_reject`/`competitor_victim`.
- `src/relay.rs` — **BỔ SUNG**: `BLOCKRAZOR_BUILDER_URL_VIRGINIA`/`_GLOBAL`,
  `BLOCKRAZOR_BUILDER_EOA`, `CLUB48_BUILDER_EOA`, `CLUB48_GAS_FEE_WEIGHT`,
  `BLOCKRAZOR_MIN_GAS_PRICE_WEI`, `builder_eoa_for`,
  `build_blockrazor_builder_send_bundle_request` (auth, `noMerge`/`positionFirst`).
- `src/config.rs` — `bribe_mode` `"coinbase"` → `"builder_transfer"` (giá trị
  cũ = FAIL LOAD có thông báo), 2 field mới `blockrazor_builder_eoa`/
  `club48_builder_eoa` (validate address), `allow_competitor_victims`,
  `is_address_20_bytes`.
- `src/venues.rs` — `SKIP_REASONS` thêm `sanity_reject`, `competitor_victim`.
- `src/lib.rs` — `pub mod competitor;`.
- `src/bin/competitor_recon.rs` — dùng `BSC_HTTP_BG` (A5).
- `config.toml` — `bribe_mode="builder_transfer"`, 2 ví builder,
  `allow_competitor_victims=false`.
- `web/index.html` + `web/app.js` — khối "Vốn cần"/"Top pool (cụm đối thủ
  chạm)"/"Sức khoẻ tỉ giá"/"Shadow mode" (nợ dashboard từ BAOCAO40/41).
- `scripts/paper_run.sh` — in abort theo từng lý do, `presign.ms` p50/p95,
  `bundle.shadow_econ`, `shadow.sim`, `rpc.bg_pool`, `competitor.*`,
  `sanity_reject`/`competitor_victim`, `pair.vet_cycle`.
- `.env.example` — `BSC_HTTP_BG`, `BLOCKRAZOR_AUTH`.
- `DEX_REGISTRY.md` — bảng **relay → ví EOA builder** (2 ví, `source_url`,
  ngày, `eth_getCode`/nonce/balance thật) + bảng endpoint + quy tắc bribe.
- `docs/CONTRACT_DESIGN.md` (**MỚI**, 366 dòng) — PHẦN B.
- `docs/STATE.md`, `docs/TASKS.md` — cập nhật đầy đủ.
- `baocao/evidence/shadow_30min_run3_presign_fast.txt` (đã lược `raw_hex`),
  `baocao/evidence/a8_two_usdt_recipients.txt`.

**KHÔNG đụng**: `.env` (chỉ ĐỌC qua `std::env::var`/script), `PRIVATE_KEY`,
`pairs.txt`, VPS, `CLAUDE.md`. `dry_run=true`/`allow_live=false`/
`bot_armed=false` giữ nguyên. Không viết/deploy contract. Không gửi bundle.

## 4. LỆNH CHẠY

```bash
cargo build --release
cargo test --release
sha256sum target/release/bsc_sandwich
scripts/paper_run.sh --minutes 30 --port 18932 --live-mode shadow   # A7
# A8 + verify 2 vi builder: python3 goi RPC that (eth_getTransactionReceipt/
# eth_getBlockByNumber/eth_getLogs/eth_getCode/eth_call), CHI DOC
git log -1 --format="%H %ci" ; git status --short
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`).
Binary sha256 **lúc chạy paper run 30 phút** (A7):
`39f5946fdd0da8260ffce36eb74d540587d2da7355f0bf4cbd228bbd1a62c776`
(bản đó chứa A1–A6; các sửa sau khi chạy: đổi tên field `bribe_bnb` →
`bribe_native`, tách `rate_rejected`, thêm `top_pools_by_net`, phần relay/
config của 2 lần BỔ SUNG GIỮA PHIÊN).
Binary sha256 **cuối phiên** (HEAD `63e11b5`):
`bc9ecd8fa36e2592faf59d7cd9c5e1f1306c1d301018f9985443ac1058f07c4a`

### `cargo test --release`

```
test result: ok. 378 passed; 0 failed; 14 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
393 passed (378 lib + 15 main), tăng từ 373 (BAOCAO41) = **+20 test**. 0 failed.

### A1 — NGUYÊN NHÂN GỐC (không phải lỗi ở `/api/econ`)

Quét `logs/bot.jsonl` TRƯỚC sửa:

```
usdt rows: 9081   co du 2 field: 5070
ty gia amount_in_bnb_equiv/amount_in:  min 0.0   p50 0.0013906   max 720.78
dong ty gia > 1.0 (DAO CHIEU): 68
  52/68 tren pool 0x16b9a828… (WBNB/USDT), token = 0xbb4cdb9c… (WBNB)
vi du: amount_in=5 USDT -> amount_in_bnb_equiv=3590.97 "BNB"  (nhan 718 thay vi chia)
```

`720.78` = **đúng nghịch đảo** tỉ giá thật (~720 USDT/BNB, suy từ chính các
dòng USDT khác). Nguyên nhân: `ReserveCache` khoá `(pair, block)` thiếu
`quote`; pool WBNB/USDT vừa là candidate (quote=USDT) vừa là pool quy đổi
(quote=WBNB) → cache trả entry chiều ngược. Cùng lỗi làm `gas_cost_usdt_wei`
bị CHIA cho 720.

**Test fixture chính dòng Chủ chỉ ra** (`0x9a248ea3…f47922`,
`profit_net_wei=219550516598821986047`):

```
test web::tests::compute_econ_real_vps_usdt_row_converts_to_about_0_3_bnb ... ok
test web::tests::compute_econ_inverted_usdt_rate_row_is_rejected_not_astronomical ... ok
test transport::tests::reserve_cache_same_pair_two_quotes_do_not_collide ... ok
```

Sau sửa, chạy 30 phút: **0 dòng ty giá đảo** (kiểm lại bằng script trên chính
log lần chạy này).

### A2 — `sanity_reject`

Đối chiếu ngưỡng với dữ liệu THẬT trước khi chốt:

```
sim.result rows checked: 106   pass: 106   reject: 0
front lon nhat  = 6.76% reserve   (nguong 10%)
profit lon nhat = 0.302% reserve  (nguong 2%)
victim lon nhat = 2.71% reserve   (nguong 100%)
```
30 phút chạy thật: `sanity_reject = 0` — cổng không cắt cơ hội thật nào.

```
test pipeline::tests::sanity_check_three_limits_each_at_its_boundary ... ok
test pipeline::tests::decide_paper_v2_shallow_pool_front_over_10pct_reserve_is_sanity_reject ... ok
test pipeline::tests::decide_paper_quote_usdt_shallow_pool_is_sanity_reject ... ok
test pipeline::tests::sanity_check_rejects_victim_amount_bigger_than_whole_pool ... ok
pool 20 WBNB: front_in=1500000000000000000 wei (7.50% reserve) profit=129713... wei (0.6486% reserve) -> Simulated
```

### A3 — cụm đối thủ (30 phút thật)

```
competitor.subscribed  seeds=[0xB406…d07eF, 0xa739Dfab…69758c, 0x8180aD6A…6c54]
competitor.funded            = 623 lan / 30 phut
victim_in_competitor_cluster=true = 33 dong
skip competitor_victim       = 7     (allow_competitor_victims=false + live_mode=shadow)
test competitor::tests::funded_wallet_from_real_block_122076185_is_in_cluster ... ok  (victim 0xaaBae02D… dong A1)
```

### A4 — pre-sign KHÔNG fork: `presign.ms`

```
---- presign.ms p50/p95 (A4, muc tieu p95 < 20 ms) ----
n=12 p50=0.009 ms p95=0.011 ms max=0.027 ms
presign_ms 1 bundle that: {"vet":0.000982,"reserve":0.000753,"mined_index":0.001892,
                           "nonce":0.000073,"total_before_sign":0.007696}
presign_total_ms (ke ca KY 2 chan): 0.236 – 0.371 ms
```
**p95 = 0.011 ms so mục tiêu < 20 ms** (BAOCAO41: fork đồng bộ, không kịp).

### A7 — shadow 30 phút (WSL, port 18932, `--live-mode shadow`)

```
shadow.signer_loaded  self_address=0x961e5861a853cfb7a46863c27a66bd4e6c71a8f7
dem bundle.shadow = 9
tx.abort theo tung ly do:
  vet_stale                3
  reserve_stale            0
  mined_index_cold         0
  victim_already_mined     0
  nonce_not_prefetched     0
  sign_failed_front/back   0
```
**9 ký được / 12 lần thử = 75% ký kịp** (BAOCAO41: **0/36**). 3 abort
`vet_stale` đều rơi vào ~15 phút đầu khi vet nền chưa phủ hết 126 pool.

`latency.decision_vs_mined` 10/10 dòng đều **âm** (`-1`, một dòng `-2`) — bot
quyết định TRƯỚC khi victim lên block.

`bundle.shadow_econ` (9 dòng, tất cả quote USDT):
```
profit_net 71.05 USDT  bribe 28.42  net_after_bribe 42.63   cluster=false
profit_net 64.94 USDT  bribe 25.98  net_after_bribe 38.96   cluster=false
profit_net 23.78 USDT  bribe  9.51  net_after_bribe 14.27   cluster=false
profit_net  1.58 USDT  bribe  0.63  net_after_bribe  0.95   cluster=false
profit_net 54.61 USDT  bribe 21.85  net_after_bribe 32.77   cluster=false
profit_net 34.46 USDT  bribe 13.78  net_after_bribe 20.68   cluster=false
profit_net  7.31 USDT  bribe  2.92  net_after_bribe  4.38   cluster=false
profit_net  9.85 USDT  bribe  3.94  net_after_bribe  5.91   cluster=false
profit_net  2.27 USDT  bribe  0.91  net_after_bribe  1.36   cluster=false
```
(field JSON tên `bribe_bnb` trong log lần chạy này là **SAI TÊN** — giá trị là
USDT, đúng quote của pool; đã sửa thành `bribe_native`+`bribe_unit` NGAY SAU
lần chạy, có trong commit.)

`shadow.sim` (mô phỏng bundle 3 chân bằng revm nền): **9/9 dòng
`skipped:"usdt_not_supported_by_simulate_sandwich"`** — `simulate_sandwich`
dựng chân front bằng `swapExactETHForTokens*` (native BNB) nên KHÔNG dùng được
cho quote USDT. **KHÔNG bịa số** cho nhánh chưa hỗ trợ; ghi CÒN NỢ.

### A5 — RPC nền + latency

```
{"event":"rpc.bg_pool","bg_url_count":3,"hot_url_count":5,"separated":true,
 "source":"3 URL cuoi cua BSC_HTTP (mac dinh)",
 "bg_urls":["https://bsc-dataseed1.defibit.io/***","https://bsc-rpc.publicnode.com/***","https://bsc.rpc.blxrbdn.com/***"]}
```
p95 `seen_to_decision_ms` = **344.56 ms** (30 phút). So sánh THẬT:
BAOCAO40 (WSL, chưa có shadow) **321.25** → BAOCAO41 (thêm shadow, chung pool)
**355.69** → phiên này (tách pool nền) **344.56**. Tức **có cải thiện 3.1% so
BAOCAO41 nhưng VẪN CAO HƠN 7.3% so mốc BAOCAO40** — **KHÔNG đạt** mục tiêu
"p95 ≤ 321 ms" của lệnh. Ghi CÒN NỢ, không tô hồng.

**Bug thật phát hiện khi chạy lần 1** (đã sửa, chạy lại): `bsc-rpc.publicnode.com`
trả `-32602 "Archive requests require a personal token"` cho mọi lần revm đọc
storage → 126 dòng `pair.vet_error`, 0 pool được vet, đường ký abort
`vet_stale` 100%. `is_unsupported_method_error` không nhận mã này nên không
đổi URL. Đã thêm pattern + test.

### A6 — `/api/econ` sau 30 phút

```
candidate=26674 net_pos=12 best_net_bnb=0.099343 p50_ms=0.01 p95_ms=344.56 stale_pct=0.00 decode_fail_smartrouter=570
buckets_front_in_bnb : [('<0.01',0),('0.01-0.05',0),('0.05-0.2',0),('0.2-1',0),('>=1',12 net_pos=12, sum 0.5499 BNB)]
capital_for_80pct_profit.usdt = {capital_needed_native: 2999.995 USDT, taken_for_80pct: 8/12,
                                 captured 339.50 / total 393.38 USDT}
competitor = {candidate: 33 (0.124% tong), simulated: 0, pools_touched: 3}
bribe = {samples: 12, sum_bribe_bnb: 0.21996}
```
Toàn bộ 12 cơ hội có lãi đều cần **đúng trần vốn 3000 USDT** (`max_front_usdt`)
— tức trần vốn, không phải thị trường, đang là thứ quyết định quy mô.

### Bảng pool có lãi (tính từ log lần chạy này) — dùng cho go/no-go #2

```
POOL CO LAI (net_pos>0) trong 30 phut:
  0xdfe23efbdb03ea985c2423eb916fca8cae1d02ef quote=usdt candidate=22 net_pos=7  cluster_victim=7  compete_hit=4
  0xcec13213c390d51121f82ba2ecafb8e11e0af7a3 quote=usdt candidate=30 net_pos=5  cluster_victim=14 compete_hit=3
pool quote=WBNB co lai: 0
tong pool co candidate: 270
```

### A8 — 2 địa chỉ nhận USDT (tx `0x7574d418…cc8bc`, block 121919996)

Địa chỉ ĐẦY ĐỦ (chính 2 địa chỉ Chủ hỏi ở BAOCAO41 mà khi đó bị rút gọn):

```
0x3164240e…Ed16Fa7Ae = 0x3164240e9dd40c69cf6ffefde66fbb1ed16fa7ae   nhan 306.90 USDT
0x40cC5EfD…fBdb5E5A5 = 0x40cc5efd0cf20f304300104b342778dfbdb5e5a5   nhan  19.68 USDT
block 121919996 miner = 0x4d15d9bcd0c2f33e7510c0de8b42697ca558234a  -> KHONG phai 2 dia chi tren
getCode = 0 byte (EOA) ; nonce 430 / 342
Quet Transfer(USDT) [121916996..121922996]:
  0x3164240e…: NHAN 2 lan/326.34 USDT tu DUNG 1 dia chi (0xB406 SEED)
               GUI  2 lan/326.34 USDT toi 0xdfe23efb… va 0x7fd71204…
  0x40cc5efd…: NHAN 1 lan/19.68 tu 0xB406 ; GUI 1 lan/19.68 toi 0x7fd71204…
  balanceOf(USDT) hien tai: ca 2 = 0.0000
2 dia chi nhan tien: eth_call xac nhan LA POOL V2 THAT
  0xdfe23efb…: token0=0x01fbed06…(BNC) token1=USDT factory=0xcA143Ce3… (V2 Factory DA PIN)
  0x7fd71204…: token0=USDT token1=0xb391e9be… factory=0xcA143Ce3…
```

**KẾT LUẬN A8**: 2 địa chỉ **KHÔNG PHẢI ví fee builder/validator, cũng KHÔNG
PHẢI ví chia lãi** — chúng là **ví "burner" dùng-một-lần** của chính cụm đối
thủ (vào = ra đúng bằng nhau, chuyển THẲNG vào pair rồi `pair.swap()`, xong
số dư về 0), y hệt 3 ví trong tx `0x4916caa0f1…` của BAOCAO41. Vì vậy câu hỏi
phụ "tỉ lệ fee/lãi trên 100 tx → mốc `bribe_pct_of_profit`" **KHÔNG áp dụng**
(không có khoản fee nào để đo). Muốn đo bribe THẬT của cụm này cần
`debug_traceTransaction` — RPC công khai đang dùng KHÔNG hỗ trợ (xác nhận
BAOCAO41) → ghi **MISSING**, không suy diễn số.

Tình cờ đây là **xác nhận độc lập cho thiết kế B2**: đối thủ chuyển token
thẳng vào pair (không qua Router) — đúng mẫu tiết kiệm gas mà
`docs/CONTRACT_DESIGN.md` đề xuất.

### `/api/skips` sau 30 phút

```
{"below_min":0,"competitor_victim":7,"deadline":0,"decode_fail":10159,"gas_cap":0,
 "honeypot_or_tax":0,"hooks_unread":0,"no_pool":82,"nonce_future":0,"nonce_stale":0,
 "not_in_list":2889,"not_pancake_router":0,"not_quote_pair":687,"not_wbnb_pair":0,
 "rpc_error":15,"sanity_reject":0,"sell_direction":9737,"sim_error":0,"thin_liq":0,
 "unprofitable":2735,"venue_unpinned":180,"victim_would_revert":170}
```

### `git status --short` SAU commit + `git log -1`

```
(rỗng — working tree clean)
63e11b5696bb487138956a7ef43388bd148bca87 2026-09-16 11:25:12 +0700
```

## 6. CHAIN — `0x38`

Mọi kết nối xác nhận `chain_id=56`. Verify THẬT phiên này (WSL,
`https://bsc-dataseed1.bnbchain.org`, `eth_chainId` trả `0x38` trước khi gọi):

```
BlockRazor builder EOA 0x1266C6bE60392A8Ff346E8d5ECCd3E69dD9c5F20
   eth_getCode = 0 byte (EOA - dung nhu docs mo ta) nonce = 40.158.171 balance = 0.0000 BNB
48 Club Builder Control EOA 0x4848489f0b2BEdd788c696e2D79b6b69D7484848
   eth_getCode = 0 byte (EOA)                      nonce = 62.749.823 balance = 62.6169 BNB
pool 0xdfe23efbdb03ea985c2423eb916fca8cae1d02ef  code=14981 byte  factory=0xcA143Ce3… (V2 DA PIN)
pool 0x7fd71204a755f128d788ee1cd89b9cee8b8812e8  code=14981 byte  factory=0xcA143Ce3…
receipt tx 0x7574d418…cc8bc block 121919996 status=0x1 gasUsed=94089
```

**Lưu ý cách đọc `getCode = 0`**: luật "pin = `getCode > 0`" của CLAUDE.md áp
cho **contract**. 2 ví builder là **EOA**, `getCode = 0` là ĐÚNG KỲ VỌNG —
bằng chứng thay thế là nonce 40/62 triệu + balance thật.

## 7. REGISTRY

`DEX_REGISTRY.md` **thêm mục mới**: bảng **relay → ví EOA nhận bribe** (2 ví,
`source_url`, ngày đọc docs 2026-09-16, `eth_getCode`/nonce/balance thật) +
bảng 4 endpoint (2 builder BlockRazor + fallback + 48 Club) + quy tắc bribe
từng relay. Pin V2/V3/V4/WBNB/USDT **không đổi**.

## 8. KHÔNG LÀM

- Không viết Solidity, không deploy contract (đúng lệnh PHẦN B).
- Không gửi bundle/tx thật (`relay.rs` vẫn KHÔNG network — test
  `no_http_network_calls_anywhere_in_relay_rs` + `no_send_raw_transaction_call_anywhere_in_src`
  vẫn pass).
- Không bật live/`bot_armed`/`dry_run=false`/`live_mode="live"`.
- Không đụng VPS (paper 24h port 18910 vẫn chạy nguyên).
- Không đụng `.env`, `PRIVATE_KEY`, `pairs.txt`, `CLAUDE.md`.
- Không implement leg chuyển bribe tới ví builder (cần contract, cụm 6).
- Không hạ ngưỡng kinh tế trong `config.toml` ship.
- Không kết luận chiến lược thay Chủ — go/no-go #2 KHÔNG ĐẠT thì dừng ở đó.

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **GO/NO-GO cho contract = NO-GO** (`docs/CONTRACT_DESIGN.md` B7): điều kiện
  1 ĐẠT (75% ≥ 50%), điều kiện 2 **KHÔNG ĐẠT** — 30 phút không có **pool quote
  WBNB nào có lãi**, cả 2 pool có lãi đều là USDT và đều bị cụm đối thủ chạm.
  Cần chạy ≥ 4–24 giờ để biết đây là đặc điểm thật của `pairs.txt` hay chỉ do
  cửa sổ ngắn. **Quyết định tiếp theo là của Chủ** (đổi pool WBNB / chấp nhận
  đối đầu trên pool USDT / đổi hướng chiến lược).
- **p95 `seen_to_decision` 344.56 ms** — tốt hơn BAOCAO41 (355.69) nhưng vẫn
  CAO HƠN mốc BAOCAO40 (321.25). Mục tiêu lệnh "≤ 321 ms" **chưa đạt**.
- **`shadow.sim` chưa hỗ trợ quote USDT** → 9/9 bundle không có `profit_sim`
  để đối chiếu. Cần thêm biến thể token→token trong `sim_evm.rs`.
- **`vet_stale` đầu mỗi lần chạy**: có thể nạp lại `state/pairs_vetted.json`
  lúc boot để cổng (a) ấm ngay — chưa làm.
- **`pair.vet_error "missing trie node"`** (169 lần/30 phút) trên node public
  không đủ state → một phần pool không bao giờ vet được ⇒ không bao giờ ký
  được cho pool đó. Cần node archive riêng (`BSC_HTTP_SIM` trả phí).
- **`bribe_mode="builder_transfer"` mới TÍNH + LOG**, chưa có leg chuyển BNB
  tới ví builder (cần contract, B2/B3).
- **Tầng gửi relay + tra trạng thái bundle 48 Club → `RiskGuard::record_result`**
  chưa tồn tại.
- **`BLOCKRAZOR_AUTH` chưa có trong `.env`** — Chủ tự điền; thiếu thì đường
  builder BlockRazor bị TẮT (đã code đúng: `None` + log, không panic), bot
  dùng đường 2 `eth_sendMevBundle`.
- **48 Club `48spSign`** bỏ qua (chưa là member) — ghi MISSING.
- **2 reason mới `sanity_reject`/`competitor_victim` chưa có trong bảng skip
  của CLAUDE.md** — phiên này KHÔNG được sửa file đó, cần Chủ cập nhật.
- **Bribe THẬT của cụm đối thủ: MISSING** — cần `debug_traceTransaction`
  (RPC công khai không hỗ trợ).

Commit: `63e11b5696bb487138956a7ef43388bd148bca87`
