# BAOCAO38 — cụm `real-economics-mode2` cụm B

## 1. LÁT
`real-economics-mode2` cụm B: sửa cổng tax cho token đã vet (fix bug
BAOCAO37), gas thật (F-03), `/api/econ`, F-27 validator, đo kinh tế 60 phút
trên pair-mode, chạy `real_rpc_*`, backfill `docs/TASKS.md` mục BAOCAO33/34.

## 2. LỆNH NHẬN
Khối lệnh Grok `real-economics-mode2` cụm B (mục 0-8 đầy đủ, xem lệnh gốc
trong lịch sử phiên). Máy: WSL. Git HEAD bắt đầu: `8cf5923f9a358156a1b45f70d9615f1d1a360d57`.
Cấm subagent/fork ghi file (luật #4) — toàn bộ code/docs trong BAOCAO này do
phiên chính viết trực tiếp; 1 fork nghiên cứu ban đầu được thử nhưng KHÔNG
thực hiện được việc gì (0 tool call, xem mục 8) nên bị bỏ, phần còn lại của
phiên tự đọc file trực tiếp.

## 3. FILE ĐỔI

Commit `2e32dc3d4e73514350d03038cfe9c8236708c489` — cụm chính:
- `src/pairbook.rs` — `PairBook::is_tax_ok`/`knows_pool`/`is_vet_failed`
  (mục 0, fix bug BAOCAO37) + 4 test mới.
- `src/pipeline.rs` — `PipelineSkip::GasCap`, `compute_gas_cost_wei`,
  `evaluate_candidate`/`evaluate_candidate_quote` nhận `gas_cost_wei`/
  `skip_tax_gate`/`gas_cost_bnb_wei`/`gas_cost_in_quote_wei`,
  `decide_paper_v2` nhánh pair route `vet_failed` vào `honeypot_or_tax` thay
  vì rơi xuống `not_in_list`, `TxLogMeta` +7 field (mục 2),
  `convert_gas_cost_bnb_to_usdt`, `log_outcome_v2` thêm field mới +
  `profit_gross_wei`/`profit_net_wei`. ~15 test mới/sửa.
- `src/transport.rs` — `GasOracle` (F-03) + 4 test (mock JSON-RPC theo
  method), test `nonce_future_then_ok_after_k_confirms_same_sender` (mục 5).
- `src/sim_evm.rs` — THÊM HÀM MỚI `measure_gas_units` (F-03, đo gas unit
  thật bằng revm) + 1 test `#[ignore]` — KHÔNG sửa logic sim_evm hiện có
  (giữ đúng CẤM "sim_evm logic" của lệnh; xem giải trình mục 8).
- `src/config.rs` — 3 field mới `gas_units_front`/`gas_units_back`/
  `gas_price_max_gwei` + `gas_price_max_wei()` + test.
- `src/venues.rs` — thêm `"gas_cap"` vào `SKIP_REASONS`.
- `src/web.rs` — `GET /api/econ` (mục 3, `compute_econ_from_rows` thuần +
  `boot_wall_clock` filter — xem commit `522df53`), `ValidateStats`/
  `ValidateGroupStats` tách isolated/non_isolated (F-27, mục 4),
  `FunnelCounters` +`gas_cap`. ~13 test mới.
- `src/main.rs` — wire `GasOracle`/gas units vào `handle_paper_tx` (cả 2
  nhánh WBNB/USDT), `gas_units_boot_task` (đo 1 lần lúc boot), `meta.detail
  = "vet_fail"`, `spawn_victim_validator` gọi `push(row, isolated, pct)`
  mới, `boot_wall_clock`.
- `config.toml` — 3 field mới + comment.
- `CLAUDE.md` — mục Config (+3 field, đổi ý nghĩa `front/back_max_gas_bnb_wei`),
  Skip (+`gas_cap`), Math (gas thật cho WBNB, bỏ luật "USDT không trừ gas").
- `docs/STATE.md` — cập nhật "TRẠNG THÁI HIỆN TẠI" + mục mới
  "real-economics-mode2 cụm B" (chi tiết đầy đủ mục 0-5).
- `docs/TASKS.md` — backfill hàng BAOCAO33/34, cập nhật hàng
  `real-economics-mode2`, mục "Nợ CÒN THẬT" (F-03 XONG cho v2, F-27 XONG,
  nonce_future note).
- `README.md`, `docs/RUN.md` — mục 3/5/6/7 cho field/API mới.

Commit `88ac3b1f96e1b7b0fa3533c676dfd7531af18125` — bổ sung sót:
- `web/index.html`, `web/app.js` — hiển thị `/api/econ` trên dashboard
  (section "Kinh tế").

Commit `522df53c60acf4bf4ff05dea455dbd69ee17c039` — fix phát hiện lúc verify
60 phút thật:
- `src/web.rs` — `compute_econ_from_rows` nhận thêm `since_ts`, lọc dòng
  `ts >= boot_wall_clock` (xem mục 5 OUTPUT THẬT để biết vì sao cần).
- `src/main.rs` — `AppStateInner.boot_wall_clock: chrono::DateTime<Utc>`.
- `docs/STATE.md` — ghi lại phát hiện + cách sửa.

(Commit `8ccdd392b405bfe5cb769a2d995987f0f4e540f6` "scripts: +x" nằm GIỮA 2
commit trên trong lịch sử git — do Chủ tự commit ngoài phiên này (tác giả
`0xcoldtrace`, chỉ đổi mode +x 6 file `scripts/*.sh`, không đổi nội dung),
không phải Code làm — ghi rõ để không ai hiểu nhầm là Code tự thêm.)

**KHÔNG đụng**: `.env`, `pairs.txt`, cờ live, `decoder.rs` (không sửa dòng
nào), `sim_evm.rs` logic hiện có (chỉ THÊM hàm `measure_gas_units` mới, độc
lập, không sửa `run_sandwich`/`decide_with_evm`/`measure_tax_on_fork`/...).

## 4. LỆNH CHẠY

```bash
cargo build --release
cargo test --release
sha256sum target/release/bsc_sandwich
git log -1 --format="%H %ci"
git status --short

scripts/paper_run.sh --minutes 60 --port 8799

cargo test --lib real_rpc_fetch_expected_nonce_for_wbnb_contract -- --ignored --nocapture
cargo test --lib real_rpc_v2_get_pair_wbnb_usdt -- --ignored --nocapture
cargo test --lib real_rpc_v2_get_pair_and_reserves_for_non_wbnb_quote_usdt -- --ignored --nocapture
cargo test --lib real_rpc_v3_quote_wbnb_to_usdt -- --ignored --nocapture
cargo test --lib real_rpc_measure_gas_units_on_cake -- --ignored --nocapture
cargo test --lib real_rpc_sim_evm_matches_sim_v2_when_zero_tax -- --ignored --nocapture
cargo test --lib real_rpc_victim_prediction_matches_onchain -- --ignored --nocapture
cargo test --lib real_rpc_ternary_search_evm_warm_cache_timing -- --ignored --nocapture
cargo test --lib real_rpc_sim_evm_block_cache_and_tax -- --ignored --nocapture   # (chạy 2 lần, xem ô 5)
cargo test --lib real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window -- --ignored --nocapture
cargo test --lib real_rpc_scan_finds_known_historical_cl_initialize_log -- --ignored --nocapture
cargo test --lib real_rpc_replay_real_sandwich_triplets -- --ignored --nocapture
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`). **Binary sha256 (sau commit cuối
`522df53`)**: `33f84ed709a658fb93d0bee2531ee39c5c718fbf74a565971bead0a9dec22bdf`.
**Git HEAD lúc chạy paper_run 60 phút** (commit `2e32dc3`, TRƯỚC fix
`boot_wall_clock`): `2e32dc3d4e73514350d03038cfe9c8236708c489`, sha256 lúc đó
`0b28104dbfa2ec18ecd77236d45f8df82562f6fd5f8626e1a30299775abfda6f`.

