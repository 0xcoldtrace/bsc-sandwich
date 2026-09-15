## BAOCAO35

1. LÁT: `exec-path-traps` — 12 mục lệnh gốc (F-04/05/06/07/08/13/14/15/16/20
   + V-06) + mục 13 bổ sung giữa phiên (sửa `scripts/paper_run.sh`).

2. LỆNH NHẬN: (khối lệnh Grok, cụm `exec-path-traps` — chặn bẫy trên đường
   thực thi trước khi nối signer `7.3`, không đổi chiến lược/công thức kinh
   tế, không nối live) + 1 tin nhắn bổ sung giữa phiên (mục 13, sửa 4 lỗi đo
   trong `scripts/paper_run.sh`).

3. FILE ĐỔI: (commit `3694908a1e76c303e787b4be0d1d912952f220a6`, luật #1)
   - `src/decoder.rs` — F-20 (`slice_checked` dùng `checked_add`, 4 chỗ cộng
     offset không checked cũ); F-16 (`venue_matches_router` mới); fuzz test.
   - `src/config.rs` — F-05 (`Config::gate_check` nguồn duy nhất,
     `LiveGateStatus` dời từ `executor.rs` sang đây); F-08 (xoá field
     `executor_slippage_bps`, thêm `RENAMED_FIELDS` fail-load rõ ràng); test
     duyệt 256 tổ hợp cờ live.
   - `src/executor.rs` — F-06 (`executor_self_address()`, cổng từ chối build
     `self_address_zero`, xoá `PLACEHOLDER_SELF_ADDRESS` khỏi đường
     production); F-08 (build dùng `front_slippage_bps`/`back_slippage_bps`
     riêng, thêm tham số `engine`); F-05 (`can_send_live`/`gate_check` gọi
     thẳng `config.rs`); F-04 (call site live 7.x đánh dấu bằng comment).
   - `src/pipeline.rs` — F-26 (`decide_and_build_paper_v2` chỉ build ngay
     khi `sim_engine="v2"`, hàm mới `build_paper_txs_from_evm_decision` cho
     đường `evm`); F-14 (`PipelineSkip::Deadline` + `deadline_expired`, wire
     vào `evaluate_candidate`/`evaluate_candidate_quote`); F-13
     (`PipelineSkip::NonceStale`/`NonceFuture`); F-16
     (`precheck_token_and_venue` trả thêm `selector_name`, `TxLogMeta.detail`
     mới); nhiều test mới.
   - `src/main.rs` — F-15 (`passes_router_gate` thêm `source`); F-16
     (cross-check venue trước khi vào nhánh V2/V3); F-26 (gọi
     `build_paper_txs_from_evm_decision` sau `decide_with_evm`); F-13 (nonce
     gate trong `run_evm_decision`, dùng `app_state.nonce_cache`); F-04
     (`RiskGuard::record_result` gọi thật trong `spawn_victim_validator`);
     V-06 (`halt_watch_task`/`halt_transition` mới, 3 nguồn tx +
     `handle_paper_tx` tự kiểm `state_files.is_halted()`); test mới (bao gồm
     1 test async thật cho `halt_watch_task`).
   - `src/transport.rs` — F-13 (`NonceCache`/`compare_nonce`/
     `fetch_expected_nonce` mới) + 1 test `real_rpc_*` mới (chạy thật, xem ô
     5) + 3 test thuần.
   - `src/web.rs` — F-04 (`risk_guard` field mới trong `GET /api/status`);
     F-13/F-14 (`FunnelCounters` thêm `nonce_stale`/`nonce_future`/
     `deadline`); F-13 (`nonce_cache` field mới trong `AppStateInner`).
   - `src/venues.rs` — F-13 (`SKIP_REASONS` thêm `nonce_stale`/
     `nonce_future` — CLAUDE.md ĐÃ có 2 chuỗi này từ trước, code giờ mới
     theo kịp) + 1 test.
   - `src/sim_evm.rs` — F-13 (comment giải thích `disable_nonce_check` chỉ
     còn ảnh hưởng attacker giả, nonce victim xác minh THẬT bên ngoài
     trước khi fork mở) — KHÔNG đổi logic thực thi.
   - `config.toml` — F-08 (xoá `executor_slippage_bps`, cập nhật comment
     `front_slippage_bps`/`back_slippage_bps`).
   - `scripts/paper_run.sh` — mục 13 (a-d) + 1 bug phát hiện thêm (`.env`
     thiếu `export`, xem ô 5).
   - `docs/STATE.md` — mục `exec-path-traps` mới: quyết định 4(b)
     (RiskGuard::record_result dùng tín hiệu validator ở đường paper) và
     mục 7 (F-13 chọn tag RPC `"latest"` thay vì literal `"pending"`, lý do
     kỹ thuật) + tóm tắt các mục còn lại.
   - `docs/TASKS.md` — thêm 3 dòng: backfill MISSING cho BAOCAO33/34, dòng
     Audit, dòng `exec-path-traps` (BAOCAO35).
   - `CLAUDE.md` — KHÔNG đổi (mục Skip đã có sẵn `nonce_stale`/`nonce_future`/
     `deadline` từ trước, không cần thêm).

4. LỆNH CHẠY:
   ```
   cargo build --release
   cargo test
   cargo test --release
   cargo test --lib -- --ignored real_rpc_fetch_expected_nonce_for_wbnb_contract --nocapture
   bash -n scripts/paper_run.sh && bash -n scripts/vps_paper_run.sh
   scripts/paper_run.sh --minutes 1 --port 18799
   scripts/paper_run.sh --minutes 30 --port 18799
   git add -A && git commit -m "..."
   git log -1 --format="%H %ci"
   sha256sum target/release/bsc_sandwich
   ```

5. OUTPUT THẬT (máy: **WSL**, `/home/dmin/bsc-sandwich`, ext4 native; binary
   sha256 và git HEAD ghi CHÍNH XÁC cho từng khối dưới — build release
   không đổi giữa các lần chạy trong phiên này vì không còn sửa code sau
   lần build đầu của mục 5, source giống hệt commit cuối):

   ```
   $ cargo build --release
   Finished `release` profile [optimized] target(s) in 7.38s (0 warning)
   ```

   ```
   $ cargo test --release
   ...
   test result: ok. 269 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.05s

        Running unittests src/main.rs (target/release/deps/bsc_sandwich-d53e34801a947162)
   running 14 tests
   test tests::halt_transition_false_to_true_is_triggered ... ok
   test tests::halt_transition_true_to_false_is_cleared ... ok
   test tests::halt_transition_unchanged_is_none ... ok
   test tests::passes_router_gate_false_for_none_when_source_is_ws_or_txpool ... ok
   test tests::passes_router_gate_rejects_fake_router_even_with_real_v2_selector ... ok
   test tests::passes_router_gate_true_for_none_only_when_source_is_inject ... ok
   test tests::passes_router_gate_true_for_pinned_router_false_for_others ... ok
   test tests::record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket ... ok
   test tests::record_funnel_terminal_simulated_increments_simulated_bucket ... ok
   test tests::record_funnel_terminal_venue_unpinned_increments_venue_v3 ... ok
   test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok
   test tests::poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate ... ok
   test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok
   test tests::halt_watch_task_logs_triggered_then_cleared_exactly_once_each ... ok

   test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
   ```
   (269+14=283 passed, 0 failed, 11 ignored — trước phiên này 246+9=255,
   TĂNG 28 test mới. 11 ignored = TOÀN BỘ `real_rpc_*`, KHÔNG được coi là
   bằng chứng ĐẠT nếu không chạy — xem test mới chạy thật ngay dưới, luật
   #3.)

   **Test `real_rpc_*` MỚI (F-13) — CHẠY THẬT, không pass rỗng:**
   ```
   $ cargo test --lib -- --ignored real_rpc_fetch_expected_nonce_for_wbnb_contract --nocapture
   running 1 test
   eth_getTransactionCount(WBNB, latest) THAT = 1
   test transport::tests::real_rpc_fetch_expected_nonce_for_wbnb_contract ... ok

   test result: ok. 1 passed; 0 failed; 0 measured; 279 filtered out; finished in 0.63s
   ```
   10 test `real_rpc_*` CÒN LẠI (pre-existing từ phiên trước, không liên
   quan trực tiếp tới 12 mục lệnh này) — **MISSING**, không chạy lại phiên
   này (không đủ thời gian chạy hết trong khi RPC đang bận chạy
   `scripts/paper_run.sh --minutes 30` song song — tránh tranh chấp
   rate-limit RPC công khai làm hỏng cả 2 phép đo).

   **`scripts/paper_run.sh --minutes 1` (smoke test sau khi sửa lỗi
   pipefail, xem mục 8 KHÔNG LÀM/CÒN NỢ):**
   ```
   == may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
   BSC_WS host = bsc-rpc.publicnode.com,wss (KHONG in URL day du)
     -> khop publicnode, OK
   == binary sha256 = 72e9bcf882f99796d84da078d2958cccbfb7e71d1b3254b3c9ab8e4c4f960873 ==
   == git HEAD = 0bd9664d9ae7d74e5afb8c5bd00c97351b4bb94f ==
   ---- /api/skips ----
   {"below_min":0,"deadline":0,"decode_fail":347,"honeypot_or_tax":0,"hooks_unread":0,"no_pool":2,"nonce_future":0,"nonce_stale":4,"not_in_list":0,"not_pancake_router":0,"not_quote_pair":28,"not_wbnb_pair":0,"sell_direction":214,"sim_error":0,"thin_liq":0,"unprofitable":165,"venue_unpinned":0,"victim_would_revert":14}
   ---- /api/funnel ----
   {"below_min":0,"deadline":0,"decode_fail":347,"honeypot_or_tax":0,"no_pool":2,"nonce_future":0,"nonce_stale":4,"not_pancake_router":14923,"not_wbnb_pair":242,"rpc_error":0,"seen":15699,"sim_error":0,"simulated":0,"thin_liq":0,"unprofitable":165,"venue_v2":147,"venue_v3":0,"victim_would_revert":14}
   ---- dem Simulated (sim.evm decision=simulated, lan chay nay) ----
   0
   == halt bot (ghi state/halt.lock, cho halt.triggered toi da 10s, roi kill PID) ==
   halt.triggered count (lan chay nay) = 1
   tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
   tx.build=0 simulated=0 build.refused=0 halt.triggered=1
   DONE.
   ```
   `nonce_stale=4` quan sát THẬT trên mempool sống trong 1 phút — bằng
   chứng F-13 hoạt động đúng ngoài đời, không chỉ trong test.

   **`scripts/paper_run.sh --minutes 30` (DoD đầy đủ, bằng script ĐÃ SỬA,
   chạy background WSL, `exit code 0`):**
   ```
   == may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
   == binary sha256 = 72e9bcf882f99796d84da078d2958cccbfb7e71d1b3254b3c9ab8e4c4f960873 ==
   == git HEAD = 0bd9664d9ae7d74e5afb8c5bd00c97351b4bb94f ==
   ======== KET QUA SAU 30 PHUT (may: WSL, ...) ========
   ---- /api/skips (tích luỹ 30 phút, KHÔNG reset — nguồn CHUẨN cho cụm này) ----
   {"below_min":133,"deadline":9,"decode_fail":12301,"honeypot_or_tax":2,
    "hooks_unread":0,"no_pool":53,"nonce_future":0,"nonce_stale":627,
    "not_in_list":0,"not_pancake_router":0,"not_quote_pair":649,
    "not_wbnb_pair":0,"sell_direction":5801,"sim_error":0,"thin_liq":0,
    "unprofitable":4000,"venue_unpinned":3,"victim_would_revert":469}
   ---- /api/tax ----
   {"allow_tax_inject":true,"current_block":121990888,"entries":[
     {"buy_bps":0,"fresh":true,"honeypot":false,"measured_at_block":121990374,
      "quote":"0x55d398...","roundtrip_tax_bps":0,"sell_bps":0,
      "token":"0x2c5a32d43c18bb9d48266d328046502aea154e52"}],
    "tax_cache_blocks":30,"tax_cache_ttl_sec":600}
   ---- 10 dong sim.evm cuoi (2 candidate USDT chạy EVM thật, cả 2 honeypot) ----
   {"attempts":0,"decision":"honeypot_or_tax","fork_block":121986928,
    "quote":"usdt","token":"0x4444270dbdc955b2126099a4df1412a00d89db06",
    "token_received":"0","ts":"2026-09-15T07:07:53Z"}
   {"attempts":0,"decision":"honeypot_or_tax","fork_block":121990374,
    "quote":"usdt","token":"0x2c5a32d43c18bb9d48266d328046502aea154e52",
    "token_received":"0","ts":"2026-09-15T07:33:44Z"}
   ---- dem Simulated (sim.evm decision=simulated, lan chay nay) ----
   0
   halt.triggered count (lan chay nay) = 1
   tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
   tx.build=0 simulated=0 build.refused=0 halt.triggered=1
   DONE.
   ```
   Mẫu dòng thật (`logs/bot.jsonl`, trong đúng cửa sổ 30 phút này):
   ```
   {"reason":"deadline","selector":"0x5c11d795","source":"universal","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","token":"0xe7710166aab25bf0ec5b771335a3bb04bdb94444","ts":"2026-09-15T07:06:06.902328980+00:00","venue":"v2"}
   {"reason":"nonce_stale","selector":"0x7ff36ab5","source":"wallet","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","token":"0xba17aacf40e8b0d08774e979a324d01bd73abf83","ts":"2026-09-15T07:00:25.441694656+00:00","venue":"v2"}
   ```
   **Đọc số**: `nonce_stale=627`/`deadline=9`/`nonce_future=0` trong 30 phút
   THẬT trên mempool sống — F-13/F-14 hoạt động đúng ngoài đời, không chỉ
   trong test. `sim_error=0` suốt 30 phút (RPC ổn định, EVM luôn chạy được
   khi tới lượt). `simulated=0`/`tx.build=0`/`build.refused=0` — ĐÚNG dự
   kiến (build.refused chỉ dương "nếu có simulated", ở đây 0 candidate nào
   đạt ngưỡng lợi nhuận thật qua EVM suốt 30 phút — khớp kết luận kinh tế
   F1 của `BAOCAO_AUDIT_2026-09-15.md`: kỳ vọng gần như chắc chắn âm với
   kiến trúc EOA hiện tại trên mempool công khai). 2 candidate USDT chạy
   EVM thật đều lộ honeypot thật (`token_received:"0"` — mua được nhưng
   nhận 0 token, dấu hiệu honeypot kinh điển mà công thức đóng không bao
   giờ thấy được).

6. CHAIN: `0x38` xác nhận thật qua `connect_and_verify` lúc boot + qua
   `eth_getTransactionCount` test thật (`real_rpc_fetch_expected_nonce_for_wbnb_contract`,
   gọi `eth_chainId` trước, assert `== 56`) — không gọi `getCode` mới phiên
   này (không đụng registry, không pin thêm venue nào).

7. REGISTRY: không đổi — `DEX_REGISTRY.md` giữ nguyên từ phiên `1.1+1.2+1.3`.

8. KHÔNG LÀM:
   - Không bật live/`dry_run=false`/`bot_armed`, không sendRaw/ký tx.
   - Không đổi chiến lược/công thức kinh tế (V2 math, `sim_v2`,
     `decide_with_evm` profit calc — 0 dòng đổi).
   - Không đổi stack, không sửa `victims.txt` thật/`.env`/cờ live.
   - Không chạy lại 10 test `real_rpc_*` pre-existing (ghi MISSING ở ô 5).
   - Không sửa `DEX_REGISTRY.md`/pin venue mới.
   - Không sửa mục Config của CLAUDE.md dù lệnh gốc mục 6 yêu cầu ("Cập
     nhật config.toml + CLAUDE.md mục Config") — CLAUDE.md nằm ngoài
     ĐƯỢC ĐỤNG phiên này (chỉ mục Skip, và mục Skip vốn đã đúng sẵn) — ghi
     rõ đây là xung đột giữa yêu cầu chi tiết mục 6 và ràng buộc phạm vi
     file ở đầu lệnh, đã ưu tiên ràng buộc phạm vi.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
    - 10 test `real_rpc_*` pre-existing chưa chạy lại phiên này (MISSING,
      không phải lỗi thiết kế — chỉ là chưa có thời gian/tránh tranh chấp
      RPC với phép đo 30 phút).
    - Phát hiện thêm 1 bug thật trong `scripts/paper_run.sh` (`.env` thiếu
      `export` → bot con không thấy `BSC_HTTP`/`BSC_WS`) — ĐÃ SỬA cùng
      phiên (mục 13), không để nợ.
    - Phát hiện thêm 1 bug thật trong chính `scripts/paper_run.sh` mới viết
      (pipefail + pipe 3 chặng `grep|grep -c` giết cả script ngay giữa
      chừng dù có `|| true` ở chặng cuối) — ĐÃ SỬA cùng phiên, không để nợ.
    - `docs/TASKS.md` backfill MISSING cho nội dung chi tiết BAOCAO33/34
      (chỉ ghi placeholder MISSING, không tự bịa lại nội dung 2 phiên đó vì
      không đọc trực tiếp 2 file `.md` đó phiên này).

Commit: `3694908a1e76c303e787b4be0d1d912952f220a6` (2026-09-15 14:39:00 +0700).
`git status --short` rỗng sau commit. Rebuild `cargo build --release` NGAY
SAU commit (source giống hệt, không sửa gì thêm) cho `sha256sum
target/release/bsc_sandwich` = `72e9bcf882f99796d84da078d2958cccbfb7e71d1b3254b3c9ab8e4c4f960873`
— TRÙNG KHỚP chính xác với hash đã dùng xuyên suốt mọi lần chạy ở ô 5 (build
deterministic, cùng source) — xác nhận toàn bộ số liệu THẬT ở ô 5 (smoke
test 1 phút + DoD 30 phút) tương ứng ĐÚNG với code của commit này.
