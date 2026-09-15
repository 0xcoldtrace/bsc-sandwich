# BAOCAO29

## 1. LÁT

`usdt-quote-asset` + `tx.skip.token-logger-fix` — 1 cụm dính (cùng module
decode/pipeline, làm nốt để khỏi nợ vòng copy): (1) sửa CLAUDE.md đúng diff
Chủ duyệt (pair mở rộng token/WBNB **hoặc** token/USDT); (2) pin USDT trong
`DEX_REGISTRY.md`; (3) decode + pipeline nhận diện quote USDT song song
WBNB, gắn nhãn quote asset; (4) math `profit_usdt` THUẦN (không trừ gas);
(5) `config.toml` thêm 4 field mới; (6) fix logger `tx.skip.token` luôn
`null` (nợ từ BAOCAO28).

## 2. LỆNH NHẬN

ĐỌC CLAUDE.md/docs/STATE.md/docs/TASKS.md/DEX_REGISTRY.md/config.toml/
src/relay.rs/src/pipeline.rs/src/pool.rs/BAOCAO27/BAOCAO28. LÀM: (1) sửa
CLAUDE.md theo diff dán nguyên văn (Pair token/WBNB-hoặc-USDT, decode
`not_quote_pair`, Math profit_usdt THUẦN + config thêm 4 field + victims.txt
không thêm cột wallet-mode USDT); (2) pin USDT
`0x55d398326f99059fF775485246999027B3197955` trong `DEX_REGISTRY.md` với
`eth_getCode` thật; (3) decode (pipeline.rs/pool.rs) nhận diện swap quote
USDT song song nhánh WBNB, KHÔNG xoá/giảm chức năng WBNB hiện có; (4) math
`profit_usdt = backUSDT - frontUSDT` không trừ gas, gas chặn riêng qua field
BNB có sẵn; (5) `config.toml` thêm `scan_quote_usdt`(false)/`min_profit_usdt`/
`max_front_usdt`/`min_reserve_usdt`; (6) fix logger `tx.skip` ghi đúng token
thật khi đã biết, giữ `null` chỉ khi chưa decode được. ĐƯỢC ĐỤNG: CLAUDE.md
(theo diff), DEX_REGISTRY.md, config.toml (local, chỉ thêm field mới, ship
`scan_quote_usdt=false`), src/ (decoder.rs/pool.rs/pipeline.rs/venues.rs/
config.rs/main.rs — phần logger), docs/, baocao/. CẤM: .env, victims.txt
thật, mọi cờ live, giảm/xoá chức năng WBNB, sendRaw, VPS/SSH (cụm này THUẦN
LOCAL). KHÔNG LÀM: bật `scan_quote_usdt=true` ở ship default, đổi
`min_profit_bnb`/`max_front_bnb`/`min_reserve_wbnb` hiện có, đụng VPS. ĐẠT
CẦN DÁN: cargo test output ≥15 dòng, cargo build output, đoạn CLAUDE.md đã
sửa, đoạn DEX_REGISTRY.md USDT mới + `eth_getCode` thật. VIẾT:
`baocao/BAOCAO29.md` đủ 10 ô, chữ CHỜ GROK | FAIL | CHƯA XONG (cấm chữ ĐẠT).

## 3. FILE ĐỔI

- `CLAUDE.md` — áp đúng diff Chủ duyệt (dán nguyên văn, không diễn giải
  lại): bullet "Pair" (token/WBNB hoặc token/USDT), bullet Decode
  (`not_quote_pair` thay `not_wbnb_pair` cho USDC/3+ token, USDT là quote
  hợp lệ khi `scan_quote_usdt=true`), mục Math (thêm đoạn `profit_usdt`
  THUẦN + đoạn Config thêm 4 field + đoạn victims.txt không thêm cột
  wallet-mode USDT).
- `DEX_REGISTRY.md` — thêm hàng USDT vào bảng "Core" (cùng WBNB), thêm 1
  đoạn Ghi chú giải thích nguồn pin + RPC dùng verify.