### `cargo build --release` + `cargo test --release`

```
Finished `release` profile [optimized] target(s) in 17.32s   (0 warning)
test result: ok. 308 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```
308 lib + 15 main = **323 passed** (tăng từ 295 đầu phiên, +28 test).

`git status --short` (SAU commit cuối, TRƯỚC khi thêm chính file BAOCAO này):
```
(rỗng)
```
`git log -1 --format="%H %ci"`:
```
522df53c60acf4bf4ff05dea455dbd69ee17c039 2026-09-15 18:30:01 +0700
```

### `scripts/paper_run.sh --minutes 60 --port 8799` (60 phút THẬT, WSL, `.env` thật)

```
== may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
BSC_WS host = bsc-rpc.publicnode.com,wss (KHONG in URL day du) -> khop publicnode, OK
== binary sha256 = 0b28104dbfa2ec18ecd77236d45f8df82562f6fd5f8626e1a30299775abfda6f ==
== git HEAD = 2e32dc3d4e73514350d03038cfe9c8236708c489 ==
== pairs.txt: tong=93 vetted=44 chua_vet=49 ==
```

30 dòng `funnel.minute` cuối (rút gọn, mẫu đầu/cuối):
```
{"decode_fail":377,...,"honeypot_or_tax":0,"gas_cap":0,"seen":12657,"simulated":0,"unprofitable":122,"venue_v2":157,"ts":"2026-09-15T10:48:46+00:00"}
...
{"decode_fail":379,...,"honeypot_or_tax":0,"gas_cap":0,"seen":17525,"simulated":0,"unprofitable":25,"venue_v2":81,"ts":"2026-09-15T11:20:25+00:00"}
```
**`honeypot_or_tax=0` XUYÊN SUỐT 60 phút** (mọi phút) — xác nhận fix bug mục
0 hoạt động đúng trên dữ liệu thật liên tục, không phải may mắn 1 lần.
`unprofitable` giảm dần theo thời gian (122 → 25) khi vet nền dần loại các
pool có candidate xấu — quan sát hợp lý, không giải thích thêm (không suy
diễn nguyên nhân xa hơn dữ liệu cho phép).

