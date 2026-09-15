# BAOCAO41 — cụm `competitor-recon-and-strategy`

## 1. LÁT
`competitor-recon-and-strategy` — F1 trinh sát đối thủ MEV thật (RPC thật),
bribe model mô phỏng (F-02), SỬA bug Critical F-01 (bundle relay thiếu
victim leg), raw tx reconstruction, shadow mode ký thật (KHÔNG gửi). Bao
gồm **SỬA GIỮA PHIÊN** (mục 1) sau khi Chủ chỉ ra sai sót phương pháp bằng
bằng chứng on-chain thật — đã verify lại toàn bộ và viết lại kết luận.

## 2. LỆNH NHẬN
Khối lệnh Grok `competitor-recon-and-strategy`, 6 mục (trinh sát đối thủ,
bribe model, bundle encode, shadow mode, docs/RUN.md, đo lại latency). Máy:
WSL (code + shadow), KHÔNG đụng VPS. HEAD bắt đầu: `5b17a83`. Không subagent
ghi file (luật #4) — toàn bộ code/RPC call/phân tích phiên này do phiên
chính tự làm trực tiếp.

Giữa phiên, Chủ gửi 2 lần sửa/thêm yêu cầu:
1. Hỏi thêm 3 địa chỉ (`0x8180aD6A…23FcE6c54` selector `0xaacb5f51`,
   `0x3164240e…Ed16Fa7Ae`, `0x40cC5EfD…fBdb5E5A5`) — 2 địa chỉ sau bị rút
   gọn, KHÔNG đủ ký tự để tra RPC, chưa làm được (xem ô 10). Địa chỉ đầu
   TÌM RA ĐƯỢC đầy đủ qua chính investigate mục 1e (xem dưới).
2. **SỬA GIỮA PHIÊN (mục 1)**: chỉ ra phương pháp đo "dormant" của
   `0xa739Dfab...` SAI (executor không phát Swap event), yêu cầu đo lại
   bằng `tx.to` + đối chứng 2 block thật (`122070562`, `122076185`), yêu
   cầu làm cụm theo dòng tiền cùng block (mục 1e), chạy lại bảng 38/65 pool
   cả CŨ/MỚI, KHÔNG kết luận backrun-only cho tới khi có bảng cụm.

## 3. FILE ĐỔI

Commit `7a836c14f06b899d2e0c4d2f3a35bff8e75298db`:

- `src/bin/competitor_recon.rs` (bin MỚI, ~630 dòng) — trinh sát RPC thật,
  CHỈ ĐỌC. Phần A (Swap-log sender/to), Phần A2 (**SỬA GIỮA PHIÊN**: cluster
  theo dòng tiền `Transfer` cùng block, `scan_transfer_from_role`), Phần B
  (126 pool `pairs.txt`, bảng đối thủ CŨ + MỚI). `get_logs_retry` (backoff
  429/-32005). `--env RECON_SKIP_PART_A` để debug nhanh.
- `src/relay.rs` — **SỬA F-01 (audit Critical)**: `build_48club_send_bundle_request`/
  `build_blockrazor_send_mev_bundle_request` nhận thêm `victim_raw_hex`,
  bundle giờ ĐÚNG 3 leg `[front, victim_raw, back]` (trước chỉ 2 leg
  `[front, back]` — thiếu victim, lỗ chắc chắn nếu từng nối live). Toàn bộ
  test cập nhật + test RPC thật `real_rpc_bundle_with_real_victim_raw_tx`.
- `src/transport.rs` — `fetch_raw_tx_verified` (ưu tiên
  `eth_getRawTransactionByHash`, fallback `eth_getTransactionByHash` +
  `Encodable2718`, verify `keccak256(raw)==hash`). Test RPC thật
  `real_rpc_reconstruct_raw_tx_type0_and_type2`.
- `src/pipeline.rs` — `compute_bribe_wei` (F-02, hàm thuần), gate
  `evaluate_candidate`/`evaluate_candidate_quote` trên `profit - bribe`
  thay vì `profit` thô. `TxLogMeta` thêm `bribe_wei`/`net_pos_after_bribe_wei`.
  Test mới: 3 test `compute_bribe_wei_*` + 1 test ĐẠT CẦN DÁN chứng minh gate
  thật sự chặn (`decide_paper_v2_pair_mode_bribe_eats_thin_margin_becomes_unprofitable`).
- `src/config.rs` — 5 field mới: `bribe_pct_of_profit`/`bribe_min_bnb`/
  `bribe_max_bnb`/`bribe_mode` (F-02), `live_mode` (mục 4, "off"|"shadow"|
  "live", ship "off"). Fail load rõ ràng nếu sai chuỗi (cùng khuôn
  `sim_engine`).
- `src/shadow.rs` (module MỚI, ~330 dòng) — shadow mode: `load_shadow_signer`/
  `self_address` (dùng `alloy_signer_local::PrivateKeySigner` — KHÁC
  `executor::load_signer` chỉ trả `B256`, hàm đó GIỮ NGUYÊN), `sign_leg` (ký
  EIP-1559 type 2 thật qua `EthereumWallet`+`TransactionBuilder`+
  `Encodable2718`), `pre_sign_revet` (đo lại victim-chưa-mined + reserve +
  tax `measure_tax_evm` NGAY TRƯỚC KHI KÝ), `build_and_log_shadow_bundle`
  (log `bundle.shadow`/`tx.abort`, KHÔNG relay simulate — xác nhận
  `eth_callBundle` KHÔNG tồn tại ở CẢ 2 relay, cURL thật `-32601`). 7 test
  đơn vị (ký thật với key giả lập hợp lệ, verify keccak/type-2-prefix).
- `src/main.rs` — load `shadow_wallet` lúc boot (chỉ khi `live_mode=
  "shadow"`), `spawn_shadow_sign_task` (task nền, hỗ trợ CẢ WBNB lẫn USDT
  sau khi mở rộng giữa phiên).
- `src/web.rs` — `AppStateInner.shadow_wallet`, `/api/shadow` (bundle ký/
  abort + ví redact), `/api/status` thêm `live_mode`/`shadow_armed`/
  `shadow_self_address`/`bribe_*`. `/api/econ` thêm khối `"bribe"`.
- `src/lib.rs` — `pub mod shadow;`.
- `Cargo.toml`/`Cargo.lock` — thêm `alloy-signer-local = "2.4.2"` (XÁC NHẬN
  THẬT nay đã có trên crates.io, khác BAOCAO15 lúc đó CHƯA có) + feature
  `signer-local` cho `alloy`.
- `scripts/paper_run.sh` — `--live-mode shadow` (mặc định `"off"`, không đổi
  hành vi cũ), in số `bundle.shadow`/`tx.abort` khi bật.
- `config.toml` — 5 field mới (mục trên), ship `bribe_pct_of_profit=40.0`,
  `bribe_min_bnb=0.0005`, `bribe_max_bnb=0.01`, `bribe_mode="coinbase"`,
  `live_mode="off"`.
- `.env.example` — cập nhật comment `PRIVATE_KEY` (dùng cho shadow mode).
- `docs/STATE.md`/`docs/TASKS.md`/`docs/DOC_MAP.md`/`docs/RUN.md` — cập nhật
  đầy đủ + mục "SỬA GIỮA PHIÊN" chi tiết + mục "Lên live" mới trong RUN.md.
- `baocao/evidence/*.txt` (5 file) — output RPC thật đầy đủ, dẫn nguồn ô 5.

**KHÔNG đụng**: `.env`, `PRIVATE_KEY` (chỉ ĐỌC qua `std::env::var`),
`pairs.txt`, VPS, cờ live (`allow_live`/`bot_armed`/`dry_run` giữ nguyên
`false`/`false`/`true`), không viết/deploy contract executor, không
`sendRaw`/broadcast (xác nhận bằng test quét toàn `src/`, xem ô 5).

## 4. LỆNH CHẠY

```bash
cargo build --release
cargo test --release
sha256sum target/release/bsc_sandwich target/release/competitor_recon
git log -1 --format="%H %ci"

set -a; . .env; set +a
./target/release/competitor_recon                    # Phần A+A2+B, 3 lần (debug rate-limit + sửa giữa phiên)
scripts/paper_run.sh --minutes 30 --port 1893x --live-mode shadow  # 2 lần (mở rộng USDT giữa phiên)
cargo test --release --lib real_rpc_reconstruct_raw_tx_type0_and_type2 -- --ignored --nocapture
cargo test --release --lib real_rpc_bundle_with_real_victim_raw_tx -- --ignored --nocapture

# Xác nhận bằng chứng Chủ đưa ra (curl thật, không qua binary)
curl eth_getBlockByNumber 122070562 / 122076185
curl eth_getTransactionReceipt (2 tx đối chứng)
curl eth_getLogs Transfer USDT from=0xB406 (tìm cluster)
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`). Binary sha256 (HEAD `7a836c1`,
không đổi từ lúc build tới lúc commit):
`bsc_sandwich` = `2d3062bb0f608ab0da8087912afd8cc941a017b3aba533eec402ca8ebb075edb`
`competitor_recon` = `ff45b7e1ccba885edf5ab616189a905cc540805441b26728e3453236cec5446d`

