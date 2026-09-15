# BAOCAO30

## 1. LÁT

`quote-live-wiring-funnel-diagnostics` — cụm dính 5 mục theo lệnh Grok.
**3/5 mục làm được** (2: nối `decide_paper_quote` vào `main.rs` cho
WBNB+USDT; 3: funnel log mỗi phút; 4: 2 unit test decode bắt buộc + skip
reason `sell_direction` mới). **2/5 mục KHÔNG làm được, ghi rõ lý do, không
bịa output**: mục (1) sửa CLAUDE.md theo 1 diff không có trong khối lệnh
nhận được phiên này; mục (5) VPS (seed tax allowlist + `scan_quote_usdt=true`
+ rerun 30 phút) bị chặn vì sandbox phiên này không có outbound SSH (port
22) tới VPS đã biết. GATE (câu hỏi 30 phút) vì vậy KHÔNG trả lời được.

## 2. LỆNH NHẬN

ĐỌC CLAUDE.md/docs/STATE.md/docs/TASKS.md/config.toml/src/pipeline.rs/
src/main.rs/src/decoder.rs/BAOCAO27/28/29. LÀM: (1) sửa CLAUDE.md áp đúng
diff dán trên (mục Math: revm bắt buộc, ĐO TAX, QUOTE_SET) — **diff đó
không có trong nội dung lệnh nhận được, xem ô 8/ô 10**; (2) nối
`decide_paper_quote` vào `main.rs::handle_paper_tx` cho WBNB+USDT
(Phase 1), không xoá/sửa hành vi `decide_paper_v2` WBNB hiện có; (3) funnel
log bắt buộc mỗi phút (`seen/decoded/quote_ok/two_hop/buy_side/liq_ok/
tax_ok/simulated/profitable`), thêm skip reason còn thiếu nếu cần
(`multihop`, `sell_direction`, `no_inventory`); (4) 2 unit test decode bắt
buộc (swapExactETHForTokens→WBNB amountIn=tx.value; swapExactTokensForTokens
path [USDT,X]→USDT); (5) TRÊN VPS: seed tax allowlist 6 token qua
`/api/tax`, bật `scan_quote_usdt=true` CHỈ VPS, rerun mode 3 (universal) 30
phút, đăng nhập bằng `key/bsc_vps_ed25519`. GATE bắt buộc: trả lời "bao
nhiêu swap [WBNB,CAKE]/[WBNB,USDT]/[USDT,X] xuất hiện, dừng ở bước nào" —
nếu bug filter thì PHẢI sửa trong cụm này. KHÔNG thêm `revm` phiên này.
ĐƯỢC ĐỤNG: CLAUDE.md (theo diff), src/main.rs, src/pipeline.rs,
src/decoder.rs (nếu cần), config.toml (LOCAL, không thiếu field), docs/,
baocao/, VPS `.env` (không đụng), VPS config.toml (chỉ `scan_quote_usdt`),
`state/tax_inject`. CẤM: `.env`, victims.txt thật, mọi cờ live, `revm`/
deploy thật, sendRaw, đổi `min_profit_bnb/max_front_bnb/min_reserve_wbnb`,
giảm/xoá hành vi WBNB. VIẾT: `baocao/BAOCAO30.md` đủ 10 ô, chữ CHỜ GROK |
FAIL | CHƯA XONG (cấm ĐẠT).

## 3. FILE ĐỔI