`/api/skips` (cuối 60 phút, TÍCH LUỸ toàn phiên):
```json
{"below_min":0,"decode_fail":24748,"gas_cap":0,"honeypot_or_tax":0,"hooks_unread":0,"no_pool":0,"nonce_future":0,"nonce_stale":0,"not_in_list":3117,"not_pancake_router":0,"not_quote_pair":0,"not_wbnb_pair":17262,"sell_direction":0,"sim_error":0,"thin_liq":0,"unprofitable":4504,"venue_unpinned":12,"victim_would_revert":0}
```

`/api/pairs` (44 pool, mẫu): hầu hết `buy_bps:0 sell_bps:0 honeypot:false
candidate:true` với `last_vet_sec_ago` 285-2988s — xác nhận `pairs_vet_task`
đã vet nền THÀNH CÔNG hầu hết 44 pool trong 60 phút (2 entry vẫn `null` lúc
lấy mẫu — BUSD `0xe9e7...D56` và SFP `0xD41F...dfb` — có thể chưa tới lượt
trong vòng vet đang chạy dở, không phải lỗi). `state/pairs_vetted.json`
không tồn tại lúc lấy mẫu (job ghi file SAU khi xong CẢ vòng — vẫn đang xử
lý dở khi tôi kiểm tra giữa chừng).

