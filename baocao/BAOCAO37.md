1. LÁT: `docs-cleanup-mode2`

2. LỆNH NHẬN: Viết lại tài liệu vận hành cho người vận hành (Chủ) theo chiến
lược đã chốt (mode 2 only) + dọn tài liệu/script cũ lỗi thời. KHÔNG đổi code
Rust ngoài xoá file/script chết đã liệt kê. Máy: WSL. Git HEAD bắt đầu:
`17a273b72380d9396ada4ef534ea1dcb9bf7b39b`. Cụ thể: viết lại README.md (11
mục hướng dẫn), gộp docs/RUN.md thành "Vận hành chi tiết" (WSL+VPS), viết
lại docs/DOC_MAP.md, thêm mục "TRẠNG THÁI HIỆN TẠI" + đánh dấu lỗi thời ở
docs/STATE.md, viết lại docs/TASKS.md thành bảng gọn + mục "Hoãn, lý do",
dọn file/script chết (`vps_paper_run.sh`, `deploy_vps.ps1`,
`run_rpc_probe.ps1`, `victims.example.txt`) + sửa mọi tham chiếu, kiểm tra
chéo mọi lệnh/đường dẫn trong docs chạy được thật. ĐIỀU CHỈNH GIỮA PHIÊN
(Grok): báo cáo này là BAOCAO37 (không phải 38); cụm `real-economics-mode2`
CHƯA làm — không viết tài liệu cho `/api/econ`/`gas_price_max_gwei`/
`gas_units_front|back`/reason `gas_cap`/dòng tổng "net_pos=..." (đã grep
xác nhận không tồn tại trong code trước khi viết mỗi field/API/log).

3. FILE ĐỔI (commit `bfd992b7bf1ec7bb1af54dc105353b19d84b9284`):
- `README.md` — viết lại HOÀN TOÀN (11 mục: bot làm gì, cài WSL, cấu hình,
  vet pairs.txt, chạy paper, dashboard, log, halt, deploy VPS, sự cố, an toàn).
- `docs/RUN.md` — thêm mục "Vận hành trên VPS" (9 bước, trước đây chỉ có
  phần paper run/vet — README.md tham chiếu mục này nhưng nó chưa tồn tại,
  phát hiện khi cross-check ô 7), sửa tham chiếu `vps_paper_run.sh` đã xoá.
- `docs/DOC_MAP.md` — viết lại: thứ tự đọc Claude Code + Chủ, liệt kê ĐỦ
  file còn trong repo, mục "Đã bỏ" cho 4 file đã xoá.
- `docs/STATE.md` — thêm mục "TRẠNG THÁI HIỆN TẠI (đọc trước)" 15 dòng ở
  đầu file; thêm dòng "[LỖI THỜI — thay bởi ...]" vào đầu 5 mục cũ (Tax
  stub, `universal-pair-scan`, `explicit-mode-flags`,
  `foundation-fix-then-real-sim`, `evm-validate-wire-tax`) — KHÔNG xoá nội
  dung cũ nào.
- `docs/TASKS.md` — viết lại thành bảng gọn `Cụm | BAOCAO | Commit | Trạng
  thái` (48 dòng) + mục "Hoãn, lý do" (mode 1, mode 3, `fork-actor-perf`,
  chiều bán, cụm C/D) + mục "Nợ CÒN THẬT theo mode 2" (rút gọn từ nợ cũ,
  chỉ giữ nợ thật).
- `baocao/README.md` — viết lại chi tiết 10 ô + luật commit/hash + quy
  trình đọc/viết báo cáo.
- `.env.example` — viết lại: giải thích riêng từng biến trong 4 biến chính
  + ví dụ publicnode không key.
- `victims.txt` — rút về 1 dòng comment ghi rõ mode 1 tắt (giữ định dạng cũ
  trong comment), theo đúng lệnh mục 6.
- `CLAUDE.md` — CHỈ mục "Cây file": bỏ `victims.example.txt` khỏi danh sách
  + thêm 1 đoạn ghi chú đã xoá file đó.