- `src/pipeline.rs`:
  - `PipelineSkip::SellDirection` (biến thể mới, `as_str()="sell_direction"`)
    — CHỈ dùng trong `decode_and_classify_quote` (hàm quote-aware MỚI, từ
    BAOCAO29). `decode_and_classify`/`decide_paper_v2`/`decide_paper` KHÔNG
    đổi 1 dòng — vẫn trả `not_wbnb_pair` y hệt trước.
  - `decode_and_classify_quote` — thêm nhánh phát hiện "quote asset CÓ
    trong path nhưng sai phía" (`token_vs(quote).is_some()` khi `token_a !=
    quote`) → `SellDirection` thay vì rơi tuột vào `NotQuotePair` chung
    chung. Không đụng `decoder.rs`.
  - 3 test mới: 2 test bắt buộc lệnh mục (4)
    (`decode_and_classify_quote_swap_exact_eth_for_tokens_gives_wbnb_and_amount_in_eq_tx_value`,
    `decode_and_classify_quote_swap_exact_tokens_for_tokens_usdt_path_gives_usdt`)
    + 1 test đối chứng `SellDirection` vs hành vi cũ không đổi.
- `src/main.rs`:
  - `FunnelCounters`/`FunnelSnapshot`/`FunnelHit`/`funnel_hit()`/
    `funnel_report_task()` — MỚI, KHÔNG đưa vào `AppStateInner`
    (`src/web.rs` ngoài `ĐƯỢC ĐỤNG` phiên này), sống độc lập, truyền qua
    `Arc<FunnelCounters>` riêng.
  - `handle_paper_tx` — thêm tham số `funnel: Arc<FunnelCounters>`, thêm
    nhánh fallback USDT (chỉ chạy khi nhánh WBNB gốc trả `NotWbnbPair` VÀ
    `cfg.scan_quote_usdt=true`), gọi `funnel.record_hit(funnel_hit(&outcome))`
    trước khi log. Nhánh WBNB gốc (`precheck_token_only` ->
    `resolve_v2_reserves` -> `decide_and_build_paper_v2`) 0 dòng đổi.
  - `connect_rpc`/`subscribe_pending_txs`/`poll_txpool_pending`/
    `watch_inject_file` — thêm tham số `funnel: Arc<FunnelCounters>`, truyền
    xuống các lời gọi `handle_paper_tx`.
  - `main()` — tạo `funnel = Arc::new(FunnelCounters::new())`, spawn
    `funnel_report_task(logger.clone(), funnel.clone(), 60s)`.
  - `#[cfg(test)] mod tests` — thêm 7 test `funnel_hit_*`.
- `docs/STATE.md` — thêm mục `quote-live-wiring-funnel-diagnostics` (quyết
  định kiến trúc + verify thật + lý do 2 mục blocked).
- `docs/TASKS.md` — thêm dòng bảng + nợ mới.
- `baocao/BAOCAO30.md` (file này, mới).
- **KHÔNG đổi**: `CLAUDE.md`, `config.toml` (LOCAL, `scan_quote_usdt` vẫn
  `false`), `src/decoder.rs`, `src/venues.rs`, `src/web.rs`, VPS/`.env`.

## 4. LỆNH CHẠY

```
cargo build --lib --bins
cargo test
cargo test --lib decode_and_classify_quote_swap_exact -- --nocapture
# Verify thật (binary release, config tạm scratchpad, KHÔNG sửa config.toml):
cargo run --release --bin bsc_sandwich -- <scratch>/smoke_config.toml
```

## 5. OUTPUT THẬT

**`cargo build --lib --bins`**:
```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.89s
```
(0 warning.)