**Phát hiện timing thật (KHÔNG phải bug, ghi rõ)**: `gas_units_boot_task`
(chờ tối đa 60s) ra `gas.units_measure_giveup` (giữ fallback config
160000/140000) vì `pair.reload` lần đầu hoàn tất SAU đó ~700ms
(`10:17:11.158` vs `10:17:11.865`) — race hiếm giữa 2 task nền lúc boot.
Không ảnh hưởng tính đúng đắn (fallback vẫn hợp lệ), chỉ mất cơ hội dùng số
đo thật cho riêng lần chạy 60 phút này. Đo thật gas unit CAKE qua test
`real_rpc_measure_gas_units_on_cake` riêng (ô dưới): front=121916
back=105539 — thấp hơn fallback (160000/140000), xác nhận fallback không hề
đánh giá THẤP gas thật (an toàn theo đúng hướng).

**Phát hiện + sửa THẬT giữa phiên (`/api/econ` đọc lẫn lịch sử)**:
`GET /api/econ` bản đầu đọc TOÀN BỘ `logs/bot.jsonl` (file dùng chung, không
xoá giữa các lần chạy) nên `candidate=87225` — SAI, lẫn số liệu các phiên
TRƯỚC (tổng file 197140 dòng, nhưng riêng 60 phút này chỉ có ~115050 dòng
mới, tính tay ra `candidate=49787`, khớp gần đúng `/api/skips` — 24748 vs
24879 decode_fail, lệch nhỏ do biên thời điểm lấy mẫu). Đã SỬA
(`boot_wall_clock`, commit `522df53`) — chưa chạy lại 60 phút để xác nhận số
đã đúng qua chính `/api/econ` (tốn thêm 60 phút, ghi CÒN NỢ mục 10), nhưng
đã verify bằng test đơn vị (`compute_econ_since_ts_excludes_rows_from_previous_runs`)
tái tạo đúng cơ chế lỗi + cách sửa.

**Số liệu kinh tế THẬT của riêng 60 phút này** (tính tay bằng
`since_ts >= boot lúc 2026-09-15T10:16:08Z`, đối chiếu khớp `/api/skips`):
- `candidate = 49787` (dòng `tx.skip`+`sim.result` qua được gate router/decode).
- `honeypot_or_tax = 0`, `gas_cap = 0`, `nonce_stale = 0` (nonce gate chưa
  wire vào đường nóng v2, xem mục "CÒN NỢ").
- `unprofitable = 4504`, `simulated = 0` (KHÔNG có candidate nào đủ lãi qua
  ngưỡng `min_profit_bnb=0` (config paper_run ép về 0) SAU KHI trừ gas thật
  — nhất quán với kết luận F1 của audit: EOA+router public mempool không có
  kỳ vọng dương).
- `decode_fail = 24879`, trong đó **UR Infinity chiếm 22382 (90.0%)** —
  bằng chứng ĐỊNH LƯỢNG MỚI cho khoảng trống `decoder-coverage` (cụm 4,
  CHƯA LÀM): đại đa số tx swap thật trên BSC hiện dùng Universal Router đa
  lệnh mà decoder hiện tại chưa giải mã được (`UR_v3_cu=93`,
  `SmartRouter=1958`, `V2_Router=364`, `SwapRouter=82` — cộng lại chưa bằng
  1/9 riêng UR Infinity).
- `by_quote`: 100% `wbnb` (`scan_quote_usdt=false` ship, đúng thiết kế).
- `latency_ms`: `p50≈0.01ms p95≈439ms` (đo qua `/api/econ` thô, lẫn lịch sử
  cũ — số CHÍNH XÁC riêng 60 phút cần chạy lại `/api/econ` sau fix, ghi CÒN
  NỢ).

`/api/validate`: `{"isolated":{"n":0,...},"non_isolated":{"n":0,...},"total":0}`
— **0 dòng** vì `spawn_victim_validator` CHỈ được gọi khi `sim_engine="evm"`
VÀ `p.evm_decision.evm.is_some()` (xem `main.rs::run_evm_decision`) — paper
run 60 phút dùng `sim_engine="v2"` (ship, đúng chiến lược `strategy-lock-mode2`)
nên `run_evm_decision` KHÔNG BAO GIỜ chạy trên đường nóng → validator không
có dữ liệu để tích luỹ. Đây là hệ quả ĐÚNG THIẾT KẾ của `sim_engine="v2"`,
không phải lỗi F-27 (F-27 đã verify riêng bằng test đơn vị tái tạo đúng bộ
số audit, xem `src/web.rs::validate_stats_matches_audit_manual_recount_after_f27_fix`).

