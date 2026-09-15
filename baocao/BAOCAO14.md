1. LÁT: Bỏ hoàn toàn chiều victim BÁN token khỏi pipeline — `decide_paper_v2`
chỉ còn xử lý chiều victim MUA token (swap WBNB/BNB -> token). Dọn code chết
liên quan + cập nhật docs cùng phiên (một cụm dính liền).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml,
      baocao/BAOCAO13.md.

LÁT: Bỏ hoàn toàn chiều victim BÁN token khỏi pipeline. decide_paper_v2 chỉ còn
     xử lý chiều victim MUA token (swap WBNB/BNB -> token). Dọn code chết liên
     quan, cập nhật test/docs dính cụm này trong cùng phiên.
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/pipeline.rs, src/sim_v2.rs, docs/STATE.md, docs/TASKS.md,
           baocao/BAOCAO14.md

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, bật cờ
     live/dry_run=false/bot_armed, sendRaw bất kỳ, đổi pair khỏi WBNB, thêm
     lại logic chiều bán dưới bất kỳ hình thức nào (kể cả bản "luôn trả
     Unprofitable an toàn").
```

3. FILE ĐỔI:
- `src/pipeline.rs`:
  - Xoá enum `SwapDirection` (`VictimBuy`/`VictimSell`) hoàn toàn.
  - `decode_and_classify` rút gọn: bỏ nhánh phân loại chiều, chỉ còn 1 điều
    kiện `decoded.path.token_a != wbnb() -> not_wbnb_pair` (giống hệt nhánh
    tương ứng trong `decode_and_prefilter`). Trả `(DecodedSwap, Address)`
    thay vì `(DecodedSwap, Address, SwapDirection)`.
  - `precheck_token_only` sửa theo chữ ký mới của `decode_and_classify`.
  - `evaluate_candidate` bỏ tham số `direction: SwapDirection`, bỏ `match`
    (trước đó route `VictimBuy -> search_max_front_in` /
    `VictimSell -> search_max_front_in_sell`) — giờ gọi thẳng
    `sim_v2::search_max_front_in`.
  - `decide_paper_v2` (2 điểm gọi `evaluate_candidate` — nhánh wallet và
    nhánh pair) bỏ truyền `direction`.
  - Sửa doc-comment đầu file + doc-comment `evaluate_candidate` (không còn
    nhắc "chưa có model chiều bán" — giờ ghi rõ "đã bị bỏ hẳn, xem
    docs/STATE.md").
  - Xoá test `decide_paper_sell_direction` + helper `build_tokens_for_eth`
    (chỉ tồn tại để phục vụ test đó, không còn nơi nào khác gọi — đã grep
    xác nhận trước khi xoá).
  - THÊM test mới `decide_paper_v2_sell_direction_is_not_wbnb_pair`: dựng
    calldata `swapExactTokensForETH` (path=[token,WBNB], chiều bán) gọi thẳng
    `decide_paper_v2`, xác nhận LUÔN `Skip(NotWbnbPair)` + `source="none"` dù
    `from` có trong `victims.txt` — chứng minh việc bỏ chiều bán có hiệu lực
    THẬT ở entry point (không chỉ xoá code không dùng), theo đúng ý "test
    cover chung decide_paper_v2 (chiều mua) phải còn nguyên và pass" mở rộng
    thêm 1 test xác nhận đường bán bị chặn.
  - `decide_paper` (bản gốc, wallet-mode `4.1`/`5.1`+) và `decode_and_prefilter`
    KHÔNG bị đụng — không sửa 1 dòng.
- `src/sim_v2.rs`:
  - Xoá `simulate_front_then_victim_sell` (public, cùng doc-comment dài giải
    thích phát hiện toán học).
  - Xoá `quote_at_sell` (private).
  - Xoá `search_max_front_in_sell` (public, cùng doc-comment).
  - Xoá 3 test: `sell_direction_hand_verified_numbers_profit_is_negative`,
    `sell_direction_larger_victim_amount_makes_loss_worse`,
    `search_max_front_in_sell_converges_near_zero_not_at_cap`.
  - Đã grep toàn repo xác nhận 3 hàm trên KHÔNG còn được gọi ở bất kỳ file
    nào khác trước khi xoá (chỉ `pipeline.rs` gọi `search_max_front_in_sell`,
    đã sửa cùng phiên).
  - `get_amount_out`/`simulate_front_then_victim`/`search_max_front_in`/
    `victim_still_ok` (chiều mua, dùng bởi `decide_paper` gốc) KHÔNG bị đụng.
- `docs/STATE.md`: thêm mục "QUYẾT ĐỊNH — chiều victim bán, phiên BAOCAO14:
  BỎ HẲN, không sim, không code" (cuối file) — ghi rõ lý do (thứ tự lệnh gốc
  luôn lỗ về toán AMM đã chứng minh ở BAOCAO13; hướng fix đúng là kỹ thuật
  "back-run" — front bán trước/back mua lại sau — nhưng đòi hỏi tồn kho
  token/flashloan, NGOÀI SCOPE "1 signer, cấm flashloan" của CLAUDE.md),
  liệt kê đủ danh sách đã xoá, ghi rõ cấm code lại dưới bất kỳ hình thức nào.
- `docs/TASKS.md`:
  - Dòng "Pair-mode" đổi mô tả: bỏ nhắc "victim sell direction" trong tên cụm
    (không còn tồn tại), ghi rõ "ĐÃ BỎ HẲN khỏi pipeline ở BAOCAO14 (không
    phải giữ nguyên/DISABLED)", thêm `BAOCAO14` vào cột Phiên.
  - Mục nợ "Cụm pair-mode/sell-direction (BAOCAO13) — CÒN NỢ cần Grok quyết
    định" đổi thành "ĐÃ ĐÓNG ở BAOCAO14" — ghi kết quả BỎ, không phải "giữ
    nguyên", trỏ sang mục quyết định mới trong `docs/STATE.md`.
  - Mục nợ "Nợ đơn vị `evaluate_candidate` (BAOCAO13)" đánh dấu ĐÃ HẾT ÁP
    DỤNG (chiều bán — nguồn gây lệch đơn vị — không còn tồn tại).
- `baocao/BAOCAO14.md` — MỚI (file này).

KHÔNG ĐỤNG: `CLAUDE.md`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md`,
`config.toml`, `src/main.rs`, `src/web.rs`, `src/executor.rs`,
`src/pairbook.rs`, `src/config.rs` — không cần sửa vì chữ ký `decide_paper_v2`
(tham số vào/ra) không đổi, chỉ nội bộ hàm private (`evaluate_candidate`,
`decode_and_classify`) và `sim_v2.rs` bị sửa.

4. LỆNH CHẠY:
```
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -5
```

5. OUTPUT THẬT:
```
$ cargo test 2>&1 | tail -25
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 155 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.08s

     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-ccc2de570b43de27.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\bin\rpc_probe.rs (target\debug\deps\rpc_probe-2c9d69c9c0cabe6c.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bsc_sandwich

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
```
$ cargo build --release 2>&1 | tail -5
    Finished `release` profile [optimized] target(s) in 0.47s
```

Đối chiếu số lượng test: BAOCAO13 có 158 passed. Phiên này xoá 4 test (3 test
sell-direction ở `sim_v2.rs` + 1 test `decide_paper_sell_direction` ở
`pipeline.rs`), thêm 1 test mới (`decide_paper_v2_sell_direction_is_not_wbnb_pair`)
→ 158 - 4 + 1 = 155, khớp đúng số `cargo test` in ra ở trên. Không có
`FAILED` nào, 2 `ignored` giữ nguyên (test RPC thật `#[ignore]` từ trước,
không đổi phiên này).