- `src/venues.rs` — thêm `USDT_ADDRESS`/`USDT_GET_CODE_LEN`; `SKIP_REASONS`
  thêm `"not_quote_pair"` (giữ `"not_wbnb_pair"`); 2 test mới.
- `src/decoder.rs` — `TwoTokenPath::token_vs(quote: Address)` (hàm generic
  mới, `token_vs_wbnb()` giờ gọi lại hàm này với `wbnb()` — không đổi hành
  vi cũ); 1 test mới.
- `src/pool.rs` — `resolve_v2_pair_for_quote`/`get_reserves_vs_quote` (hàm
  generic mới); `resolve_v2_pair`/`get_reserves_vs_wbnb` giờ là lớp mỏng gọi
  2 hàm trên với `quote=wbnb()` (không đổi chữ ký/hành vi cho caller cũ); 2
  test mới (1 pure calldata, 1 `#[ignore]` RPC thật).
- `src/config.rs` — 4 field mới (`scan_quote_usdt`/`min_profit_usdt`/
  `max_front_usdt`/`min_reserve_usdt`), validate mở rộng (9 field thay 6),
  3 hàm `_wei()` mới; cập nhật `base_toml()` fixture; 6 test mới.
- `src/pipeline.rs` — `PipelineSkip::NotQuotePair`; `QuoteAsset` enum mới;
  `decode_and_classify_quote`/`evaluate_candidate_quote`/`decide_paper_quote`/
  `precheck_quote_only`/`resolve_reserves_for_quote` (TOÀN BỘ hàm MỚI, song
  song `decode_and_classify`/`decide_paper`/`decide_paper_v2` — 0 dòng nào
  của 3 hàm đó bị sửa); cập nhật `test_config_toml()` fixture; 8 test mới.
- `src/main.rs` — fix bug logger: tách hàm thuần `token_hint_from_precheck`,
  gọi đúng chỗ thay vì hardcode `None`; thêm `#[cfg(test)] mod tests` (mới,
  2 test).
- `config.toml` — thêm 4 field mới cuối file (comment tiếng Việt có dấu,
  đúng quy ước từ `relay-finalize`/BAOCAO24), ship `scan_quote_usdt=false`.
- `docs/STATE.md` — thêm mục `usdt-quote-asset` (quyết định thiết kế + fix
  logger).
- `docs/TASKS.md` — thêm dòng bảng `usdt-quote-asset` + 2 bullet nợ mới.
- `baocao/BAOCAO29.md` (file này, mới).

## 4. LỆNH CHẠY

```
curl (verify chain + pin USDT, RPC công khai, không phải .env)
cargo build --lib --bins
cargo test
```

## 5. OUTPUT THẬT

**Verify chain + pin USDT (RPC công khai `bsc-dataseed.binance.org`, đúng
tiền lệ BAOCAO02 — KHÔNG phải RPC runtime bot)**:
```
eth_chainId -> {"jsonrpc":"2.0","id":1,"result":"0x38"}   # = 56, dung
eth_getCode(0x55d398326f99059fF775485246999027B3197955) -> bytecode ERC20/BEP20 that
  (0x608060405234801561001057600080fd5b506004361061012c...)
byte_len (tinh bang awk, khong Python) = 4413
```

**`cargo build --lib --bins`**:
```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.00s
```