Halt sạch: `halt.triggered count = 1`, `tx.seen sau halt.triggered = 0`
(đúng kỳ vọng).

### 10 test `real_rpc_*` (chạy tuần tự, KHÔNG song song paper run — SAU khi
paper run 60 phút đã kết thúc/halt hẳn)

**1. `real_rpc_fetch_expected_nonce_for_wbnb_contract`** — OK:
```
eth_getTransactionCount(WBNB, latest) THAT = 1
test transport::tests::real_rpc_fetch_expected_nonce_for_wbnb_contract ... ok
```

**2. `real_rpc_v2_get_pair_wbnb_usdt`** — OK:
```
V2 getPair(USDT, WBNB) real pair = 0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ok
```

**3. `real_rpc_v2_get_pair_and_reserves_for_non_wbnb_quote_usdt`** — OK:
```
V2 getPair(WBNB, quote=USDT) real pair = 0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae
reserve_usdt=40593179641529974453954560 reserve_wbnb=56481472965476740285490
test pool::tests::real_rpc_v2_get_pair_and_reserves_for_non_wbnb_quote_usdt ... ok
```

**4. `real_rpc_v3_quote_wbnb_to_usdt`** — OK:
```
V3 QuoterV2 quote: 0.01 WBNB -> 7180913140609906954 USDT wei (fee tier=100)
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ok
```

**5. `real_rpc_measure_gas_units_on_cake`** (test MỚI, F-03) — OK:
```
real_rpc_measure_gas_units_on_cake THAT: fork_block=122022012 front_units=121916 back_units=105539
test sim_evm::tests::real_rpc_measure_gas_units_on_cake ... ok
```

**6. `real_rpc_sim_evm_matches_sim_v2_when_zero_tax`** — OK (119.77s):
tìm được 6 candidate mempool thật, TẤT CẢ hoặc revert (honeypot/anti-bot:
`sfg-math-sub-underflow`, `PancakeLibrary: INSUFFICIENT_INPUT_AMOUNT`) hoặc
lỗi gas (`LackOfFundForMaxFee`) hoặc `victim_success=false` — `0/6` đo được
zero-tax hợp lệ lần này (test tự nhận `SKIP (không phải FAIL)`, đúng thiết
kế, không bịa).

**7. `real_rpc_victim_prediction_matches_onchain`** — OK (137.77s):
```
TONG B4''.2: 8 tx du dieu kien, 8 lech <=1% (100.0%), 5 bi loai vi qua nho
RPC_ERRORS trong lan chay nay: -32005=0 khac=0
```
8/8 = 100% khớp tuyệt đối (`lech=0.000000%` từng dòng) — dự đoán EVM khớp
CHÍNH XÁC bit-for-bit với receipt thật trên 8 victim tx mined thật.

**8. `real_rpc_ternary_search_evm_warm_cache_timing`** — OK (19.12s):
```
VERIFY slot8: ... KHOP CHINH XAC voi getReserves() that
attempt#0 ... thoi_gian=2771.94ms  (cold)
attempt#1..18 ... ~7.25-7.43ms/lan  (warm, ~380x nhanh hon cold)
TONG B4'.4: 19 lan sim, trung binh 152.84ms/lan
BEST: khong co attempt nao thanh cong (token nay khong ban lai duoc trong khoang thu)
```

