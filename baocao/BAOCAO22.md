# BAOCAO22

## 1. LÁT

`explicit-mode-flags` — thêm 2 cờ tường minh (`wallet_scan_enabled`,
`pair_scan_enabled`) bật/tắt nhánh wallet/pair-mode trong `decide_paper_v2`,
cùng test cho cả 3 tổ hợp. Một cụm, không cắt vụn.

## 2. LỆNH NHẬN

```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, config.toml, baocao/BAOCAO21.md,
      src/pipeline.rs (decide_paper_v2), src/config.rs.

LÁT: explicit-mode-flags — thêm cờ tường minh bật/tắt cho CẢ 3 mode candidate
     trong decide_paper_v2 (wallet/pair-list/universal), để Chủ có thể bật
     đúng 1 mode, tắt 2 mode còn lại, mà không cần xoá dữ liệu file.

     Thêm 2 field config.toml MỚI, bắt buộc:
     - `wallet_scan_enabled: bool` — ship `true` (BẮT BUỘC default true, vì
       đây là hành vi GỐC đang chạy thật dựa trên victims.txt — default false
       sẽ vô tình TẮT bot đang vận hành khi Chủ cập nhật config, cấm tuyệt
       đối làm vậy).
     - `pair_scan_enabled: bool` — ship `true` (cùng lý do — hành vi gốc dựa
       trên pairs.txt, không được đổi default thành false).
     `pair_scan_universal` GIỮ NGUYÊN tên + default `false` (đã có từ
     BAOCAO20, không đổi).

     Sửa `decide_paper_v2`: nhánh wallet CHỈ xét nếu `cfg.wallet_scan_enabled`
     (false -> bỏ qua hẳn nhánh này, kể cả khi from khớp victims.txt); nhánh
     pair-list CHỈ xét nếu `cfg.pair_scan_enabled`; nhánh universal CHỈ xét
     nếu `cfg.pair_scan_universal` (không đổi logic cũ). Thứ tự ưu tiên GIỮ
     NGUYÊN khi nhiều hơn 1 mode bật cùng lúc: wallet > pair > universal >
     not_in_list — KHÔNG ép buộc chỉ-1-mode ở tầng code (đó là Chủ tự quản lý
     qua config, không phải validate/fail-load).
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/config.rs, src/pipeline.rs (chỉ decide_paper_v2 +
           decide_and_build_paper_v2 nếu cần), config.toml (chỉ thêm 2 field
           mới, default true, không đổi field cũ kể cả pair_scan_universal),
           docs/STATE.md, docs/TASKS.md, baocao/BAOCAO22.md

CẤM: CLAUDE.md, victims.txt thật, .env, cờ live, sendRawTransaction, đổi pair
     khỏi WBNB, hồi sinh chiều victim bán, sửa executor.rs/calldata.rs/
     pool.rs/relay.rs/main.rs, đổi default 2 field mới thành false (PHẢI là
     true), đổi bất kỳ field cũ nào khác.

LÀM:
- 2 field mới + validate (thiếu field = fail load).
- 3 test bắt buộc: default (cả 2 field true, pair_scan_universal false) PHẢI
  giữ NGUYÊN hành vi y hệt trước khi có lệnh này; wallet_scan_enabled=false
  làm ví trong victims.txt KHÔNG còn ưu tiên; pair_scan_enabled=false làm
  pool trong pairs.txt KHÔNG còn match.
- 1 test mô phỏng đúng kịch bản Chủ mô tả: chỉ 1 cờ true, 2 cờ false lần
  lượt cho cả 3 tổ hợp (chỉ wallet / chỉ pair / chỉ universal), đúng nguồn
  match, source đúng field tương ứng.
- Ghi trong docs/STATE.md hướng dẫn Chủ cách dùng đúng 1 mode.

ĐẠT CẦN DÁN: cargo test + cargo build --release, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO22.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

(Lệnh gốc từ Grok có vài dòng bị lỗi copy/xuống dòng giữa chừng — hiểu theo
đúng tinh thần rõ ràng nhất: 3 tổ hợp = "chỉ wallet true" / "chỉ pair true" /
"chỉ universal true", mỗi tổ hợp 2 cờ còn lại `false`.)

## 3. FILE ĐỔI

- `src/config.rs` — thêm 2 field bắt buộc `wallet_scan_enabled: bool` +
  `pair_scan_enabled: bool` (doc-comment giải thích lý do ship `true` bắt
  buộc, đặt ngay sau `pair_scan_universal`, không đổi thứ tự field cũ nào).
  `base_toml()` (test helper) thêm 2 dòng mặc định `true`/`true`. 4 test mới:
  `missing_explicit_mode_flags_fail_load`, `explicit_mode_flags_ship_default_is_true`,
  `explicit_mode_flags_can_be_set_false_independently`.
- `src/pipeline.rs` — `decide_paper_v2`: bọc nhánh wallet trong
  `if cfg.wallet_scan_enabled { ... }`, nhánh pair đổi
  `if pairbook.contains(...)` thành `if cfg.pair_scan_enabled && pairbook.contains(...)`
  (nhánh universal giữ nguyên `if cfg.pair_scan_universal`, không đụng). Cập
  nhật doc-comment phía trên hàm giải thích 2 cờ mới. `test_config_toml()`
  thêm 2 dòng `wallet_scan_enabled = true\npair_scan_enabled = true\n`. 6 test
  mới trong mục `explicit-mode-flags`:
  `explicit_mode_flags_default_ship_true_unchanged_behavior`,
  `wallet_scan_disabled_falls_through_to_pair_when_both_match`,
  `wallet_scan_disabled_and_no_other_match_is_not_in_list`,
  `pair_scan_disabled_falls_through_to_universal_when_pool_listed`,
  `pair_scan_disabled_and_universal_off_is_not_in_list`,
  `explicit_mode_flags_exactly_one_true_matches_correct_source` (test kịch
  bản 3 tổ hợp, ĐẠT CẦN DÁN).
- `config.toml` — thêm 2 dòng field mới (`= true`) + comment giải thích, sau
  khối `pair_scan_universal`. Không đổi field nào khác.
- `docs/STATE.md` — thêm mục `## explicit-mode-flags` (cuối file): lý do ship
  `true` bắt buộc, đoạn code gate trong `decide_paper_v2`, danh sách 9 test
  mới, kết quả `cargo test`, hướng dẫn Chủ cách dùng đúng 1 mode.