### `cargo test --release`

```
test result: ok. 358 passed; 0 failed; 14 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```
373 passed (358 lib + 15 main), tăng từ 362 (BAOCAO40) = **+11 test mới**
(4 bribe + 7 shadow, chưa tính relay/transport test cập nhật). 0 failed.

### `git status --short` SAU commit

```
(rỗng — working tree clean)
```
`git log -1 --format="%H %ci"`: `7a836c14f06b899d2e0c4d2f3a35bff8e75298db 2026-09-16 02:33:37 +0700`

### Mục 1 — Trinh sát đối thủ (3 lần chạy `competitor_recon`, WSL, RPC
`rpc-bsc.48.club`)

**Lần 1** (`baocao/evidence/competitor_recon_run1.txt`) — phát hiện BUG
THẬT: Phần A quét 400.000 block/địa chỉ (800 lần `eth_getLogs`) làm RPC trả
`429` liên tục cho Phần B ngay sau đó → Phần B "0 log" SAI (rate-limit,
không phải không có dữ liệu). Sửa `get_logs_retry` (backoff 500ms→4s) +
giảm `PART_A_BLOCK_BUDGET` 400k→150k.

**Lần 2** (`competitor_recon_run2.txt`, sau khi sửa) — Phần B THẬT: 108 pool
WBNB, 3000 block, 9359 log Swap, **1264 victim ≥0.05 BNB, 343/1264 (27.1%)**
có neighbor ±3. Contract `0xa739...` kết luận SAI "dormant" (xem SỬA GIỮA
PHIÊN dưới).