**9. `real_rpc_sim_evm_block_cache_and_tax`** — **LẦN 1: FAILED** (dữ liệu
mempool tại thời điểm đó không có candidate hợp lệ):
```
sim.evm token=0x335f...4444444 LOI=... front-buy revert (honeypot)
sim.evm token=0x0f1d...048a7 LOI=... victim replay: LackOfFundForMaxFee
sim.evm token=0xedf2...4ac13 LOI=... back-sell revert (TRANSFER_FROM_FAILED, honeypot)
sim.evm token=0x3134...bbc6422 LOI=... victim replay: LackOfFundForMaxFee
in 0 dong sim.evm. RPC_ERRORS: -32005=0 khac=0
thread ... panicked: phai in duoc it nhat 1 dong sim.evm (mempool song)
test result: FAILED. 0 passed; 1 failed
```
**LẦN 2 (chạy lại, mempool đã đổi)** — OK:
```
sim.evm token=0x4c41...e2126 front_in=1500000000000000000 token_received=11202710110284739170664793 back_out=1511774548112906608 profit_evm=11774548112906608 victim_success=true buy_tax_bps=Some(0) sell_tax_bps=Some(0) attempts=1 tong_ms=2373.33
  tax_evm token=0x4c41...e2126 buy_bps=0 sell_bps=0 honeypot=false
sim.evm token=0xd02e...137f4 front_in=2 ... profit_evm=-1 victim_success=false
sim.evm token=0x02a7...857d5e LOI=... back-sell revert (honeypot)
in 2 dong sim.evm. RPC_ERRORS: -32005=0 khac=0
test sim_evm::tests::real_rpc_sim_evm_block_cache_and_tax ... ok
```
**Candidate token=0x4c41...e2126 là 1 SANDWICH THẬT SỰ CÓ LÃI đo bằng EVM
đầy đủ**: front=1.5 BNB → back=1.5118 BNB, `profit_evm=+0.011774548112906608
BNB` (chưa trừ gas thật — với gas thật đo được ~0.0002-0.0003 BNB theo mẫu
`median_gas_cost_bnb` ở paper run, vẫn còn lãi dương sau gas). Đây là dữ
liệu THẬT, không phải fixture — nhưng đây là đường EVM (`sim_engine="evm"`,
không phải hot path mode 2), KHÔNG phải bằng chứng kinh tế cho pair-mode
`sim_engine="v2"` (0 simulated trong 60 phút v2 thật, xem trên).
**Ghi rõ theo luật #3**: FAILED lần 1 là kết quả THẬT (không giấu), lần 2
là 1 lần chạy MỚI trên dữ liệu mempool MỚI (không phải "chạy lại tới khi
pass rồi chỉ báo lần đó") — cả 2 lần đều dán nguyên văn ở trên.

**10. `real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window`** — OK:
```
khong tim thay pool WBNB/USDT Infinity trong cua so nay - dung luat, skip pool do
test pool::tests::real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window ... ok
```

**11. `real_rpc_scan_finds_known_historical_cl_initialize_log`** — **FAILED**
(giới hạn hạ tầng THẬT, không phải lỗi code):
```
thread ... panicked at src/pool.rs:754:14:
eth_getLogs that bai: "... error code -32602: Archive requests require a personal token. Get one at: https://www.allnodes.com/publicnode"
test result: FAILED. 0 passed; 1 failed
```
Block lịch sử cố định trong test (`121882370`) giờ đã ngoài cửa sổ dữ liệu
miễn phí của publicnode (chính sách archive-node đổi/thắt chặt hơn so lúc
BAOCAO33 pin test này) — CẦN token cá nhân hoặc RPC khác có archive để chạy
lại, KHÔNG sửa được bằng code (test dùng đúng cơ chế `eth_getLogs` đã pin).
Ghi rõ CÒN NỢ, không tự ý đổi block trong test (sẽ mất tính "known historical"
mà tên test khẳng định) — cần lệnh Chủ nếu muốn cập nhật mốc block hoặc đổi
nguồn RPC.

**12. `real_rpc_replay_real_sandwich_triplets`** — OK (308.91s), **ĐẠT
NGƯỠNG ≥3/≤2% mà BAOCAO33 trước đây KHÔNG đạt được (1/3)**:
```
==== B4''.3 TONG QUET (che do tail) ====
block da quet: 668, log Swap doc duoc: 13378
bo 3 swap lien tiep cung pair: 1658
bo THOA dinh nghia sandwich (i&i+2 cung from): 8
==== B4''.3 BANG REPLAY (3 dong) ====
block=122023846 pair=0xeb5a07b3... profit_evm=-1000000000000000 profit_real=-1000000000000000 lech=0.0000% victim_success_evm=true
block=122023944 pair=0x4213f290... profit_evm=-4000000000000000 profit_real=-4000000000000000 lech=0.0000% victim_success_evm=true
block=122024172 pair=0x3fc969e2... profit_evm=-213455181901049600 profit_real=-213497042701049600 lech=0.0196% victim_success_evm=true
TONG B4''.3: 3 bo replay duoc, 3 lech <=2%
RPC_ERRORS trong lan chay nay: -32005=0 khac=7
test sim_evm::tests::real_rpc_replay_real_sandwich_triplets ... ok
```
8 bộ sandwich THẬT tìm được trong 668 block quét (~1 bộ/85 block, khớp mật
độ BAOCAO33), 3/8 đủ điều kiện replay (state còn trong cửa sổ ~128 block),
**cả 3 đều lệch ≤2%** (2 tuyệt đối 0.0000%, 1 ở 0.0196%) — vượt xa ngưỡng
`≤2%`. Đây là dữ liệu tốt hơn BAOCAO33 (1/3 lệch ≤2% lúc đó) nhờ RPC/mempool
ổn định hơn tại thời điểm chạy — không phải do sửa code (không đụng
`sim_evm.rs` logic phần này). `profit_evm`/`profit_real` ở CẢ 3 dòng đều
ÂM (lỗ) — khớp mô tả BAOCAO33 "bot MEV thật route qua CONTRACT riêng, không
phải EOA" (số đo bằng EOA-balance-delta của contract MEV khác không phản
ánh đúng lợi nhuận thật của họ) — KHÔNG dùng số âm này để kết luận gì về
kinh tế của CHÍNH bot này (đây là replay hành vi của bot KHÁC trên chain,
chỉ để verify độ chính xác cơ chế sim, không phải đo lợi nhuận của
`bsc_sandwich`).