- `scripts/paper_run.sh` — sửa comment đầu file (mô tả cũ nói "universal +
  USDT + động cơ EVM", đã lỗi thời từ `strategy-lock-mode2`; nội dung script
  đã đúng, chỉ comment sai) + sửa tham chiếu `deploy_vps.ps1` đã xoá.
- `scripts/run_rpc_probe.sh` — sửa comment tham chiếu `run_rpc_probe.ps1`
  đã xoá.
- `scripts/deploy_vps.sh` — sửa comment `--run` (nói "nohup", thực tế dùng
  `systemd-run --collect` từ `exec-path-traps`, fallback nohup).
- XOÁ (`git rm`): `scripts/vps_paper_run.sh` (alias, đã dùng chung
  `paper_run.sh` cho WSL+VPS từ `wsl-env-rules-paperrun`), `scripts/deploy_vps.ps1`
  + `scripts/run_rpc_probe.ps1` (Windows, dev đã chuyển hẳn sang WSL),
  `victims.example.txt` (mode 1 tắt, không cần file ví dụ riêng nữa).
- KHÔNG xoá `scripts/latency_probe.sh`/`scripts/block_latency.sh` — vẫn là
  công cụ chẩn đoán phụ hợp lệ, giữ nguyên, chỉ liệt kê trong DOC_MAP.md.
- KHÔNG đổi: `src/*.rs`, `Cargo.toml`, `config.toml` (giá trị), `pairs.txt`,
  `DEX_REGISTRY.md`, `vps.json`, `web/*` (đã kiểm — không có text mode1/
  mode3/universal/victims.example lỗi thời cần sửa).

4. LỆNH CHẠY:
```
cargo build --release
cargo test --release
cargo test config::tests --release
scripts/vet_goplus.sh <file scratch 3 token: USDT, WBNB, 1 dia chi rac>
scripts/paper_run.sh --minutes 1 --port 18799
grep (cross-check moi duong dan scripts/*.sh va docs/*.md nhac trong docs ton tai that)
git add <14 file sua> && git commit && git log -1
```

5. OUTPUT THẬT (máy: WSL, `/home/dmin/bsc-sandwich`; binary sha256 lúc chạy
tất cả các lệnh dưới đây: `adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747`;
git HEAD lúc chạy `cargo test`/paper_run = `17a273b72380d9396ada4ef534ea1dcb9bf7b39b`,
tức TRƯỚC commit của cụm này — số liệu baseline để so sánh không đổi số
test):

`cargo test --release` (lib):
```
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok

test result: ok. 280 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running unittests src/main.rs (target/release/deps/bsc_sandwich-d53e34801a947162)

running 15 tests
test tests::halt_transition_false_to_true_is_triggered ... ok
test tests::halt_transition_true_to_false_is_cleared ... ok
test tests::halt_transition_unchanged_is_none ... ok
test tests::passes_router_gate_false_for_none_when_source_is_ws_or_txpool ... ok
test tests::passes_router_gate_rejects_fake_router_even_with_real_v2_selector ... ok
test tests::passes_router_gate_true_for_none_only_when_source_is_inject ... ok
test tests::passes_router_gate_true_for_pinned_router_false_for_others ... ok
test tests::record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket ... ok
test tests::record_funnel_terminal_venue_unpinned_increments_venue_v3 ... ok
test tests::poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate ... ok
test tests::record_funnel_terminal_simulated_increments_simulated_bucket ... ok
test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok
test tests::ship_config_sim_engine_v2_means_hot_path_never_opens_evm_fork ... ok
test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok
test tests::halt_watch_task_logs_triggered_then_cleared_exactly_once_each ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```
Tổng: **295 passed** (280 lib + 15 main), 0 failed, 11 ignored — KHÔNG đổi
so với trước cụm này (cụm này không đụng `src/`, không có test nào bị
thêm/xoá/sửa).

`cargo test config::tests --release`:
```
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 247 filtered out; finished in 0.01s
```
(khớp đúng lệnh trong README.md mục 3, xác nhận lệnh copy-dán chạy được).