**SỬA GIỮA PHIÊN**: Chủ đưa 2 block đối chứng — verify THẬT:

```
Block 122070562, tx 0x3395dadaf2b4fd6ad9987a5fa709eaedca18f779f837ba8bb720708c73713abf:
  from=0xb406021e07b31e1f7850fcccd7076094f18d07ef
  to=0xa739dfab40ef6585f1174fce90ec96330669758c
  selector=0x5aab2274 gas=90188 gasPrice=0.05gwei nonce=350245

Block 122076185, tx 0xcb2018256ed6d047e97351b25c33e71d0b61b9c5367ffdc1aa5ecc893b3c571b (idx 6):
  logs: Transfer(USDT) 0xB406->0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7 amount=2757930000000000000000 (= 2757.93 USDT, KHOP DUNG so Chu dua)
        Approval(USDT) owner=0xB406 spender=0xa739
  tx ke tiep (idx 7, cung block) 0xaaBae02D...->Router(0x10ED43...), selector 0x38ed1739
  Swap THAT tren pool 0xcec13213c390d51121f82ba2ecafb8e11e0af7a3, token 0xe210c0583c1071714eded2d8beeab05ab5bb7777
  (CHINH LA token duy nhat tung Simulated trong shadow-run dau phien nay - doi chieu doc lap)
```

Cơ chế thật (tx `0x4916caa0f14719ec3cc6404e87984ff95d014cfe742afa7c63476d83d0d256fe`,
block 122082156): 1 lời gọi `0x5aab2274` cấp vốn CÙNG LÚC cho 3 địa chỉ khác
nhau (547.5/223.4/42.1 USDT), cả 3 swap NGAY 3 vị trí kế tiếp (5,6,7) cùng
pool — bot đa-ví dùng ví "burner" cấp vốn tức thời.

**Lần 3** (`competitor_recon_run3_cluster.txt`, sau khi sửa Part A2) —
quét `Transfer USDT from=0xB406` 50.000 block: **4080 lần**. Top nhận vốn:
`0x8180ad6a7c9f8f4864e9909480fba4123fce6c54` **2850 lần** (địa chỉ đầy đủ
Chủ hỏi — xếp thứ 2 chỉ 5 lần). Cluster 12 địa chỉ, chạm 4 pool
(`0xcec13213...`/`0xdfe23efbdb...`/`0x7fd71204a755...`/`0xf867ca539dbb...`
— toàn bộ quote USDT, ngoài phạm vi Phần B chỉ xét WBNB).

Bảng đối thủ CŨ vs MỚI (65 pool có victim, 108 pool WBNB, 3000 block
`[122080971..122083970]`, 2042 victim ≥0.05 BNB):