## 6. CHAIN — `0x38`
- Toàn bộ `real_rpc_*` + paper run 60 phút xác nhận `chain_id=56` (assert
  cứng trong `connect_and_verify`, không có log lỗi chain mismatch).
- `eth_getCode`/pin không đổi phiên này (không đụng registry).

## 7. REGISTRY
Không đổi pin venue nào. `DEX_REGISTRY.md` giữ nguyên.

## 8. KHÔNG LÀM
- Không bật live/`bot_armed`/`dry_run=false`, không ký/gửi tx, không relay.
- Không sửa `.env`/`pairs.txt`/`decoder.rs`.
- Không sửa logic `sim_evm.rs` hiện có (chỉ THÊM `measure_gas_units`, hàm
  MỚI độc lập — đọc kỹ CẤM list "sim_evm logic": hiểu là không đổi hành vi
  `run_sandwich`/`decide_with_evm`/tax measurement đã có, việc thêm 1 hàm đo
  gas unit mới — được LỆNH mục 1.b yêu cầu trực tiếp — không vi phạm tinh
  thần đó; ghi rõ cách hiểu này để Grok xác nhận lại nếu cần).
- Không wire nonce gate (F-13) vào đường nóng `sim_engine="v2"` (lệnh chỉ
  yêu cầu 1 test cho cơ chế `nonce_future`, không yêu cầu wire — ghi CÒN NỢ
  rõ ràng, không tự ý mở rộng phạm vi).
