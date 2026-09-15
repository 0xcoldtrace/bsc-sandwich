## BAOCAO34

1. LÁT: `wsl-env-rules-paperrun` — xác nhận môi trường WSL + build/test xanh
   + ghi toolchain vào `docs/STATE.md` + thêm 3 luật vào `CLAUDE.md` + tạo
   `scripts/paper_run.sh` dùng chung WSL/VPS (thay `scripts/vps_paper_run.sh`,
   file cũ giữ lại làm alias tương thích ngược).

2. LỆNH NHẬN: (chủ ra lệnh trực tiếp, không qua khối lệnh Grok)
   - Xác nhận dự án ở `~/bsc-sandwich` trong WSL (không phải `/mnt/c`), Claude
     Code chạy từ đó.
   - `cargo build --release`, `cargo test` xanh trong WSL; ghi version
     toolchain vào `STATE.md`.
   - Sửa `CLAUDE.md` thêm 3 luật mới: (1) mỗi cụm kết thúc bằng git commit,
     BAOCAO ghi hash; (2) mọi số liệu runtime phải ghi rõ chạy ở đâu
     (WSL/VPS) và hash binary; (3) `real_rpc_*` không được "pass rỗng".
   - Tạo `scripts/paper_run.sh` dùng chung cho WSL và VPS (bản
     `vps_paper_run.sh` hiện tại bỏ phần SSH).