**`cargo test` (rút gọn — 226 test lib + 9 test main.rs, TOÀN BỘ PASS, 0
sửa assertion cũ, +10 test mới so với BAOCAO29 [223+2] = 3 pipeline + 7
main.rs)**:
```
running 231 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test calldata::tests::encoded_selectors_match_decoder_well_known_constants ... ok
test decoder::tests::decode_v3_exact_input_multihop_is_not_wbnb_pair ... ok
test pipeline::tests::decode_and_classify_quote_swap_exact_eth_for_tokens_gives_wbnb_and_amount_in_eq_tx_value ... ok
test pipeline::tests::decode_and_classify_quote_swap_exact_tokens_for_tokens_usdt_path_gives_usdt ... ok
test pipeline::tests::decode_and_classify_quote_sell_direction_is_distinguished_from_not_quote_pair ... ok
test pipeline::tests::decide_paper_v2_sell_direction_is_not_wbnb_pair ... ok
test pipeline::tests::usdt_quote_disabled_by_default_is_not_quote_pair ... ok
test pipeline::tests::usdt_quote_profit_has_no_gas_subtracted_matches_gas_wei_zero_exactly ... ok
test venues::tests::skip_reasons_contains_both_not_wbnb_pair_and_not_quote_pair ... ok
test executor::tests::no_send_raw_transaction_call_anywhere_in_src ... ok

test result: ok. 226 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 4.09s

     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-7ea757478be717ce.exe)

running 9 tests
test tests::funnel_hit_decode_fail_stops_at_seen ... ok
test tests::funnel_hit_honeypot_or_tax_stops_before_tax_ok ... ok
test tests::funnel_hit_not_in_list_below_min_and_thin_liq_stop_before_liq_ok ... ok
test tests::funnel_hit_victim_would_revert_and_unprofitable_pass_tax_ok_but_not_simulated ... ok
test tests::funnel_hit_not_wbnb_pair_and_not_quote_pair_stop_before_quote_ok ... ok
test tests::funnel_hit_sell_direction_stops_before_buy_side ... ok
test tests::funnel_hit_simulated_marks_every_stage_true ... ok
test tests::token_hint_from_precheck_keeps_token_when_decode_succeeded ... ok
test tests::token_hint_from_precheck_is_none_only_when_decode_itself_failed ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**2 test bắt buộc lệnh mục (4), chạy riêng để dán rõ ĐẠT CẦN DÁN**:
```
running 2 tests
test pipeline::tests::decode_and_classify_quote_swap_exact_eth_for_tokens_gives_wbnb_and_amount_in_eq_tx_value ... ok
test pipeline::tests::decode_and_classify_quote_swap_exact_tokens_for_tokens_usdt_path_gives_usdt ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 229 filtered out; finished in 0.00s
```

**Verify THẬT wiring (KHÔNG phải chỉ unit test)** — chạy binary `--release`
với config tạm (copy `config.toml` LOCAL, chỉ đổi `scan_quote_usdt=true`
trong SCRATCHPAD, KHÔNG sửa `config.toml` thật) + `.env` LOCAL sẵn có
(`BSC_HTTP`/`BSC_WS` thật) + bơm 1 dòng `state/inject_tx.jsonl` (calldata
`swapExactTokensForTokens` THẬT dựng bằng `cast calldata` của Foundry, path
`[USDT, CAKE]`, cả 2 địa chỉ well-known thật, KHÔNG bịa selector), chạy 75
giây rồi dừng — log THẬT trong `logs/bot.jsonl`:
```
{"event":"rpc.connect","transport":"http", ...}
{"event":"rpc.connect","transport":"ws", ...}
{"event":"tx.skip","from":"0x1234567890123456789012345678901234567890","reason":"no_pool","source":"usdt","token":"0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82","ts":"2026-09-14T23:38:55.800733400+00:00"}
{"buy_side":0,"decoded":0,"event":"funnel.minute","liq_ok":0,"profitable":0,"quote_ok":0,"seen":0,"simulated":0,"tax_ok":0,"ts":"2026-09-14T23:38:55.800297+00:00","two_hop":0}
{"buy_side":1,"decoded":1,"event":"funnel.minute","liq_ok":0,"profitable":0,"quote_ok":1,"seen":1,"simulated":0,"tax_ok":0,"ts":"2026-09-14T23:39:55.805707700+00:00","two_hop":1}
```
Giải thích: nhánh WBNB gốc trả `not_wbnb_pair` trước (path không có WBNB) —
nhánh fallback USDT (mới) nhận diện ĐÚNG CAKE là token/USDT là quote, gọi
`eth_call Factory.getPair` THẬT qua RPC thật, không tìm thấy pool CAKE/USDT
trực tiếp → `no_pool` (đúng, không bịa — CAKE thường pair với WBNB/BUSD).
Token trong log KHÔNG còn `null` (fix logger BAOCAO29 áp dụng đúng cho cả
nhánh USDT). Funnel phút thứ 2 = `seen=1,decoded=1,quote_ok=1,two_hop=1,
buy_side=1,liq_ok=0,...` — khớp CHÍNH XÁC với việc dừng đúng trước `liq_ok`
(vì `no_pool`, không rõ reserve). `state/inject_tx.jsonl`/`logs/bot.jsonl`
đã được restore lại nguyên trạng sau khi verify (2 file này gitignored).

## 6. CHAIN

`0x38` (56) xác nhận qua `.env` LOCAL thật lúc verify wiring (mục 5,
`rpc.connect` cho cả `http`/`ws`, thấy trong log thật). Không pin thêm
contract nào phiên này — không đụng registry.

## 7. REGISTRY

Không đổi `DEX_REGISTRY.md` phiên này (không có venue/contract mới cần
pin).

## 8. KHÔNG LÀM

- **KHÔNG sửa `CLAUDE.md`** — mục (1) của lệnh yêu cầu "áp ĐÚNG diff dán
  trên", nhưng nội dung diff thực tế KHÔNG có mặt trong khối lệnh nhận được
  phiên này (nhắc tới "revm bắt buộc" nhưng cùng lệnh lại ghi "KHÔNG thêm
  revm phiên này — cụm B riêng", mâu thuẫn nội bộ càng xác nhận diff đó
  thuộc 1 lệnh/phiên khác). CLAUDE.md mục 0.ANTI cấm bịa nội dung — không
  đoán diff để áp cho xong.
- **KHÔNG làm được mục (5) VPS** — SSH (port 22) outbound từ sandbox phiên
  này bị timeout tới cả 3 IP VPS tìm thấy trong `~/.ssh/known_hosts` (từ
  các phiên trước), trong khi HTTPS (443) ra ngoài vẫn hoạt động bình
  thường (`curl https://www.google.com` → 200) — xác nhận đây là giới hạn
  hạ tầng/mạng của sandbox phiên này, không phải VPS đã chết hay Code từ
  chối làm. Không seed được tax allowlist, không bật được
  `scan_quote_usdt=true` trên VPS, không rerun được 30 phút.