**`cargo test` (rút gọn — toàn bộ 223 test lib PASS, không có test nào bị
sửa assertion so với trước phiên, + 2 test mới trong `main.rs`)**:
```
test decoder::tests::token_vs_generalizes_to_arbitrary_quote_address ... ok
test config::tests::missing_usdt_quote_asset_fields_fail_load ... ok
test pipeline::tests::decide_paper_quote_wbnb_branch_still_works_when_usdt_disabled ... ok
test pipeline::tests::precheck_quote_only_never_touches_rpc_for_early_skip_reasons ... ok
test pipeline::tests::usdt_quote_enabled_honeypot_or_tax_when_unmeasured ... ok
test pipeline::tests::usdt_quote_enabled_thin_liq_when_pool_reserve_below_min_reserve_usdt ... ok
test pipeline::tests::usdt_quote_disabled_by_default_is_not_quote_pair ... ok
test pool::tests::calldata_encodes_correctly_for_non_wbnb_quote_usdt ... ok
test pool::tests::real_rpc_v2_get_pair_and_reserves_for_non_wbnb_quote_usdt ... ignored
test pipeline::tests::usdt_quote_unprofitable_when_profit_below_min_profit_usdt_config ... ok
test venues::tests::skip_reasons_contains_both_not_wbnb_pair_and_not_quote_pair ... ok
test venues::tests::usdt_pinned_and_nonzero ... ok
test pipeline::tests::usdt_quote_profit_has_no_gas_subtracted_matches_gas_wei_zero_exactly ... ok

test result: ok. 223 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 4.11s

     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-....exe)

running 2 tests
test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok
test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
(5 test `#[ignore]` là RPC sống, đúng quy ước cũ — không chạy trong
`cargo test` mặc định. Trước phiên này tổng test lib là 206 [BAOCAO25] rồi
tăng dần qua các cụm sau — phiên này cộng thêm 17 test mới [1 decoder + 2
pool + 6 config + 8 pipeline] + 2 test `main.rs` mới [file trước đây chưa có
`mod tests`], không có test cũ nào bị xoá/sửa assertion.)

**Bằng chứng số học "profit_usdt không trừ gas" (chạy riêng THẬT bằng
`cargo test --lib pipeline::tests::usdt_quote_profit_has_no_gas_subtracted -- --nocapture`,
số dán dưới đây là output THẬT, không bịa)**:
```
running 1 test
usdt quote sim OK: front_in=2999995000000000000000 back_out=3233788939895660525529 profit_wei=233793939895660525529
test pipeline::tests::usdt_quote_profit_has_no_gas_subtracted_matches_gas_wei_zero_exactly ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 227 filtered out; finished in 0.01s
```
`front_in≈2999.995 USDT` (bị chặn gần sát `max_front_usdt=3000` trừ
`gas_reserve_bnb_wei=0.005 BNB`, đúng `RiskGuard::front_cap_after_gas_reserve`),
`profit_wei=233793939895660525529` wei ≈ `233.79 USDT` — assertion trong
test xác nhận số này khớp CHÍNH XÁC với lời gọi trực tiếp
`sim_v2::search_max_front_in(..., gas_wei=0)`, và lệch ĐÚNG bằng
`cfg.gas_wei()` (= `6e15` wei = `front_max_gas_bnb_wei + back_max_gas_bnb_wei`
ship) so với lời gọi cùng tham số nhưng `gas_wei=cfg.gas_wei()` — chứng minh
bằng số học (không chỉ khẳng định bằng lời) rằng nhánh USDT không trừ gas
vào `profit_wei`.

## 6. CHAIN

`0x38` xác nhận THẬT qua RPC công khai (`eth_chainId`), dùng để verify pin
USDT (`eth_getCode` > 0, 4413 byte). Không pin thêm contract router/factory
nào phiên này (USDT là quote asset, không phải venue family).

## 7. REGISTRY

Thêm 1 hàng USDT vào mục "Core" của `DEX_REGISTRY.md` (cùng WBNB) —
`0x55d398326f99059fF775485246999027B3197955`, `pinned_date=2026-09-15`,
`getCode=4413`, `PINNED`. KHÔNG đổi bảng V2/V3/V4 (USDT không phải router/
factory). `src/venues.rs::registry_snapshot` KHÔNG liệt kê USDT (đúng —
hàm đó chỉ mô tả family AMM V2/V3/V4/mới hơn, không phải quote asset; xem
docs/TASKS.md mục nợ nếu Chủ muốn dashboard hiển thị USDT sau này).

## 8. KHÔNG LÀM

- KHÔNG bật `scan_quote_usdt=true` ở ship default (`config.toml` vẫn
  `false`, xác nhận qua test `scan_quote_usdt_ship_default_is_false`).