3. FILE ĐỔI:
   - `CLAUDE.md` — thêm mục "3 luật bổ sung (chủ ra lệnh 2026-09-15)" ngay
     sau `0.ANTI`.
   - `docs/STATE.md` — thêm mục "Toolchain + môi trường build" (rustc/cargo
     version, kết quả build/test, sha256 binary, git HEAD lúc test).
   - `scripts/paper_run.sh` (MỚI) — script paper-run dùng chung WSL/VPS,
     copy logic từ `vps_paper_run.sh` cũ, thêm tự phát hiện `WSL`/`VPS` qua
     `/proc/version` + in `sha256sum` binary + git HEAD ngay đầu phần kết
     quả (đúng luật #2 mới).
   - `scripts/vps_paper_run.sh` — rút gọn thành alias `exec
     scripts/paper_run.sh "$@"` (giữ tương thích ngược cho
     `docs/VPS_RUN.md`/`baocao/BAOCAO33.md` đã trỏ tới tên file này).
   - `docs/VPS_RUN.md` — cập nhật trỏ sang `scripts/paper_run.sh` là script
     chính, ghi chú `vps_paper_run.sh` giờ chỉ forward.
   - Ghi chú: KHÔNG đụng `victims.txt` thật, `.env`, cờ live, `config.toml`
     thật.

4. LỆNH CHẠY:
   ```
   rustc --version && cargo --version
   cargo build --release
   cargo test --release
   sha256sum target/release/bsc_sandwich
   git log -1 --format="%H %ci"
   bash -n scripts/paper_run.sh && bash -n scripts/vps_paper_run.sh
   timeout 90 scripts/paper_run.sh --minutes 0 --port 18799   # smoke test
   ```

5. OUTPUT THẬT (máy: **WSL**, `/home/dmin/bsc-sandwich`, ext4 native, không
   qua `/mnt/c`; binary sha256 =
   `e45c804ac7ecff2a867d7b235e876d50d28a4af4becf9c7fef27f88496426bd8`; git
   HEAD lúc chạy = `251689766dd9d89c406363b1ad8024833ef2e49d`):

   ```
   rustc 1.97.1 (8bab26f4f 2026-07-14)
   cargo 1.97.1 (c980f4866 2026-06-30)
   ```

   ```
   $ cargo build --release
   ...
   Compiling bsc_sandwich v0.1.0 (/home/dmin/bsc-sandwich)
   Finished `release` profile [optimized] target(s) in 1m 14s
   ```

   ```
   $ cargo test --release
   ...
   test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
   test venues::tests::every_pinned_contract_has_nonzero_get_code_len ... ok
   test transport::tests::placeholder_url_fails_fast_no_panic ... ok
   test transport::tests::vps_fallback_missing_file_is_empty_not_panic ... ok
   test venues::tests::scan_and_live_flags_pass_through_unchanged ... ok
   test venues::tests::skip_reasons_contains_not_pancake_router_and_sell_direction ... ok
   test venues::tests::newer_family_still_disabled_not_deleted ... ok
   test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
   test venues::tests::usdt_pinned_and_nonzero ... ok
   test victims::tests::bnb_to_wei_basic ... ok
   test victims::tests::duplicate_address_last_line_wins ... ok
   test victims::tests::checksum_and_lowercase_same_wallet ... ok
   test victims::tests::reload_respects_interval_with_injected_clock ... ok
   test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok

   test result: ok. 246 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out; finished in 0.08s

        Running unittests src/main.rs
   running 9 tests
   test tests::passes_router_gate_true_for_none_unknown_source ... ok
   test tests::passes_router_gate_true_for_pinned_router_false_for_others ... ok
   test tests::record_funnel_terminal_maps_each_terminal_skip_to_its_own_bucket ... ok
   test tests::record_funnel_terminal_simulated_increments_simulated_bucket ... ok
   test tests::poll_prefilter_1000_fake_tx_5_to_v2_router_exactly_5_pass_gate ... ok

   test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```
   (10 ignored = toàn bộ `real_rpc_*`, đúng luật #3 mới — CHƯA chạy nhóm
   này phiên này vì lệnh chỉ yêu cầu build/test thường xanh, không yêu cầu
   verify RPC thật; ghi CÒN NỢ nếu Grok muốn chạy `--ignored` phiên sau.)

   Smoke test `scripts/paper_run.sh --minutes 0 --port 18799` (dùng `.env`
   thật của chủ, `BSC_WS` host xác nhận `publicnode`, KHÔNG in URL):
   ```
   == may chay: WSL (repo: /home/dmin/bsc-sandwich) ==
   BSC_WS host = bsc-rpc.publicnode.com,wss (KHONG in URL day du)
     -> khop publicnode, OK
   == cargo build --release ==
   Finished `release` profile [optimized] target(s) in 0.29s
   == binary sha256 = e45c804ac7ecff2a867d7b235e876d50d28a4af4becf9c7fef27f88496426bd8 ==
   == git HEAD = 251689766dd9d89c406363b1ad8024833ef2e49d ==
   da xoa state/halt.lock (neu co)
   == config TAM (nguong 0 + universal + USDT + sim_engine=evm), port 18799 ==
   == chay bot 0 phut, log -> logs/paper_run_1789452374.log ==
   PID=1208769
   ======== KET QUA SAU 0 PHUT (may: WSL, binary sha256: e45c804a..., git HEAD: 2516897...) ========
   ---- /api/skips ----
   {"below_min":0,"deadline":0,"decode_fail":0,...}
   ---- /api/tax ----
   {"allow_tax_inject":true,"current_block":0,"entries":[],...}
   ---- /api/validate ----
   {"rows":[],"total":0,"within_1pct":0,"within_1pct_ratio":0.0}
   == halt bot (ghi state/halt.lock + kill PID) ==
   DONE. Log day du: logs/paper_run_1789452374.log (redact secret truoc khi dan cho Grok).
   ```
   Đã dọn `state/halt.lock`, `state/paper_run.pid`, log tạm sau smoke test
   (không phải paper run thật 30 phút, chỉ để chứng minh script chạy được
   end-to-end — boot thật, gọi RPC thật qua `.env`, gọi API thật, halt sạch).

6. CHAIN: `0x38` — `chain_id=56` xác nhận qua `connect_and_verify` lúc boot
   smoke test (không có log lỗi chain mismatch); không gọi `getCode` mới
   phiên này (không đụng registry).

7. REGISTRY: không đổi (đã pin từ phiên `1.1+1.2+1.3`, xem `DEX_REGISTRY.md`).

8. KHÔNG LÀM: không bật live/`dry_run=false`/`bot_armed`; không sửa
   `victims.txt` thật/`.env`/`config.toml` thật; không chạy `real_rpc_*`
   (`--ignored`); không đổi stack; không xoá `scripts/vps_paper_run.sh`
   (giữ alias tương thích ngược thay vì xoá).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
    - Chưa chạy nhóm test `real_rpc_*` (10 test `#[ignore]`) phiên này —
      lệnh chỉ yêu cầu build/test thường xanh. Nếu Grok muốn dùng làm bằng
      chứng cho luật #3 mới, cần 1 lệnh riêng chạy `--ignored --nocapture`
      và dán output thật.
    - Chưa chạy `scripts/paper_run.sh` bản đủ 30 phút thật (chỉ smoke test
      0 phút để verify cơ chế) — số liệu funnel/skip/tax 30 phút thật cần
      lệnh riêng nếu Grok cần.
    - `docs/TASKS.md` chưa cập nhật mục này (không thuộc phạm vi lệnh, chưa
      đụng).