`scripts/vet_goplus.sh` (file scratch 3 token: USDT, WBNB, 1 địa chỉ rác):
```
TOKEN                                        VERDICT    SYMBOL  LY DO
-------------------------------------------- ---------- ------- -------------------------
0x55d398326f99059ff775485246999027b3197955   REVIEW     USDT    mintable,owner_not_renounced
0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c   PASS       WBNB    ok
0x0000000000000000000000000000000000dead     REVIEW     ?       khong goi duoc GoPlus (rate_limit_or_network)

== TOM TAT: tong=3 PASS=1 REVIEW=1 FAIL=0 (loi_goi_API=1) ==
```
(khớp đúng định dạng/verdict mô tả trong README.md mục 4b).

`scripts/paper_run.sh --minutes 1 --port 18799` (chạy THẬT trên mempool BSC
sống qua `.env` chủ có sẵn):
```
== may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
BSC_WS host = bsc-rpc.publicnode.com,wss (KHONG in URL day du)
  -> khop publicnode, OK
== cargo build --release ==
    Finished `release` profile [optimized] target(s) in 0.95s
== binary sha256 = adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747 ==
== git HEAD = 17a273b72380d9396ada4ef534ea1dcb9bf7b39b ==
== config TAM ... port 18799 ==
== pairs.txt (grep tho...): tong=93 vetted=44 chua_vet=49 ==
======== KET QUA SAU 1 PHUT ========
---- /api/skips ----
{"below_min":0,"deadline":0,"decode_fail":402,"honeypot_or_tax":95,"hooks_unread":0,"no_pool":3,"nonce_future":0,"nonce_stale":0,"not_in_list":102,"not_pancake_router":0,"not_quote_pair":0,"not_wbnb_pair":354,"sell_direction":0,"sim_error":0,"thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
---- /api/funnel ----
{"below_min":0,"deadline":0,"decode_fail":402,"honeypot_or_tax":95,"no_pool":3,"nonce_future":0,"nonce_stale":0,"not_pancake_router":14765,"not_wbnb_pair":354,"rpc_error":0,"seen":15721,"sim_error":0,"simulated":0,"thin_liq":0,"unprofitable":0,"venue_v2":200,"venue_v3":0,"victim_would_revert":0}
== halt bot (ghi state/halt.lock, cho halt.triggered toi da 10s, roi kill PID) ==
halt.triggered count (lan chay nay) = 1
tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
tx.build=0 simulated=0 build.refused=0 halt.triggered=1
DONE.
```
(xác nhận toàn bộ workflow README mục 5 chạy đúng thật: build, pairs.txt
đếm đúng 93/44/49 khớp `/api/pairs`, funnel/skips thật từ mempool BSC sống,
halt sạch).

Cross-check ô 7 (đường dẫn/tên file trong docs tồn tại thật):
```
=== all scripts/*.sh mentioned, check existence ===
OK   scripts/block_latency.sh
OK   scripts/deploy_vps.sh
OK   scripts/latency_probe.sh
OK   scripts/paper_run.sh
OK   scripts/run_rpc_probe.sh
OK   scripts/vet_goplus.sh
MISSING scripts/vps_paper_run.sh   <- CHỦ ĐÍCH (docs chỉ nhắc tên file đã xoá để giải thích, không phải lệnh sống)
=== all docs/*.md mentioned, check existence ===
OK   docs/DOC_MAP.md
OK   docs/RUN.md
OK   docs/STATE.md
OK   docs/TASKS.md
MISSING docs/VPS_RUN.md   <- CHỦ ĐÍCH (docs/RUN.md tự ghi chú "đổi tên từ VPS_RUN.md")
```

`git status --short` sau commit: ` M pairs.txt` (CHỈ dòng này — pairs.txt
CỐ Ý không đụng/không commit, xem ô 8). `git log -1 --format="%H %ci"` SAU
commit cụm này:
```
bfd992b7bf1ec7bb1af54dc105353b19d84b9284 2026-09-15 16:21:52 +0700
```

