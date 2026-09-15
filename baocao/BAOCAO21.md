# BAOCAO21

## 1. LÁT

`relay-bundle-builder` — module MỚI `src/relay.rs`, hàm THUẦN (không gọi
HTTP/network) build request body JSON-RPC bundle cho 2 relay đã research thật
(48 Club — `eth_sendBundle`; BlockRazor — `eth_sendMevBundle`). Một cụm,
không cắt vụn.

## 2. LỆNH NHẬN

```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO20.md, Cargo.toml.

LÁT: relay-bundle-builder — module MỚI src/relay.rs, hàm THUẦN (không gọi
     HTTP/network) build request body JSON-RPC bundle cho 2 relay đã research
     thật (nguồn dưới, KHÔNG được đoán/bịa field nào ngoài danh sách này):

     ## 48 Club (Puissant Builder v2)
     - RPC endpoint (chỉ ghi làm hằng số, KHÔNG gọi): `https://rpc.48.club`
       (chain 56). Nguồn: https://docs.48.club/privacy-rpc (đọc 2026-09-15).
     - Method: `eth_sendBundle`. Nguồn:
       https://docs.48.club/puissant-builder/send-bundle (đọc 2026-09-15).
     - Params: `txs: [String]` (raw signed tx hex, BẮT BUỘC),
       `backrunTarget: Option<String>` (hash), `maxBlockNumber: Option<u64>`
       (default hiện tại+100 nếu None), `maxTimestamp: Option<u64>`,
       `revertingTxHashes: Option<Vec<String>>`, `noMerge: Option<bool>`,
       `positionFirst: Option<bool>`, `48spSign: Option<String>` (optional,
       bỏ qua phiên này — không có 48SoulPoint key thật).

     ## BlockRazor
     - RPC endpoint: `https://bsc.blockrazor.xyz`. Nguồn:
       https://docs.blockrazor.io/transaction-submission/rpc/bsc/integration
       (đọc 2026-09-15).
     - Method: `eth_sendMevBundle`. Nguồn:
       https://docs.blockrazor.io/transaction-submission/rpc/bsc/eth_sendbundle.md
       (đọc 2026-09-15).
     - Params: `txs: [String]` (raw signed tx hex, tối đa 50, BẮT BUỘC),
       `revertingTxHashes: Option<Vec<String>>`,
       `maxBlockNumber: Option<u64>` (default hiện tại+100 nếu None).

     Hàm nhận `front_raw_hex: &str, back_raw_hex: &str` (raw signed tx hex —
     CHƯA có signer thật trong repo nên test dùng CHUỖI HEX GIẢ LẬP rõ ràng
     đánh dấu là fixture, giống cách calldata.rs test dùng địa chỉ fixture,
     KHÔNG claim là tx thật), trả `serde_json::Value` đúng schema JSON-RPC
     2.0 đầy đủ (`jsonrpc`,`id`,`method`,`params`) cho cả 2 relay. Log ra
     (nếu có logger) event `bundle.build_preview` — CHỈ LOG, KHÔNG gọi HTTP
     POST tới bất kỳ URL relay nào phiên này.

stack = Rust. serde_json ĐÃ có sẵn trong Cargo.toml, đủ dùng — KHÔNG thêm
reqwest/hyper/bất kỳ HTTP client crate nào phiên này.

ĐƯỢC ĐỤNG: src/relay.rs (mới), src/lib.rs (+pub mod relay;), docs/STATE.md,
           docs/TASKS.md, baocao/BAOCAO21.md

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, Cargo.toml (không
     thêm crate HTTP/network nào), bật cờ live, GỌI HTTP THẬT tới bất kỳ URL
     nào (kể cả "chỉ test"), sendRawTransaction, đổi pair khỏi WBNB, hồi sinh
     chiều victim bán, sửa executor.rs/pipeline.rs/calldata.rs/pool.rs.

