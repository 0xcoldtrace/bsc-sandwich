## BAOCAO36

1. LÁT: `strategy-lock-mode2` — Chủ CHỐT chiến lược (2026-09-15). Cụm này CHỈ
   cập nhật TÀI LIỆU + CONFIG + KHUNG DỮ LIỆU vet (không đổi thuật toán, không
   đổi decoder), đúng phạm vi lệnh nhận.

2. LỆNH NHẬN: (khối lệnh Grok, cụm `strategy-lock-mode2` — chốt mode 2 only
   [pairs.txt], vet tay + `scripts/vet_goplus.sh` lọc thô, `sim_engine="v2"`
   trên đường nóng, revm giữ 3 việc vet nền/pre-sign/validator. Git HEAD bắt
   đầu: `3694908a1e76c303e787b4be0d1d912952f220a6`.)

3. FILE ĐỔI: (commit `5675f8133a8da90313b3636a3af9fcbead24ffeb`, luật #1)
   - `CLAUDE.md` — mục MỚI "Chiến lược đã chốt (2026-09-15)" (4 quyết định
     chép nguyên văn, đặt ngay sau `## Sản phẩm`); mục Sản phẩm (dòng
     `victims.txt` → "mode 1, TẮT mặc định", thêm dòng `pairs.txt` = nguồn
     candidate duy nhất, định dạng dòng mới `SYMBOL | vetted YYYY-MM-DD |
     tax b/s | owner ... | note`); mục Math (đoạn "EVM THẬT mỗi tx" thay bằng
     giải thích V2 math + gas thật đủ cho token đã vet, giữ ghi chú F-03);
     mục Config ship (`wallet_scan_enabled=false`, `sim_engine="v2"`, thêm 2
     field `pairs_vet_interval_sec=600`/`pairs_require_vetted=true`); mục
     Roadmap (cụm 2 → `real-economics-mode2`, cụm 3 `fork-actor-perf` hạ ưu
     tiên xuống sau cụm 6, có lý do kỹ thuật); Cây file `docs/VPS_RUN.md` →
     `docs/RUN.md`; footer thêm dòng "MODE 2 ONLY...".
   - `config.toml` — `wallet_scan_enabled=false` (đổi từ `true`); comment
     `sim_engine` + giá trị đổi `"evm"` → `"v2"`; 2 field mới
     `pairs_vet_interval_sec=600`/`pairs_require_vetted=true` kèm comment
     giải thích.
   - `src/config.rs` — 2 field struct mới `pairs_vet_interval_sec: u64`/
     `pairs_require_vetted: bool` (bắt buộc, không `#[serde(default)]`);
     `base_toml()` test fixture cập nhật (`wallet_scan_enabled=false`,
     `sim_engine="v2"`, 2 field mới); đổi tên test
     `explicit_mode_flags_ship_default_is_true` →
     `..._is_mode2_only` (assert đúng 3 cờ mode2); test mới
     `missing_pairs_vet_fields_fail_load`/`pairs_vet_fields_ship_defaults`/
     `pairs_require_vetted_false_loads_ok`.
   - `src/pairbook.rs` — `PairEntry.vetted_at: Option<NaiveDate>` (field
     mới); `parse_vetted_from_comment` (parse `vetted YYYY-MM-DD` từ comment,
     tách khỏi phần địa chỉ); `VetResult` struct mới + `PairBook.vet_results`/
     `vet_failed` (HashSet); `PairBook::contains()` sửa để loại pool trong
     `vet_failed`; `set_vet_result`/`vet_result`/`tokens_to_vet` (API mới);
     `reload`/`reload_if_due` thêm tham số `require_vetted: bool` (gate ngay
     lúc parse, log `pair.unvetted` 1 lần/reload, không tốn RPC cho entry
     chưa vet); `insert_test_entry` set `vetted_at: None` mặc định (chữ ký
     không đổi, không phá call site `pipeline.rs`); 9 test mới + 9 test cũ
     cập nhật tham số `require_vetted`.
   - `src/main.rs` — `pairs_vet_task` (task nền mới, `tokio::spawn` lúc boot):
     chạy ngay + lặp mỗi `pairs_vet_interval_sec`, gọi
     `sim_evm::measure_tax_evm` tuần tự (sleep 200ms/token) cho mọi entry
     `PairBook::tokens_to_vet()`, ghi `PairBook::set_vet_result` + đè
     `state/pairs_vetted.json`, log `pair.vet_fail`/`pair.vet_error`; pair
     reload task đọc `cfg.pairs_require_vetted` thật trước mỗi lần gọi
     `reload_if_due`; test mới
     `ship_config_sim_engine_v2_means_hot_path_never_opens_evm_fork`.
   - `src/web.rs` — `GET /api/pairs` thêm 5 cột: `vetted_at`/`candidate`/
     `buy_bps`/`sell_bps`/`honeypot`/`last_vet_sec_ago` (đọc qua
     `PairBook::vet_result`).
   - `src/pipeline.rs` — **NGOÀI ĐƯỢC ĐỤNG nhưng bắt buộc để compile**: thêm
     ĐÚNG 2 dòng vào fixture `test_config_toml()` (test-only, dùng chung cho
     ~50 test có sẵn trong file) — `pairs_vet_interval_sec = 600`/
     `pairs_require_vetted = false` — vì 2 field mới trong `Config` không có
     `#[serde(default)]` nên MỌI chuỗi TOML dựng `Config::from_str` (kể cả
     fixture test-only) phải có đủ field mới thiếu = fail load. KHÔNG đụng
     bất kỳ dòng logic/thuật toán/decoder nào khác trong file — diff đầy đủ
     dán ở ô 5 (2 dòng, `git diff src/pipeline.rs`).
   - `pairs.txt` — 100 dòng đổi định dạng comment từ `# SYMBOL
     reserve_wbnb~=X BNB` sang `# SYMBOL | vetted | tax  | owner  |
     reserve_wbnb~=X BNB` (`vetted` ĐỂ TRỐNG — Chủ tự điền sau khi vet tay);
     header block thêm giải thích định dạng mới. KHÔNG thêm/bớt token nào
     (đúng CẤM).
   - `scripts/vet_goplus.sh` (MỚI — Chủ chưa kịp copy file có sẵn vào repo
     phiên này, xác nhận qua hỏi lại giữa phiên; Code TỰ VIẾT) — gọi GoPlus
     Security `token_security/56` API công khai, TUẦN TỰ 1 request/token
     (phát hiện thật: batch nhiều địa chỉ trong 1 URL chỉ trả về đúng 1 kết
     quả, các địa chỉ khác bị bỏ qua âm thầm), retry-backoff 5s/10s/20s khi
     gặp `.code != 1` trong JSON body dù HTTP 200 (rate-limit thật, không
     phải lỗi mạng), in bảng PASS/REVIEW/FAIL + tóm tắt số lượng.
   - `scripts/paper_run.sh` — bỏ 3 dòng `apply pair_scan_universal true`/
     `apply scan_quote_usdt true`/`apply sim_engine '"evm"'` (config TẠM giờ
     chỉ ép ngưỡng kinh tế về 0 + `web_port`, giữ nguyên 3 field đó theo
     `config.toml` ship thật); thêm khối in nhanh `pairs.txt` tổng/vetted/
     chưa vet (grep thô, đếm an toàn tránh bug `grep -c` cũ đã ghi trong
     comment file).
   - `docs/VPS_RUN.md` → `docs/RUN.md` (`git mv`) — cập nhật nội dung: bỏ mô
     tả config TẠM ép evm/universal/usdt (giờ giữ nguyên ship), thêm mục
     "Lọc thô `pairs.txt` bằng GoPlus" + ghi chú `sim.evm` sẽ RỖNG khi chạy
     bình thường (đường nóng không mở fork EVM mỗi tx nữa).
   - `docs/DOC_MAP.md` — cập nhật tham chiếu `docs/RUN.md`, thêm 1 dòng mô
     tả.
   - `README.md` — thêm mục "Vet `pairs.txt` (mode 2, pair-mode) — lọc thô +
     chạy paper" trỏ `scripts/vet_goplus.sh` + `docs/RUN.md`.
   - `docs/STATE.md` — mục MỚI `strategy-lock-mode2`: giải thích kỹ thuật vì
     sao V2 math + gas thật đủ cho token đã vet (dẫn chứng
     `real_rpc_sim_evm_matches_sim_v2_when_zero_tax` khớp 0% lệch, BAOCAO31),
     kiến trúc `pairs_vet_task` + lý do đặt gate ở `PairBook::contains()`
     thay vì sửa `pipeline.rs`, gate `pairs_require_vetted` nằm ở
     `PairBook::reload`, 2 phát hiện hạ tầng thật của GoPlus (không batch
     được + rate-limit theo `.code` không theo HTTP status).
   - `docs/TASKS.md` — thêm 1 dòng bảng cụm `strategy-lock-mode2` (XONG,
     BAOCAO36).

4. LỆNH CHẠY:
   ```
   cargo build --release
   cargo test
   cargo test --lib
   cargo test pairbook
   git add -A && git commit -m "..."
   git log -1 --format="%H %ci"
   sha256sum target/release/bsc_sandwich
   bash -n scripts/vet_goplus.sh
   bash -n scripts/paper_run.sh
   scripts/vet_goplus.sh pairs.txt
   scripts/paper_run.sh --minutes 5 --port 18799
   ```

5. OUTPUT THẬT (máy: **WSL**, `/home/dmin/bsc-sandwich`, ext4 native — mọi
   khối dưới đây chạy SAU commit cuối, `git HEAD` + `sha256` binary GHI KÈM
   TRỰC TIẾP trong output của `scripts/paper_run.sh` tự in ra, không phải số
   Code tự chép tay):

   ```
   $ cargo build --release
   Finished `release` profile [optimized] target(s) in 16.97s (0 warning)
   ```

   ```
   $ cargo test
   ...
   test result: ok. 280 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 0.17s

        Running unittests src/main.rs (target/debug/deps/bsc_sandwich-f468d988bf49fab9)
   running 15 tests
   test tests::halt_transition_false_to_true_is_triggered ... ok
   test tests::halt_transition_true_to_false_is_cleared ... ok
   test tests::halt_transition_unchanged_is_none ... ok
   test tests::passes_router_gate_false_for_none_when_source_is_ws_or_txpool ... ok
   test tests::passes_router_gate_rejects_fake_router_even_with_real_v2_selector ... ok
   test tests::passes_router_gate_true_for_none_only_when_source_is_inject ... ok
   test tests::passes_router_gate_true_for_pinned_router_false_for_others ... ok
   test tests::record_funnel_terminal_simulated_increments_simulated_bucket ... ok
   test tests::record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket ... ok
   test tests::record_funnel_terminal_venue_unpinned_increments_venue_v3 ... ok
   test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok
   test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok
   test tests::ship_config_sim_engine_v2_means_hot_path_never_opens_evm_fork ... ok
   test tests::poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate ... ok
   test tests::halt_watch_task_logs_triggered_then_cleared_exactly_once_each ... ok

   test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
   ```
   (280+15=295 passed, 0 failed, 11 ignored — trước phiên này 269+14=283,
   TĂNG 12 test mới ròng: 9 `pairbook` + 3 `config` + 1 `main.rs` mới, TRỪ 0
   test xoá. 11 ignored KHÔNG đổi = không có `real_rpc_*` nào phải chạy thêm
   cụm này, đúng phạm vi lệnh "không đổi thuật toán".)

   ```
   $ cargo test pairbook 2>&1 | tail -18
   test pairbook::tests::parse_vetted_from_comment_bad_date_format_is_none ... ok
   test pairbook::tests::parse_vetted_from_comment_empty_vetted_field_is_none ... ok
   test pairbook::tests::parse_vetted_from_comment_old_format_no_pipe_is_none ... ok
   test pairbook::tests::parse_vetted_from_comment_valid_date ... ok
   test pairbook::tests::set_vet_result_false_removes_from_contains_true_restores ... ok
   test pairbook::tests::pairbook_garbage_line_and_comment_skipped_no_panic ... ok
   test pairbook::tests::pairbook_load_direct_addr ... ok
   test pairbook::tests::pairbook_load_token_resolve_mock ... ok
   test pairbook::tests::pairbook_resolve_rpc_error_is_skipped_not_crash ... ok
   test pairbook::tests::pairbook_wrong_second_column_is_parse_error_skipped ... ok
   test pairbook::tests::reload_require_vetted_false_keeps_all_resolved_entries ... ok
   test pairbook::tests::reload_require_vetted_true_keeps_only_dated_entry ... ok
   test pairbook::tests::tokens_to_vet_only_includes_vetted_token_entries_not_direct ... ok
   test pairbook::tests::pairbook_real_file_format_with_trailing_comment_parses_exactly_1_pool ... ok
   test pairbook::tests::reload_if_due_respects_interval_with_injected_clock ... ok

   test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 276 filtered out; finished in 0.01s
   ```

   **`scripts/vet_goplus.sh pairs.txt` — CHẠY THẬT (WSL, mạng thật ra
   GoPlus, không mock):**
   ```
   == vet_goplus.sh: 100 token tu 'pairs.txt', goi GoPlus token_security chain 56 ==
   ... (100 dong bang PASS/REVIEW/FAIL, 9 dong rate_limit_or_network trong so do) ...
   == TOM TAT: tong=100 PASS=30 REVIEW=41 FAIL=20 (loi_goi_API=9) ==
   ```
   Vài dòng mẫu thật (rút gọn từ log đầy đủ):
   ```
   0x55d398326f99059ff775485246999027b3197955   REVIEW     USDT    mintable,owner_not_renounced
   0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82   REVIEW     Cake    mintable,owner_not_renounced
   0xc748673057861a797275cd8a068abb95a902e8de   REVIEW     BabyDoge anti_whale
   0x924fa68a0fc644485b8df8abfa0a41c2e7744444   FAIL       币安人生 transfer_pausable
   0x92aa03137385f18539301349dcfc9ebc923ffb10   PASS       SKYAI   ok
   0x08ba0619b1e7a582e0bce5bbe9843322c954c340   FAIL       BMON    trading_cooldown
   0x5ac52ee5b2a633895292ff6d8a89bb9190451587   FAIL       BSCX    can_take_back_ownership,hidden_owner
   0x1633b7157e7638c4d6593436111bf125ee74703f   REVIEW     ?       khong goi duoc GoPlus (rate_limit_or_network)
   ```
   **PHÁT HIỆN THẬT (ghi vào `docs/STATE.md`, không đoán trước):** GoPlus
   `token_security` public API (a) KHÔNG hỗ trợ batch thật — nhiều
   `contract_addresses` phẩy-cách trong 1 URL chỉ trả về đúng 1 kết quả (địa
   chỉ đầu), (b) rate-limit rất chặt theo burst, trả `HTTP 200` nhưng body
   `{"code":4029}` (không phải lỗi mạng) — 9/100 token không lấy được dữ liệu
   THẬT sau 3 lần retry (35s tổng cộng mỗi token) trong lần chạy này, ghi
   `REVIEW` (KHÔNG đoán `PASS`) — đúng thiết kế an toàn, không phải bug.

   **`scripts/paper_run.sh --minutes 5 --port 18799` — CHẠY THẬT, RPC thật
   từ `.env` (`BSC_HTTP`/`BSC_WS` publicnode), mempool BSC sống:**
   ```
   == may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
   BSC_WS host = bsc-rpc.publicnode.com,wss (KHONG in URL day du)
     -> khop publicnode, OK
   == cargo build --release ==
       Finished `release` profile [optimized] target(s) in 0.26s
   == binary sha256 = adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747 ==
   == git HEAD = 5675f8133a8da90313b3636a3af9fcbead24ffeb ==
   da xoa state/halt.lock (neu co)
   == config TAM (chi nguong ve 0 + web_port, GIU NGUYEN sim_engine/pair_scan_universal/scan_quote_usdt tu config.toml that), port 18799 ==
   == pairs.txt (grep tho, xem sim.evm/pair.unvetted trong log de co so that): tong=100 vetted=0 chua_vet=100 ==
   == chay bot 5 phut, log -> logs/paper_run_1789460950.log (bot.jsonl tu dong 70716) ==
   PID=1310030
   ======== KET QUA SAU 5 PHUT (may: WSL, binary sha256: adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747, git HEAD: 5675f8133a8da90313b3636a3af9fcbead24ffeb) ========
   ---- /api/skips ----
   {"below_min":0,"deadline":0,"decode_fail":2086,"honeypot_or_tax":0,"hooks_unread":0,"no_pool":0,"nonce_future":0,"nonce_stale":0,"not_in_list":322,"not_pancake_router":0,"not_quote_pair":0,"not_wbnb_pair":1253,"sell_direction":0,"sim_error":0,"thin_liq":0,"unprofitable":0,"venue_unpinned":2,"victim_would_revert":0}
   ---- /api/funnel ----
   {"below_min":0,"deadline":0,"decode_fail":388,"honeypot_or_tax":0,"no_pool":0,"nonce_future":0,"nonce_stale":0,"not_pancake_router":15068,"not_wbnb_pair":249,"rpc_error":0,"seen":15769,"sim_error":0,"simulated":0,"thin_liq":0,"unprofitable":0,"venue_v2":63,"venue_v3":2,"victim_would_revert":0}
   ---- /api/tax ----
   {"allow_tax_inject":true,"current_block":121998504,"entries":[],"tax_cache_blocks":30,"tax_cache_ttl_sec":600}
   ---- /api/validate ----
   {"rows":[],"total":0,"within_1pct":0,"within_1pct_ratio":0.0}
   ---- dem Simulated (sim.evm decision=simulated, lan chay nay) ----
   0
   == halt bot (ghi state/halt.lock, cho halt.triggered toi da 10s, roi kill PID) ==
   halt.triggered count (lan chay nay) = 1
   tx.seen sau halt.triggered (lan chay nay) = 0 (ky vong 0)
   tx.build=0 simulated=0 build.refused=0 halt.triggered=1
   DONE. Log day du: logs/paper_run_1789460950.log (redact secret truoc khi dan cho Grok).
   Nho dan lai: may=WSL, binary sha256=adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747, git HEAD=5675f8133a8da90313b3636a3af9fcbead24ffeb (luat #2 CLAUDE.md).
   ```
   **ĐÚNG DOD lệnh nhận:** `pairs.txt` `tổng=100 vetted=0` → `candidate=0`
   (`PairBook.count()` thực tế = 0, verify qua `GET /api/pairs` giữa lúc bot
   chạy: `{"count":0,"error_lines":0,"pairs":[]}`, không đợi tới cuối log);
   `not_in_list=322 > 0` (tx thật khớp gate V2 nhưng `PairBook` rỗng nên rơi
   `not_in_list`, ĐÚNG hành vi thiết kế: mode 1 tắt, mode 2 chưa ai vet, mode
   3 tắt); `sim.*` (đếm `sim.evm decision=simulated`) `=0`; `unprofitable=0`,
   `victim_would_revert=0`, `sim_error=0` (đường nóng không mở EVM nên các
   reason đó không phát sinh — khác hẳn phiên trước dùng `sim_engine=evm`).
   `seen≈15-17k tx/phút` thật trên mempool BSC.

   ```
   $ git status --short
   (rỗng)
   $ git log -1 --format="%H %ci"
   5675f8133a8da90313b3636a3af9fcbead24ffeb 2026-09-15 15:28:56 +0700
   $ sha256sum target/release/bsc_sandwich
   adc6360708f69750e77ff0b41c6167ae4e2c7183cfb6b6062281e5871ca44747  target/release/bsc_sandwich
   ```

6. CHAIN: `0x38` xác nhận thật (WSL, `.env` RPC thật, `eth_chainId`):
   ```
   $ curl -sS -X POST -H "Content-Type: application/json" \
       --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
       https://bsc-dataseed1.bnbchain.org
   {"jsonrpc":"2.0","id":1,"result":"0x38"}
   ```
   `current_block` trong `/api/tax` lúc chạy paper: `121998504` (block thật
   lúc chạy, không MISSING).

7. REGISTRY: KHÔNG đổi cụm này (không đụng `venues.rs`/`DEX_REGISTRY.md`,
   ngoài phạm vi lệnh). V2/V3/V4-Infinity vẫn pin như BAOCAO02, `venue_v2=63
   venue_v3=2` xuất hiện thật trong funnel phiên này (đúng registry đã pin).

8. KHÔNG LÀM (đúng CẤM/NGOÀI PHẠM VI lệnh):
   - Gas thật (`eth_gasPrice` × gas đo) — cụm `real-economics-mode2` sau.
   - Histogram `victim_in`, validator non-isolated — cụm `real-economics-mode2`.
   - Decoder (`decoder.rs` KHÔNG đụng), relay, VPS deploy.
   - Thêm/bớt token trong `pairs.txt` (100 dòng GIỮ NGUYÊN, chỉ đổi format
     comment).
   - Tự điền `vetted` cho bất kỳ dòng nào (kể cả 30 token PASS ở
     `vet_goplus.sh` — đó là lọc THÔ, Chủ phải tự soát tay + tự điền).
   - Bật live/`dry_run=false`/`bot_armed`, gửi tx thật.
   - Đổi stack, nhảy cụm.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
    - **`pairs.txt` chưa có dòng nào `vetted`** — Chủ cần tự vet tay 100
      token (tham khảo bảng `vet_goplus.sh`: 30 PASS lọc thô, 41 REVIEW cần
      đọc kỹ, 20 FAIL nên cân nhắc xoá khỏi `pairs.txt`) rồi điền ngày vào
      đúng field `vetted YYYY-MM-DD` — bot mới bắt đầu sim.
    - `pairs_vet_task` (`main.rs`) chưa từng chạy thật với `vetted_at=Some`
      nào (vì `pairs.txt` hiện `vetted=0`) — cơ chế đã có test unit đầy đủ
      (`pairbook.rs`) nhưng CHƯA verify end-to-end trên mempool sống với ít
      nhất 1 token đã vet (cần Chủ điền ít nhất 1 dòng `vetted` để kiểm).
    - `state/pairs_vetted.json` CHƯA từng được ghi (điều kiện: có ít nhất 1
      token `vetted_at=Some` + có provider + có `last_block>0`) — chưa có
      dịp verify format file thật.
    - `PairBook::vet_failed`/`set_vet_result` verify bằng UNIT TEST
      (`pairbook.rs`), CHƯA verify bằng RPC thật (cần ít nhất 1 token đã vet
      + `measure_tax_evm` thật chạy trong `pairs_vet_task` — phụ thuộc mục
      trên).
    - `real-economics-mode2` (gas thật `eth_gasPrice`, histogram, validator
      non-isolated) — CHƯA làm, đúng roadmap (cụm kế tiếp).
    - `fork-actor-perf` — hạ ưu tiên xuống sau cụm 6, đúng lệnh.
    - `scripts/vet_goplus.sh`: 9/100 token bị rate-limit không lấy được dữ
      liệu thật (ghi REVIEW, không phải PASS/FAIL) — Chủ nên chạy lại script
      riêng cho 9 token đó sau khi rate-limit GoPlus tự hồi phục (không có
      trong lệnh này, script đã roundtrip tuần tự toàn bộ 100 token 1 lần
      theo đúng yêu cầu "chạy thật 1 lần").