| Chỉ số | CŨ (đơn lẻ) | MỚI (cụm 12 địa chỉ) |
|---|---|---|
| opportunity_pools | 48/65 | 65/65 |
| pool cụm chạm tới (WBNB) | 0 | 0 (4 pool cụm đều quote USDT, ngoài phạm vi Phần B) |

Kiểm tay sâu (172 log Swap pool `0xcec13213...`, 18 lần funding gần hoạt
động Swap): 12/18 swap NGAY vị trí kế tiếp funding (dist=1, không chỗ trống
cho victim thứ 3); 6/18 swap muộn hơn (gap 29-77) nhưng KHÔNG có địa chỉ nào
khác swap trên đúng pool ở giữa. **0/18 case có dấu hiệu victim bị kẹp giữa
2 chân của cụm này.** KHÔNG kết luận backrun-only (đúng yêu cầu Chủ) — kết
luận đầy đủ xem `docs/STATE.md` mục "SỬA GIỮA PHIÊN".

Selector `0xaacb5f51` Chủ hỏi: KHÔNG có trong 4byte.directory
(`{"count":0,...}`) lẫn openchain.xyz (`{"function":{"0xaacb5f51":null}}`) —
2 nguồn độc lập, không tìm được tên hàm.

### Mục 3 — Raw tx reconstruction + F-01 (RPC thật)

```
test transport::tests::real_rpc_reconstruct_raw_tx_type0_and_type2 ... ok
dung RPC: https://bsc.blockrazor.xyz/***
type0/Legacy: hash=0xa9cd0954... raw_len=872 nguon=eth_getRawTransactionByHash
type2/EIP-1559: hash=0xbcf25f1a... raw_len=1911 nguon=eth_getRawTransactionByHash

test relay::tests::real_rpc_bundle_with_real_victim_raw_tx ... ok
victim_raw THAT: hash=0x3a8fa10d... nguon=eth_getRawTransactionByHash len=457
48club + blockrazor bundle 3-leg voi victim THAT: OK
```

### Mục 4 — Shadow mode (2 lần chạy 30 phút WSL, `--live-mode shadow`)

**Lần 1** (`shadow_30min_run1.txt`) — chỉ hỗ trợ WBNB, 0/10 sim.result là
WBNB (toàn bộ USDT) → 0 bundle.shadow, không đủ dữ liệu → mở rộng shadow
sang USDT giữa phiên (calldata.rs đã có sẵn `encode_*_usdt`).

**Lần 2** (`shadow_30min_run2.txt`, sau khi mở rộng USDT):
```
{"event":"shadow.signer_loaded","self_address":"0x961e5861a853cfb7a46863c27a66bd4e6c71a8f7", ...}
dem bundle.shadow: 0
dem tx.abort reason=pre_sign_revet_failed: 36
{"event":"tx.abort","honeypot":false,"reason":"pre_sign_revet_failed","reserve_still_ok":true,
 "roundtrip_tax_bps":0,"tax_ok":true,"token":"0xe210c0583c1071714eded2d8beeab05ab5bb7777",
 "victim_pending_confirmed":false, ...}
(35 dòng tx.abort khác cùng khuôn — 34/36 chỉ fail victim_pending_confirmed,
2/36 fail thêm tax_ok=false)
candidate=19607 net_pos=36 best_net_bnb=0.271203 p50_ms=0.01 p95_ms=355.69 stale_pct=0.00
```

**Kết quả THẬT, không bịa**: **0/36 bundle ký thành công** — 100% bị chặn ở
`pre_sign_revet` vì `victim_pending_confirmed=false` (victim ĐÃ lên block
trước khi re-vet chạy xong). Nguyên nhân nhiều khả năng: `measure_tax_evm`
(fork `revm`+`AlloyDB`, cold-fetch state qua RPC) chậm hơn thời gian còn lại
tới khi victim lên block (~3s BSC) — re-vet AN TOÀN ĐÚNG THIẾT KẾ (không ký
nhầm khi victim đã mined) nhưng **cho thấy kiến trúc hiện tại (fork EVM
đồng bộ ngay trước ký) không kịp thời gian thực với RPC công khai hiện có**.
2 relay (`puissant-builder.48.club`, `bsc.blockrazor.xyz`) xác nhận KHÔNG có
`eth_callBundle` (`-32601` cả 2, cURL thật) — không relay simulate được.

### Mục 6 — Latency (đo lại sau khi thêm shadow mode)