Danh sách toàn bộ test trong `pipeline.rs`/`sim_v2.rs` sau khi sửa (chạy lọc
riêng để xác nhận không sót/không lẫn):
```
$ cargo test 2>&1 | grep -E "^test (pipeline|sim_v2)::tests"
test pipeline::tests::address_not_in_victims_txt_is_not_in_list ... ok
test pipeline::tests::decide_paper_v2_sell_direction_is_not_wbnb_pair ... ok
test pipeline::tests::decide_paper_v2_neither_match_is_not_in_list ... ok
test pipeline::tests::precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason ... ok
test pipeline::tests::honeypot_or_tax_skip_when_measured_tax_exceeds_max_roundtrip_tax_config ... ok
test pipeline::tests::thin_liq_skip_when_pool_reserve_below_min_reserve_config ... ok
test pipeline::tests::victim_a_passes_min_size_but_skips_honeypot_or_tax_when_unmeasured ... ok
test pipeline::tests::decide_paper_v2_wallet_wins_over_pair_when_both_match ... ok
test pipeline::tests::victim_b_below_min_is_skipped_with_clear_reason ... ok
test pipeline::tests::decide_paper_pair_source ... ok
test sim_v2::tests::get_amount_out_zero_input_or_reserve_is_none ... ok
test sim_v2::tests::get_amount_out_exact_fixture_reserves ... ok
test pipeline::tests::decide_paper_wallet_source ... ok
test sim_v2::tests::hand_verified_full_sandwich_numbers ... ok
test sim_v2::tests::victim_still_ok_pass_and_revert ... ok
test pipeline::tests::max_exposure_bnb_zero_disables_cap_front_can_exceed_5_bnb ... ok
test pipeline::tests::changing_config_between_two_decide_paper_calls_changes_front_cap ... ok
test pipeline::tests::max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb ... ok
test pipeline::tests::unprofitable_skip_when_profit_positive_but_below_min_profit_bnb_config ... ok
test pipeline::tests::victim_a_reaches_sim_when_tax_cache_injected_zero ... ok
test sim_v2::tests::search_result_front_in_always_within_bounds_even_with_large_max ... ok
test pipeline::tests::victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero ... ok
test pipeline::tests::log_outcome_writes_expected_events ... ok
test sim_v2::tests::search_never_exceeds_max_front_bnb ... ok
```
Không còn `search_max_front_in_sell`/`sell_direction_*`/`SwapDirection` nào
trong danh sách trên (đúng "dọn code chết" của lệnh). `cargo build --release`
không có `warning` nào (đã kiểm `| grep -i warn` rỗng) — xác nhận không sót
hàm/biến chết nào bị compiler cảnh báo.