- **GATE KHÔNG trả lời được** — phụ thuộc hoàn toàn vào mục (5) (cần mempool
  thật 30 phút trên VPS). Không bịa số liệu 30 phút/số swap WBNB-CAKE/
  WBNB-USDT/USDT-X để né việc thiếu.
- KHÔNG thêm `revm` (đúng CẤM lệnh).
- KHÔNG đụng `src/decoder.rs` — không thêm phân biệt `multihop` ở tầng
  decoder (rủi ro phá test cũ `decode_v3_exact_input_multihop_is_not_wbnb_pair`
  nếu đổi `TwoTokenPath::from_packed_v3_path`, cân nhắc lợi ích/rủi ro thấy
  không đáng phiên này — xem docs/STATE.md).
- KHÔNG thêm `PipelineSkip::Multihop`/`NoInventory` — `no_inventory` không
  áp dụng cho kiến trúc bot hiện tại (không giữ tồn kho token/không
  flashloan), thêm vào sẽ là enum chết không bao giờ kích hoạt.
- KHÔNG đụng `src/venues.rs` (`SKIP_REASONS`)/`src/web.rs` (`AppStateInner`)
  — ngoài `ĐƯỢC ĐỤNG` phiên này. Hệ quả: `sell_direction` chưa hiện trên
  dashboard `/api/skips`, `funnel.minute` chưa có API riêng (chỉ trong
  `logs/bot.jsonl`).
- KHÔNG đổi `config.toml` LOCAL thật (`scan_quote_usdt` vẫn `false`) — chỉ
  dùng file tạm trong scratchpad (ngoài repo) để verify wiring thật.
- KHÔNG đổi `min_profit_bnb`/`max_front_bnb`/`min_reserve_wbnb`, không
  giảm/xoá hành vi WBNB (verify: toàn bộ test cũ pass, 0 sửa assertion),
  không sendRaw, không bịa pin/output.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **CLAUDE.md diff (mục 1) chưa áp** — cần Grok dán lại NGUYÊN VĂN diff ở
  lệnh sau (nội dung liên quan "revm bắt buộc, ĐO TAX, QUOTE_SET" — lưu ý
  bản thân lệnh phiên này cấm thêm `revm`, nên khả năng cao diff đó dành
  cho 1 cụm/phiên khác, cần Grok xác nhận lại phạm vi trước khi dán).
- **Mục (5) VPS + GATE chưa làm được** — cần 1 trong 2: (a) Chủ/Grok cấp 1
  kênh có outbound SSH (port 22) mở tới VPS ở phiên sau, hoặc (b) xác nhận
  IP VPS hiện tại còn sống/đổi IP mới rồi thử lại. Sau khi có kênh: seed
  tax allowlist 6 token qua `/api/tax` (địa chỉ lấy từ `venues.rs`/
  `pairs.txt` đã pin), bật `scan_quote_usdt=true` CHỈ VPS, rerun mode 3
  (universal) 30 phút, trả lời GATE bằng số liệu THẬT.
- `sell_direction` chưa vào `venues.rs::SKIP_REASONS` → không hiện trên
  dashboard `/api/skips` (vẫn đếm đúng trong log JSONL thô/`skip_counts`
  nội bộ) — cần khối lệnh riêng đụng `venues.rs`/`web.rs` nếu Chủ muốn hiện
  đầy đủ.
- `funnel.minute` chỉ có trong `logs/bot.jsonl`, chưa có `/api/funnel` hay
  hiển thị trên dashboard web — cần khối lệnh riêng đụng `web.rs` nếu Chủ
  muốn xem trực quan thay vì đọc log thô.
- `two_hop` trong funnel LUÔN bằng `quote_ok` (chưa có tín hiệu multihop
  riêng phiên này, xem "KHÔNG LÀM") — nếu Chủ muốn số `two_hop` có ý nghĩa
  riêng, cần 1 cụm riêng thêm `SkipReason::Multihop` vào `decoder.rs` (có
  thể cần sửa có chủ đích 1 test cũ ở decoder.rs, không né được nếu làm).
- Kế thừa nợ cũ chưa đổi (từ BAOCAO27/28/29): V4/Infinity sim chưa có, đo
  tax thật (probe contract) chưa làm, `RiskGuard::record_result` chưa gọi,
  relay chưa nối dây, `pairs.txt` VPS lỗi parse, `PRIVATE_TX_URL` Chủ dán ở
  BAOCAO27 vẫn chưa ghi, `executor.rs`/`calldata.rs` chưa hỗ trợ build tx
  paper cho quote USDT (chỉ WBNB).