ĐẠT CẦN DÁN: cargo test + cargo build --release, ≥15 dòng output cuối.
```

## 3. FILE ĐỔI

- `src/relay.rs` (MỚI) — 2 hàm build thuần `build_48club_send_bundle_request`/
  `build_blockrazor_send_mev_bundle_request` (đúng field pin trong lệnh, dùng
  `serde_json::Map` insert có điều kiện để field `Option::None` bị LOẠI khỏi
  JSON thay vì ghi `null`), 2 struct options (`Club48BundleOptions`,
  `BlockRazorBundleOptions`, `Default` = tất cả `None`), 1 hàm chuẩn hoá
  `normalize_raw_tx_hex` (thêm `0x` nếu thiếu), 1 hàm ghép
  `build_and_log_relay_bundle_previews` (build cả 2 + log 1 event
  `bundle.build_preview`), 2 hằng số endpoint (chỉ tham chiếu, không gọi). 9
  test mới (schema mặc định/optional cho từng relay, chuẩn hoá hex, giới hạn
  50 tx BlockRazor, log preview, và test tự-grep xác nhận không có
  `reqwest`/`hyper`/`TcpStream`/`http::Client` trong chính file này).
- `src/lib.rs` — thêm `pub mod relay;` (1 dòng, giữ thứ tự alphabet như các
  module khác).
- `docs/STATE.md` — thêm mục `## relay-bundle-builder` ghi nguồn field pin,
  2 quyết định kỹ thuật Claude tự chọn ngoài phạm vi lệnh (hình dạng `params`
  bọc mảng, quy ước loại field `None`), lý do `current_block`/`request_id` là
  tham số bắt buộc, và kết quả test.
- `docs/TASKS.md` — thêm 1 dòng bảng roadmap (MỘT PHẦN, đứng ngoài 0.x-7.x) +
  1 mục nợ mới ở "Nợ / MISSING hiện tại".
- `baocao/BAOCAO21.md` — file này.

KHÔNG đụng: `CLAUDE.md`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md`,
`Cargo.toml`, `executor.rs`, `pipeline.rs`, `calldata.rs`, `pool.rs`, không
đổi cờ live nào, không thêm crate HTTP/network nào, không gọi HTTP thật, không
sendRaw, không hồi sinh chiều victim bán.

## 4. LỆNH CHẠY

```
cargo build --release
cargo test
```

## 5. OUTPUT THẬT

`cargo build --release`:
```
    Finished `release` profile [optimized] target(s) in 0.47s
```
(build sạch từ trước đó trong cùng phiên đã in `Compiling bsc_sandwich v0.1.0
... Finished release profile [optimized] target(s) in 21.63s` — lần chạy sau
chỉ "Finished" vì không có gì đổi thêm sau lần compile đó.)

`cargo test` (≥15 dòng cuối, đầy đủ số test tổng):
```
test transport::tests::placeholder_url_fails_fast_no_panic ... ok
test executor::tests::no_send_raw_transaction_call_anywhere_in_src ... ok
test transport::tests::vps_fallback_missing_file_is_empty_not_panic ... ok
test venues::tests::every_pinned_contract_has_nonzero_get_code_len ... ok
test venues::tests::newer_family_still_disabled_not_deleted ... ok
test venues::tests::scan_and_live_flags_pass_through_unchanged ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
test venues::tests::wbnb_pinned_and_nonzero ... ok
test victims::tests::bnb_to_wei_basic ... ok
test victims::tests::bnb_to_wei_rejects_garbage ... ok
test victims::tests::checksum_and_lowercase_same_wallet ... ok
test victims::tests::duplicate_address_last_line_wins ... ok
test victims::tests::garbage_lines_logged_and_skipped_no_panic ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test relay::tests::build_and_log_relay_bundle_previews_logs_single_event_with_both_requests ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 196 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.10s

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

196 passed = 187 cũ (BAOCAO20) + 9 test mới trong `relay::tests` (chạy riêng
lọc `cargo test relay::` xác nhận đủ tên, tất cả `ok`, 0 failed):
`normalize_raw_tx_hex_adds_0x_prefix_when_missing`,
`normalize_raw_tx_hex_keeps_existing_0x_prefix`,
`club48_request_matches_pinned_schema_with_default_options`,
`club48_request_includes_all_optional_fields_when_provided`,
`blockrazor_request_matches_pinned_schema_with_default_options`,
`blockrazor_request_includes_reverting_tx_hashes_and_explicit_max_block_number`,
`blockrazor_txs_never_exceeds_documented_50_tx_limit`,
`build_and_log_relay_bundle_previews_logs_single_event_with_both_requests`,
`no_http_network_calls_anywhere_in_relay_rs`.