6. CHAIN: Không đụng chain/pin phiên này (không sửa `DEX_REGISTRY.md`/
`src/venues.rs`). `chain_id: 56` không đổi. `eth_getCode`: MISSING (không
cần cho cụm này — không có contract mới nào cần pin, đây là cụm dọn logic
thuần Rust, không chạm on-chain).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (không
nằm trong phạm vi ĐƯỢC ĐỤNG của lệnh này).

8. KHÔNG LÀM:
- Không giữ lại bất kỳ code/nhánh xử lý chiều bán dưới bất kỳ hình thức nào
  (kể cả bản "luôn trả `Unprofitable` an toàn" — đã xoá thẳng, không viết
  stub).
- Không `sendRaw`/bật `allow_live`/`dry_run=false`/`bot_armed` — không đụng
  cờ nào trong `config.toml` phiên này (file không nằm trong ĐƯỢC ĐỤNG).
- Không đổi pair khỏi WBNB — `decode_and_classify` vẫn bắt buộc
  `token_a == WBNB` (WBNB đã pin, không đổi địa chỉ).
- Không sửa `CLAUDE.md`/`DEX_REGISTRY.md`/`victims.txt` thật/`.env`.
- Không nhảy sang `7.2`/`7.3` — phạm vi phiên này chỉ dọn `decide_paper_v2`/
  `sim_v2.rs`.
- Không tách test/docs ra phiên sau — test mới + `docs/STATE.md`/
  `docs/TASKS.md` đã cập nhật cùng phiên này.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- Không có nợ mới phát sinh từ phiên này. Quyết định chiều bán đã ĐÓNG dứt
  điểm (xem `docs/STATE.md`) — phiên sau không cần hỏi lại hay code lại nhánh
  này trừ khi có lệnh Grok mới đổi kiến trúc (back-run + tồn kho token/
  flashloan, ngoài scope hiện tại).
- Các nợ khác của BAOCAO13 không liên quan cụm này (RiskGuard::record_result
  chưa gọi tự động, 7.2/7.3 chưa làm, deploy script chưa chạy thật lên VPS
  mới) giữ nguyên trạng thái CÒN NỢ như `docs/TASKS.md` đã ghi — không thuộc
  phạm vi lệnh phiên này.
