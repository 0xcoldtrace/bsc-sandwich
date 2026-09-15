# BAOCAO20

## 1. LÁT

`universal-pair-scan` — thêm chế độ quét MỌI pool WBNB (không chỉ pool khai
trong `pairs.txt`) qua field mới `config.toml::pair_scan_universal` (bắt
buộc, ship `false`). Một cụm, không cắt vụn.

## 2. LỆNH NHẬN

```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, config.toml, baocao/BAOCAO19.md,
      src/pipeline.rs (hàm decide_paper_v2), src/config.rs.

LÁT: universal-pair-scan — thêm chế độ quét MỌI pool WBNB thay vì chỉ pool
     khai trong pairs.txt. Thêm field config.toml MỚI
     `pair_scan_universal: bool` (ship default `false` — AN TOÀN, Chủ phải tự
     bật). Khi `true`: trong `decide_paper_v2`, nếu KHÔNG khớp wallet-mode
     VÀ KHÔNG khớp `PairBook` (pair chưa liệt kê pairs.txt) → thay vì
     `not_in_list`, coi là candidate với `source="universal"`, dùng NGƯỠNG
     GLOBAL đã có sẵn `pairs_min_swap_bnb` (KHÔNG thêm ngưỡng riêng). Khi
     `false` (default) → hành vi giữ NGUYÊN y hệt hiện tại (not_in_list),
     không phá test cũ nào. Thứ tự ưu tiên GIỮ NGUYÊN: wallet > pair (đã
     liệt kê) > universal (mới) > not_in_list.
...
(đầy đủ ở khối lệnh Grok gửi, không rút gọn nội dung LÀM/KHÔNG LÀM/CẤM)
```

## 3. FILE ĐỔI

- `src/config.rs` — thêm field bắt buộc `Config::pair_scan_universal: bool`
  (doc-comment giải thích), cập nhật `base_toml()` (fixture test) + 3 test mới:
  `missing_pair_scan_universal_fails`, `pair_scan_universal_ship_default_is_false`,
  `pair_scan_universal_true_loads_ok`.
- `src/pipeline.rs` — `decide_paper_v2` thêm nhánh thứ 3 (universal, sau pair,
  trước `not_in_list`), dùng chung `evaluate_candidate` + `cfg.pairs_min_swap_wei()`
  (không thêm ngưỡng riêng, không thêm `eth_call`). Cập nhật doc-comment hàm.
  Cập nhật `test_config_toml()` (fixture test) + 4 test mới:
  `universal_scan_disabled_by_default_still_not_in_list`,
  `universal_scan_enabled_unlisted_pool_becomes_candidate`,
  `universal_scan_enabled_below_threshold_is_below_min`,
  `universal_scan_enabled_wallet_still_wins`.
- `config.toml` — thêm field `pair_scan_universal = false` (cuối file, sau
  `pairs_min_swap_bnb`) kèm comment giải thích.
- `docs/STATE.md` — thêm mục `## universal-pair-scan` ghi quyết định kỹ
  thuật + lý do không tăng tải RPC (đã đọc kỹ trước khi sửa file này, đúng
  ĐƯỢC ĐỤNG).
- `docs/TASKS.md` — thêm 1 dòng bảng roadmap cho cụm này (XONG, BAOCAO20).
- `baocao/BAOCAO20.md` — file này.

KHÔNG đụng: `CLAUDE.md`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md`,
`executor.rs`, `calldata.rs`, `pool.rs`, không đổi cờ live nào, không thêm
crate mới trong `Cargo.toml`.

## 4. LỆNH CHẠY

```
cargo build --release
cargo test
```

## 5. OUTPUT THẬT

`cargo build --release`:
```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `release` profile [optimized] target(s) in 22.56s
```

`cargo test` (≥15 dòng cuối, đầy đủ số test tổng):
```
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 187 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.10s

     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-7ea757478be717ce.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\bin\rpc_probe.rs (target\debug\deps\rpc_probe-62a9296a36bef9b2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bsc_sandwich

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

7 test mới liên quan cụm này đều nằm trong 187 passed (đọc trực tiếp từ log
đầy đủ của phiên, không suy diễn): `missing_pair_scan_universal_fails`,
`pair_scan_universal_ship_default_is_false`, `pair_scan_universal_true_loads_ok`
(`config::tests`), `universal_scan_disabled_by_default_still_not_in_list`,
`universal_scan_enabled_unlisted_pool_becomes_candidate`,
`universal_scan_enabled_below_threshold_is_below_min`,
`universal_scan_enabled_wallet_still_wins` (`pipeline::tests`) — tất cả `ok`,
0 failed, 4 ignored (test `#[ignore]` cần RPC sống, không đổi so BAOCAO19).

## 6. CHAIN

MISSING — phiên này KHÔNG gọi RPC/chain nào (đúng CẤM "không thêm eth_call/RPC
mới"). `pair_addr`/`reserves` dùng trong nhánh universal đến từ
`resolve_v2_reserves` đã có sẵn từ cụm pair-mode (BAOCAO13), không đổi ở đây.
Không có pin mới, không có `getCode` mới cần verify.

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không bị đụng (đúng CẤM). V2/V3/V4-Infinity vẫn
giữ nguyên trạng thái pin từ BAOCAO02/BAOCAO19.

## 8. KHÔNG LÀM

- Không bật `pair_scan_universal=true` trong `config.toml` ship (giữ `false`
  đúng lệnh).
- Không gửi tx thật, không đổi cờ live, không đổi stack.
- Không hồi sinh chiều victim bán.
- Không sửa `executor.rs`/`calldata.rs`/`pool.rs`.
- Không thêm `eth_call`/RPC mới.
- Không tách test ra phiên sau (đã làm đủ trong phiên này, đúng NỢ ghi trong
  lệnh).

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- Chưa đo tải RPC/CPU THẬT khi `pair_scan_universal=true` chạy dài trên
  mempool BSC thật (chỉ ghi nhận lý thuyết ở `docs/STATE.md`: không tăng số
  `eth_call` mỗi tx so pair-mode hiện tại, chỉ tăng số candidate được
  `search_max_front_in`/tax-cache tra — chưa benchmark CPU thật). Nếu Chủ
  định bật thật trên VPS, nên theo dõi `logs/bot.jsonl`/`/api/skips` sau khi
  bật để xem tải tăng bao nhiêu trước khi để chạy dài hạn.
- Mọi nợ khác giữ nguyên như `docs/TASKS.md` đã liệt kê trước phiên này
  (không có nợ mới nào phát sinh ngoài mục trên).