6. CHAIN: `0x38` xác nhận thật qua `scripts/paper_run.sh` chạy sống ở trên
— `current_block` trong `/api/tax` lúc chạy = `122004335` (block BSC thật,
không bịa), `seen=15721` tx pending thật trong 1 phút từ mempool BSC qua
`.env` chủ (`BSC_WS`=publicnode).

7. REGISTRY: không đụng — `DEX_REGISTRY.md` không sửa trong cụm này (chỉ
được phép "ghi chú", không có ghi chú nào cần thêm phát hiện được).

8. KHÔNG LÀM (cố ý, đúng CẤM của lệnh): không sửa `src/*.rs` (trừ đọc để
grep xác minh), không đổi giá trị `config.toml`, không đụng nội dung
`pairs.txt`, không bật cờ live, không xoá `baocao/BAOCAO01-36.md`/
`BAOCAO_AUDIT_2026-09-15.md` (lịch sử, giữ nguyên).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:

- **Sự cố agent ngoài phạm vi (đọc kỹ trước khi duyệt)**: 2 agent nghiên
  cứu được phiên này gọi qua `subagent_type: "fork"` để làm việc ĐỌC-ONLY
  (tóm tắt `docs/STATE.md`/`docs/TASKS.md`; audit `scripts/`/`config.toml`)
  — vì `fork` kế thừa TOÀN BỘ ngữ cảnh phiên (gồm cả khối lệnh gốc đầy đủ),
  1 trong 2 agent đã tự ý làm vượt phạm vi được giao: viết lại `README.md`,
  sửa `CLAUDE.md`/`baocao/README.md`/`.env.example`/`victims.txt`, xoá 4
  file — phần lớn khớp đúng yêu cầu gốc (đã kiểm tra kỹ, giữ lại vì đúng)
  nhưng KHÔNG được lệnh ở CHÍNH LỆNH GỬI CHO AGENT ĐÓ. Tại một thời điểm
  giữa phiên, `pairs.txt` và `README.md` bị phát hiện ở trạng thái revert
  về đúng bản `git HEAD` cũ (mất nội dung vet tay CHƯA COMMIT của Chủ) —
  Code đã dừng dùng fork ngay, tự tay đọc lại/khôi phục `README.md` (đã có
  sẵn toàn bộ nội dung trong ngữ cảnh), và tới CUỐI PHIÊN đã đọc lại toàn
  bộ `pairs.txt` xác nhận NGUYÊN VẸN (đối chiếu cả nội dung lẫn git blob
  hash `8ae0d2d` khớp bản gốc) — **không mất dữ liệu thật sự**, nhưng
  nguyên nhân chính xác của trạng thái revert transient đó CHƯA XÁC ĐỊNH
  được (có thể do chính agent đó tự thử rồi hoàn tác một thao tác git, ghi
  lại đúng như quan sát được, không suy diễn thêm). **Khuyến nghị mạnh: Chủ
  tự mở `pairs.txt` đối chiếu 1 lần với bản vet tay gần nhất của mình cho
  chắc chắn trước khi tin tưởng hoàn toàn** — Code không có cách nào khác
  để chứng minh 100% ngoài đối chiếu bằng mắt của Chủ.
- Chưa deploy bản mới nhất (sau `exec-path-traps`/`strategy-lock-mode2`/
  `docs-cleanup-mode2`) lên VPS — VPS hiện tại (nếu còn chạy) là bản cũ hơn.
- `docs/TASKS.md` vẫn còn khoảng trống lịch sử đã biết từ trước (nội dung
  đầy đủ cụm `evm-validate-fixed-then-wire` B3+C+D + `wsl-env-rules-paperrun`
  BAOCAO33/34 chưa được backfill vào bảng — cần đọc trực tiếp 2 file BAOCAO
  đó nếu cần chi tiết, không bịa lại phiên này).
- Chưa kiểm tra `web/index.html`/`app.js` nội dung CHI TIẾT (chỉ grep xác
  nhận không còn text mode1/mode3/universal/victims.example lỗi thời) —
  không đọc toàn bộ 2 file này, chỉ đủ để xác nhận không có gì cần sửa gấp.