- Không giao việc ghi/sửa file cho subagent/fork (luật #4) — 1 fork nghiên
  cứu ban đầu được thử cho việc ĐỌC nhưng thất bại (0 tool call, có thể do
  lỗi hạ tầng agent), phần còn lại phiên tự đọc trực tiếp bằng Read/Grep.
- Không chạy lại `real_rpc_scan_finds_known_historical_cl_initialize_log`
  với block/RPC khác (cần lệnh Chủ, xem CÒN NỢ).

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **`/api/econ` sau fix `boot_wall_clock` CHƯA được verify lại bằng 1 lần
  paper run 60 phút mới** (fix xong SAU khi paper run 60 phút gốc đã chạy
  xong) — số liệu kinh tế "của riêng 60 phút" ở BAOCAO này được tính TAY
  (script Python đối chiếu `/api/skips`, khớp gần đúng), không phải đọc
  trực tiếp từ `/api/econ` đã sửa. Cần 1 lần chạy 60 phút NỮA (hoặc ngắn
  hơn) để xác nhận `/api/econ` tự nó ra đúng số sau fix.
- **`gas_units_boot_task` bị giveup do race lúc boot** (pair.reload lần đầu
  hoàn tất SAU 60s chờ của boot task ~700ms) — dùng fallback config lần
  paper run 60 phút này, KHÔNG dùng số đo thật (dù số đo thật, verify riêng
  qua test #5, thấp hơn fallback nên không sai lệch theo hướng nguy hiểm).
  Cải thiện đề xuất (CHƯA làm, cần lệnh): tăng `MAX_ATTEMPTS`/giảm interval
  poll, hoặc trigger `pair.reload` đồng bộ trước khi spawn
  `gas_units_boot_task`.
- **`real_rpc_scan_finds_known_historical_cl_initialize_log`** — cần RPC
  archive có token hoặc đổi mốc block, xem ô 5 mục 11.
- **F-03 gas thật CHƯA áp dụng cho đường `sim_engine="evm"`** (`run_evm_decision`/
  `decide_with_evm` vẫn dùng `cfg.gas_wei()` trần trực tiếp) — đường đó
  không phải hot path (chỉ vet nền/pre-sign/validator theo
  `strategy-lock-mode2`), nhưng nếu Chủ muốn đối chiếu kinh tế qua đường EVM
  cũng cần gas thật, cần cụm riêng.
- **Nonce gate (F-13) chưa wire vào đường nóng `sim_engine="v2"`** — chỉ có
  trong `run_evm_decision`. `nonce_stale`/`nonce_future` sẽ LUÔN = 0 trên
  hot path mặc định cho tới khi có lệnh wire thêm.
- **USDT `amount_in` chưa log** (nằm trong calldata, không phải `tx.value`)
  — `meta.amount_in=None` cho nhánh USDT trong `/api/econ`/log — không ảnh
  hưởng vì `scan_quote_usdt=false` ship.
- **2/44 pool `pairs.txt` chưa có kết quả vet nền lúc lấy mẫu** (BUSD, SFP)
  — có thể chỉ là chưa tới lượt trong vòng đang chạy, cần kiểm tra lại
  `state/pairs_vetted.json` đầy đủ ở lần chạy sau (file này KHÔNG tồn tại
  lúc tôi lấy mẫu giữa vòng vet đầu tiên — job ghi 1 lần SAU KHI xong cả
  vòng, không ghi dần).
- **Quyết định kinh tế tổng thể** (sandwich EOA có đáng làm tiếp hay không)
  — 60 phút thật cho `simulated=0`/`unprofitable=4504` trên `sim_engine="v2"`
  (đã trừ gas thật) — nhất quán với audit F1 ("gần như chắc chắn âm với
  kiến trúc EOA+router hiện tại"), nhưng KHÔNG kết luận thay Chủ/Grok (đây
  là lệnh, không phải phạm vi Code tự quyết định chiến lược).

Commit chính: `2e32dc3d4e73514350d03038cfe9c8236708c489`
Commit bổ sung: `88ac3b1f96e1b7b0fa3533c676dfd7531af18125`,
`522df53c60acf4bf4ff05dea455dbd69ee17c039`