- KHÔNG đổi `min_profit_bnb`/`max_front_bnb`/`min_reserve_wbnb`/
  `max_roundtrip_tax`/`max_exposure_bnb` hiện có trong `config.toml`.
- KHÔNG đụng `.env`, `victims.txt` thật, mọi cờ live (`allow_live`/
  `dry_run`/`bot_armed`/`live_v2`/`live_v3`/`live_v4` không đổi).
- KHÔNG xoá/sửa 1 dòng nào của `decode_and_classify`/`decide_paper`/
  `decide_paper_v2`/`decide_and_build_paper_v2`/`resolve_v2_reserves`/
  `pool::resolve_v2_pair`/`pool::get_reserves_vs_wbnb`/`token_vs_wbnb` —
  toàn bộ USDT là hàm MỚI song song (xem docs/STATE.md).
- KHÔNG nối `decide_paper_quote` vào `main.rs::handle_paper_tx` (live loop)
  — quyết định phạm vi có chủ đích, `main.rs` chỉ đụng cho fix logger (xem
  ô 10 + docs/STATE.md).
- KHÔNG đụng VPS/SSH, `executor.rs`/`calldata.rs` (build tx thật), `web.rs`
  (dashboard chưa hiển thị USDT), `relay.rs` (không cần đọc quote — dùng chỉ
  để đọc hằng số endpoint khi verify pin, không đổi file này).
- KHÔNG sendRaw, không bịa số getCode/chainId (dán nguyên response thật).
- KHÔNG tự viết chữ ĐẠT.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- `decide_paper_quote`/`precheck_quote_only`/`resolve_reserves_for_quote`
  CHƯA nối vào `main.rs::handle_paper_tx` (live loop pending-tx thật) — hàm
  public, test đầy đủ (thuần, không cần RPC sống), sẵn sàng nối dây. Cần
  quyết định kiến trúc (chạy song song `decide_paper_v2` cho mỗi tx, hay
  merge 2 entrypoint) — ngoài phạm vi 1 khối lệnh, cần lệnh Grok riêng.
- `executor.rs`/`calldata.rs` (build tx front-buy/back-sell paper) CHƯA hỗ
  trợ quote USDT — chỉ build được calldata V2 Router hướng WBNB. Candidate
  USDT `Simulated` hiện tại KHÔNG có `tx.build` log tương ứng (vì chưa nối
  main.rs — xem trên) — cần cụm riêng nếu muốn.
- KHÔNG có wallet-mode/ngưỡng min-swap-size riêng cho USDT (đúng lệnh gốc
  "KHÔNG thêm cột cho wallet-mode ở cụm này") — chỉ `min_reserve_usdt` (mức
  pool) lọc. Nếu Chủ muốn ngưỡng min-swap theo từng ví cho USDT sau này, cần
  field config mới + lệnh riêng.
- 3 ngưỡng ship (`min_profit_usdt=3.0`/`max_front_usdt=3000.0`/
  `min_reserve_usdt=15000.0`) là SỐ KHỞI TẠO THÔ do Code chọn (số tròn,
  KHÔNG price oracle, KHÔNG quy đổi tỉ giá thật từ `*_bnb`) — Chủ nên tự
  chỉnh theo giá trị thực tế mong muốn trong `config.toml`.
- `venues.rs::registry_snapshot`/`GET /api/venues` không liệt kê USDT/trạng
  thái `scan_quote_usdt` — nếu Chủ/Grok muốn dashboard hiển thị, cần khối
  lệnh riêng đụng `web.rs`/`web/` (không đụng phiên này).
- Kế thừa nợ cũ chưa đổi: V4/Infinity sim chưa có, đo tax thật (probe) chưa
  làm, `RiskGuard::record_result` chưa gọi, relay chưa nối dây, `pairs.txt`
  VPS lỗi parse (BAOCAO27/28), `PRIVATE_TX_URL` Chủ dán ở BAOCAO27 vẫn chưa
  ghi.