- `docs/TASKS.md` — thêm 1 dòng bảng roadmap (đứng ngoài 0.x-7.x, XONG,
  BAOCAO22).
- `baocao/BAOCAO22.md` — file này.

KHÔNG đụng: `CLAUDE.md`, `victims.txt` thật, `.env`, `executor.rs`,
`calldata.rs`, `pool.rs`, `relay.rs`, `main.rs`, `DEX_REGISTRY.md`, không đổi
cờ live, không đổi default 2 field mới thành `false`, không đổi
`pair_scan_universal`/bất kỳ field cũ nào khác.

## 4. LỆNH CHẠY

```
cargo test
cargo build --release
```

## 5. OUTPUT THẬT

`cargo build --release`:
```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `release` profile [optimized] target(s) in 21.58s
```

`cargo test` (≥15 dòng cuối, đầy đủ số test tổng):
```
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test relay::tests::build_and_log_relay_bundle_previews_logs_single_event_with_both_requests ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 205 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.10s

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

205 passed = 196 cũ (BAOCAO21) + 9 test mới (3 ở `config::tests`, 6 ở
`pipeline::tests`), 0 failed, 4 ignored (không đổi — 4 test `#[ignore]` RPC
sống có sẵn từ trước, không chạy trong `cargo test` mặc định). Lọc riêng 9
test mới để xác nhận đủ tên, tất cả `ok`:
```
test config::tests::missing_explicit_mode_flags_fail_load ... ok
test config::tests::explicit_mode_flags_ship_default_is_true ... ok
test config::tests::explicit_mode_flags_can_be_set_false_independently ... ok
test pipeline::tests::explicit_mode_flags_default_ship_true_unchanged_behavior ... ok
test pipeline::tests::wallet_scan_disabled_falls_through_to_pair_when_both_match ... ok
test pipeline::tests::wallet_scan_disabled_and_no_other_match_is_not_in_list ... ok
test pipeline::tests::pair_scan_disabled_falls_through_to_universal_when_pool_listed ... ok
test pipeline::tests::pair_scan_disabled_and_universal_off_is_not_in_list ... ok
test pipeline::tests::explicit_mode_flags_exactly_one_true_matches_correct_source ... ok
```
(9 dòng trên trích từ CÙNG 1 lần chạy `cargo test` ở trên, không chạy lại lần
2 — toàn bộ nằm trong log đầy đủ 205 dòng `ok` đã kiểm tra thủ công, không
bịa.)

Không có 1 dòng test cũ nào (196 test trước phiên này) bị sửa assertion —
chỉ 2 hàm dựng chuỗi TOML test (`config::tests::base_toml`,
`pipeline::tests::test_config_toml`) được thêm 2 dòng field mặc định mới
(bắt buộc vì field mới là required trong config thật).

## 6. CHAIN

Không đổi — phiên này KHÔNG gọi RPC/chain nào (đúng phạm vi lệnh, chỉ sửa
`Config`/`decide_paper_v2` thuần). Không có `eth_call`/`eth_getCode` nào cần
verify ở cụm này.

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không bị đụng. V2/V3/V4-Infinity vẫn giữ
nguyên trạng thái pin từ BAOCAO02/BAOCAO19. Cụm này không liên quan
venue/pool Pancake, chỉ là cờ điều khiển nguồn candidate (wallet/pair/
universal) trong pipeline.

## 8. KHÔNG LÀM

- Không đổi default 2 field mới thành `false` — cả hai ship `true` đúng
  lệnh (hành vi gốc đang chạy thật).
- Không ép buộc "chỉ 1 mode true" ở tầng validate/fail-load — Chủ tự quản
  lý tổ hợp qua `config.toml`.
- Không đổi `pair_scan_universal` (tên/default/logic) — giữ nguyên y hệt
  từ BAOCAO20.
- Không sửa `executor.rs`/`calldata.rs`/`pool.rs`/`relay.rs`/`main.rs`/
  `CLAUDE.md`/`victims.txt` thật/`.env`/`DEX_REGISTRY.md`.
- Không đổi cờ live, không đổi stack, không đổi pair khỏi WBNB, không hồi
  sinh chiều victim bán.
- Không sửa 1 dòng assertion nào trong 196 test cũ — chỉ thêm field mặc
  định vào 2 hàm dựng chuỗi TOML test.
- Không tách test/docs ra phiên sau (đã làm đủ trong phiên này).

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- `main.rs::handle_paper_tx` (đã gọi `decide_and_build_paper_v2` từ BAOCAO18)
  không cần sửa gì thêm ở cụm này — chữ ký `decide_paper_v2`/
  `decide_and_build_paper_v2` không đổi, chỉ đổi HÀNH VI bên trong khi cờ
  mới = `false`. Không có nợ mới phát sinh từ việc nối dây.
- Mọi nợ khác giữ nguyên như `docs/TASKS.md` đã liệt kê trước phiên này —
  không có nợ mới nào phát sinh ngoài phạm vi lệnh này (chỉ thêm 1 dòng
  roadmap ghi nhận cụm đã XONG).