p95 `seen_to_decision_ms` = **355.69ms** (30 phút WSL, run2) so BAOCAO40 WSL
**321.25ms** — **TĂNG ~10.7%** (KHÔNG đạt "không được tăng"). Vẫn dưới mục
tiêu <500ms. Nguyên nhân nhiều khả năng: shadow-mode task nền (nonce fetch +
gas oracle + fork tax re-vet) tranh chấp cùng pool RPC với đường nóng —
CHƯA tách riêng pool RPC cho shadow mode. Ghi CÒN NỢ.

## 6. CHAIN — `0x38`
Mọi kết nối xác nhận `chain_id=56` (`connect_and_verify`/`get_chain_id`).
`eth_getCode` xác nhận contract `0xa739Dfab...` CÓ bytecode thật (2601 byte).
2 block đối chứng (`122070562`/`122076185`) + nhiều tx/receipt thật đã
verify trong phiên (xem ô 5).

## 7. REGISTRY
Không đổi pin. `DEX_REGISTRY.md` giữ nguyên.

## 8. KHÔNG LÀM
- Không viết/deploy contract executor.
- Không gửi bundle thật (relay.rs vẫn KHÔNG network — test xác nhận).
- Không bật live/`bot_armed`/`dry_run=false`.
- Không đụng VPS/port 18910.
- Không đổi `pairs.txt`.
- Không hạ ngưỡng kinh tế trong `config.toml` ship.
- KHÔNG kết luận "backrun-only" (đúng yêu cầu Chủ) — chỉ báo cáo bảng cụm,
  để Chủ/Grok tự quyết định.
- Không implement `bribe_mode="coinbase"` leg chuyển BNB thật (chỉ tính +
  log, chưa có cơ chế gửi).

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU
- **2 địa chỉ Chủ hỏi vẫn rút gọn** (`0x3164240e…Ed16Fa7Ae`,
  `0x40cC5EfD…fBdb5E5A5`) — cần Chủ dán đầy đủ 40 ký tự hex.
- **Mở rộng Phần B sang 18 pool USDT `pairs.txt`** — cụm `0xB406` hoạt động
  chủ yếu trên pool USDT (`0xcec13213...`/`0xdfe23efbdb...`), Phần B hiện
  CHỈ xét WBNB nên "0/65 bị cụm phủ" KHÔNG có nghĩa "an toàn" — chỉ có nghĩa
  "ngoài phạm vi đã xét". Đây là việc quan trọng nhất còn lại trước khi có
  thể kết luận chiến lược chắc chắn.
- **Cluster set (mục 1e) mới có 12 địa chỉ (top-8 theo số lần nhận vốn)** —
  thực tế có RẤT NHIỀU ví "burner" chỉ nhận vốn 1 lần (như 3 ví trong tx
  `0x4916caa0f1...`) không lọt vào top 8 — `cluster_bracket%=0.0%` trong
  bảng MỚI vì vậy là CẬN DƯỚI, không phải số cuối cùng. Cách làm đúng hơn:
  cluster ĐỘNG theo từng block (bất kỳ tx nào nhận Transfer từ 0xB406 TRONG
  CHÍNH block đó), không dùng danh sách tĩnh — ngoài ngân sách thời gian
  phiên này.
- Shadow mode: 0/36 bundle ký thành công trong 30 phút — CHƯA có bằng chứng
  thật về `sim_profit relay vs bot`/`net_pos_after_bribe` thực tế (không có
  bundle nào để so sánh). Cần giải quyết độ trễ `measure_tax_evm` trước khi
  số liệu này có ý nghĩa.
- p95 latency TĂNG 321→356ms sau khi thêm shadow-mode task nền — chưa tách
  riêng RPC pool cho shadow, chưa xác nhận chắc chắn nguyên nhân duy nhất.
- `bribe_mode="coinbase"` chưa có cơ chế gửi BNB tới `block.coinbase` (chỉ
  tính + log).
- `relay.rs` (3 leg đã sửa đúng) vẫn CHƯA nối vào `main.rs`/live loop — cần
  `7.3`/cụm `strategy-exec` (signer LIVE thật, khác shadow) + HTTP client gửi
  relay thật.
- Selector `0xaacb5f51` vẫn chưa xác định được TÊN hàm cụ thể (không có
  trong 2 nguồn signature database công khai đã tra).
- Dashboard tĩnh (`web/app.js`/`index.html`) chưa vẽ khối shadow/bribe mới —
  API đã sẵn (`/api/shadow`, `/api/status` mở rộng), chỉ thiếu UI (cùng tình
  trạng `top_pools`/`compete` từ BAOCAO40).

Commit: `7a836c14f06b899d2e0c4d2f3a35bff8e75298db`