## 6. CHAIN

MISSING — phiên này KHÔNG gọi RPC/chain nào (đúng CẤM "GỌI HTTP THẬT tới bất
kỳ URL nào, kể cả chỉ test"). `src/relay.rs` không có `Provider`/`Signer`/HTTP
client nào, không có `eth_call`/`eth_getCode` nào cần verify. Endpoint 2 relay
chỉ được ghi làm hằng số Rust (`CLUB48_RPC_URL`, `BLOCKRAZOR_RPC_URL`), không
được dùng để mở kết nối nào trong file này — verify bằng test tự-grep
`no_http_network_calls_anywhere_in_relay_rs` (grep chính `src/relay.rs`,
loại trừ dòng comment, tìm `reqwest`/`hyper`/`TcpStream`/`http::Client`, kết
quả 0 dòng vi phạm, test `ok`).

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không bị đụng (đúng CẤM). V2/V3/V4-Infinity vẫn
giữ nguyên trạng thái pin từ BAOCAO02/BAOCAO19. Cụm này không liên quan
venue/pool Pancake, chỉ là format request gửi relay bundle-builder (hạ tầng
gửi tx, không phải AMM).

## 8. KHÔNG LÀM

- Không gọi HTTP thật tới `https://rpc.48.club`/`https://bsc.blockrazor.xyz`
  hay bất kỳ URL nào khác — không thêm `reqwest`/`hyper`/HTTP client nào vào
  `Cargo.toml` (file không bị đụng, xác nhận qua `git status`/grep).
- Không ký tx thật — `front_raw_hex`/`back_raw_hex` trong test là 2 hằng số
  FIXTURE ghi rõ trong code (`FIXTURE_FRONT_RAW_TX_HEX`/
  `FIXTURE_BACK_RAW_TX_HEX`), không claim là tx thật.
- Không thêm field `48spSign` (không có 48SoulPoint key thật).
- Không sửa `executor.rs`/`pipeline.rs`/`calldata.rs`/`pool.rs` — `relay.rs`
  đứng module riêng, CHƯA nối vào live loop/pipeline nào.
- Không đổi cờ live, không đổi stack, không đổi pair khỏi WBNB, không hồi
  sinh chiều victim bán.
- Không tách test/docs ra phiên sau (đã làm đủ trong phiên này).

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- Hình dạng `params` (bọc trong mảng `[bundle_object]`) là lựa chọn kỹ thuật
  của Claude dựa theo quy ước `eth_sendBundle` kiểu Flashbots phổ biến, KHÔNG
  được lệnh gốc nói rõ và CHƯA verify bằng cURL thật tới endpoint thật — cần
  lệnh Grok riêng cho phép gọi HTTP thật (an toàn, ví dụ request lỗi có chủ
  đích để xem relay phản hồi field gì) trước khi tin tưởng field này 100%.
- `current_block` là tham số caller tự truyền (hàm build thuần, không gọi
  `eth_blockNumber`) — chưa có nơi nào trong repo tự lấy block thật rồi gọi
  `build_48club_send_bundle_request`/`build_blockrazor_send_mev_bundle_request`.
- `relay.rs` hoàn toàn CHƯA nối vào `pipeline.rs`/`executor.rs` — muốn dùng
  thật cần: (1) signer thật ký `front_raw_hex`/`back_raw_hex` thật (nợ từ
  `7.1`, `load_signer` mới trả `B256` chưa ký được), (2) HTTP client thật +
  xử lý response/lỗi relay thật (chưa có crate nào), (3) lệnh Grok riêng cân
  nhắc rủi ro tiền thật rõ ràng trước khi bật, (4) verify field bằng cURL thật.
- Mọi nợ khác giữ nguyên như `docs/TASKS.md` đã liệt kê trước phiên này —
  không có nợ mới nào phát sinh ngoài các mục trên.
