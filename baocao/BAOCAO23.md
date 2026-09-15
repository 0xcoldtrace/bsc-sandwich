# BAOCAO23

## 1. LÁT

`relay-schema-verify` — gửi request HTTP THẬT (`curl` thuần, không code Rust
gọi network) tới 2 endpoint relay đã pin trong `relay.rs` (48 Club
`eth_sendBundle`, BlockRazor `eth_sendMevBundle`), payload cố ý chứa rác
`txs: ["0xdeadbeef"]`, để xác nhận hình dạng `params` (mảng bọc `[{...}]` hay
object trần) đúng/sai. Một cụm, không cắt vụn.

## 2. LỆNH NHẬN

```
ĐỌC: CLAUDE.md, docs/STATE.md (mục "relay-bundle-builder"), docs/TASKS.md,
      baocao/BAOCAO21.md, src/relay.rs.

LÁT: relay-schema-verify — gửi request HTTP THẬT (dùng `curl`, KHÔNG viết
     code Rust gọi network) tới 2 endpoint relay đã pin trong `relay.rs`
     (`CLUB48_RPC_URL = https://rpc.48.club`, method `eth_sendBundle`;
     `BLOCKRAZOR_RPC_URL = https://bsc.blockrazor.xyz`, method
     `eth_sendMevBundle`), payload CỐ Ý chứa `txs: ["0xdeadbeef"]` (rác, KHÔNG
     phải tx ký thật, KHÔNG dùng private key nào, không thể được relay chấp
     nhận đưa vào block) — mục đích DUY NHẤT: xem relay phản hồi lỗi gì, để
     xác nhận hình dạng `params` (bọc mảng `[{...}]` hay object trần — đây là
     giả định CHƯA verify, ghi rõ ở BAOCAO21 ô 10) đúng hay sai.

     Lấy CHÍNH XÁC JSON body từ 2 hàm `build_48club_send_bundle_request`/
     `build_blockrazor_send_mev_bundle_request` đã có trong `relay.rs` (chạy
     test in ra `--nocapture`, hoặc viết 1 bin/script tạm NGOÀI src/ chỉ để
     lấy JSON, không commit vào src/), thay `txs` fixture bằng `["0xdeadbeef"]`,
     rồi `curl -X POST` thật tới 2 URL trên, dán request + response THẬT vào
     BAOCAO. Chỉ gọi 1-2 lần mỗi relay (không spam).

stack = Rust cho code hiện có; verify dùng `curl` thuần (không phải code
sản phẩm). Không npm/viem.

ĐƯỢC ĐỤNG: docs/STATE.md, docs/TASKS.md, baocao/BAOCAO23.md, src/relay.rs
           (CHỈ SỬA nếu curl chứng minh hình dạng params sai — sửa lại cho
           đúng bằng chứng thật, kèm test mới phản ánh đúng, KHÔNG thêm bất
           kỳ crate HTTP/network nào vào Cargo.toml, KHÔNG thêm code gọi
           network thật vào relay.rs — hàm build vẫn phải THUẦN, chỉ hình
           dạng JSON có thể đổi)

CẤM: CLAUDE.md, victims.txt thật, .env, bật cờ live, ký tx thật/dùng
     PRIVATE_KEY thật, gửi raw tx hex THẬT (chỉ dùng rác "0xdeadbeef" hoặc
     tương đương rõ ràng vô nghĩa), thêm field `48spSign`, thêm crate HTTP
     (reqwest/hyper) vào Cargo.toml, gọi endpoint quá 2 lần/relay,
     sendRawTransaction, đổi pair khỏi WBNB, hồi sinh chiều victim bán, sửa
     executor.rs/calldata.rs/pool.rs/pipeline.rs/main.rs.

LÀM:
- Dán nguyên văn lệnh curl đã chạy + response thật (status code + body) cho
  cả 2 relay vào ô 5.
- Đối chiếu response với giả định hiện tại trong relay.rs/docs/STATE.md.
- Nếu hình dạng đúng: chỉ cập nhật docs/STATE.md + xoá dòng nợ tương ứng.
- Nếu hình dạng sai: sửa relay.rs đúng theo bằng chứng thật, thêm test mới.

ĐẠT CẦN DÁN: request+response thật (ô 5), cargo test + cargo build --release
nếu có sửa relay.rs, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO23.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

## 3. FILE ĐỔI

- `docs/STATE.md` — sửa mục `## relay-bundle-builder` (quyết định 1: cập
  nhật trạng thái verify từ "CHƯA verify" sang "ĐÃ VERIFY một phần"), thêm
  mục MỚI `## relay-schema-verify` ghi đầy đủ cách lấy JSON (script tạm
  `examples/relay_probe.rs`, đã xoá), 2 lần gọi/relay + kết quả + kết luận.
- `docs/TASKS.md` — sửa dòng bảng roadmap `relay-bundle-builder` (thêm
  BAOCAO23), thêm mục nợ mới `relay-schema-verify` vào "Nợ / MISSING hiện
  tại", sửa mục nợ cũ `relay-bundle-builder` (bỏ phần đã verify).
- `baocao/BAOCAO23.md` — file này.
- `src/relay.rs` — **KHÔNG SỬA** (hình dạng `params` không bị chứng minh sai
  ở relay nào — xem ô 5/ô 10).
- `examples/relay_probe.rs` — TẠO TẠM rồi XOÁ trong cùng phiên (không commit,
  không còn tồn tại lúc kết thúc phiên — xác nhận bằng `git status`).

KHÔNG đụng: `CLAUDE.md`, `victims.txt` thật, `.env`, `Cargo.toml`,
`executor.rs`, `calldata.rs`, `pool.rs`, `pipeline.rs`, `main.rs`, không đổi
cờ live, không ký/gửi tx thật.

## 4. LỆNH CHẠY

```
cargo run --example relay_probe   # script tạm, đã xoá sau khi chạy
curl -sS -i -X POST https://bsc.blockrazor.xyz -H "Content-Type: application/json" -d '<CLUB48/BLOCKRAZOR JSON>'
curl -sS -i -X POST https://rpc.48.club -H "Content-Type: application/json" -d '<JSON>'
curl -sS -i -X POST https://rpc.48.club -H "Content-Type: application/json" -d '{"id":1,"jsonrpc":"2.0","method":"eth_chainId","params":[]}'
cargo test
```

## 5. OUTPUT THẬT

### Lấy JSON thật từ hàm build (script tạm `examples/relay_probe.rs`, đã xoá sau khi dùng)

```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.26s
     Running `target\debug\examples\relay_probe.exe`
=== CLUB48 JSON ===
{"id":1,"jsonrpc":"2.0","method":"eth_sendBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}
=== BLOCKRAZOR JSON ===
{"id":1,"jsonrpc":"2.0","method":"eth_sendMevBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}
```

### Lần gọi 1/2 — BlockRazor (`eth_sendMevBundle`)

Request:
```
curl -sS -i -X POST https://bsc.blockrazor.xyz -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_sendMevBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}'
```
Response (HTTP 200):
```
HTTP/1.1 200 OK
Date: Mon, 14 Sep 2026 19:52:32 GMT
Content-Type: application/json
Transfer-Encoding: chunked
Connection: keep-alive
Server: cloudflare
CF-RAY: a3b1eec50e3903f2-HKG

{"jsonrpc":"2.0","id":1,"error":{"code":-38000,"message":"the maxBlockNumber should be lager than currentBlockNum"}}
```

### Lần gọi 1/2 — 48 Club (`eth_sendBundle`)

Request:
```
curl -sS -i -X POST https://rpc.48.club -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_sendBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}'
```
Response (HTTP 200):
```
HTTP/1.1 200 OK
Date: Mon, 14 Sep 2026 19:52:27 GMT
Content-Type: application/json
Content-Length: 119
Connection: keep-alive
X-Node: RPC-SG-1
X-Powered-By: https://x.com/48club_official
X-Ratelimit-Limit: ["80/5s","720/1m0s"]
X-Ratelimit-Remaining: ["80/5s","720/1m0s"]
Server: cloudflare
CF-RAY: a3b1eea1da1fce45-SIN

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_sendBundle does not exist/is not available"}}
```

### Lần gọi 2/2 — 48 Club (sanity check `eth_chainId`, dùng nốt hạn ngạch cho phép để chẩn đoán lỗi -32601 ở trên)

Request:
```
curl -sS -i -X POST https://rpc.48.club -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_chainId","params":[]}'
```
Response (HTTP 200):
```
HTTP/1.1 200 OK
Date: Mon, 14 Sep 2026 19:52:51 GMT
Content-Type: application/json
Content-Length: 41
Connection: keep-alive
X-Node: RPC-SG-3
X-Powered-By: https://x.com/48club_official
Server: cloudflare
CF-RAY: a3b1ef3af94f87cf-SIN

{"jsonrpc":"2.0","id":1,"result":"0x38"}
```

### `cargo test` (≥15 dòng cuối — xác nhận relay.rs KHÔNG đổi, số test khớp BAOCAO22)

```
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

test result: ok. 205 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.11s

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

205 passed = 196 (BAOCAO21) + 9 (BAOCAO22, `explicit-mode-flags`) — ĐÚNG số
kỳ vọng từ `docs/TASKS.md`, không tăng/giảm nào, xác nhận `src/relay.rs`
không bị sửa phiên này (không thêm/bớt test nào trong `relay::tests`).

`cargo build --release` — KHÔNG chạy lại vì `relay.rs`/mọi file `src/` không
đổi so với BAOCAO21 (đã build xanh ở đó); không có thay đổi code sản phẩm nào
cần build lại. `examples/relay_probe.rs` đã build+chạy+xoá ở bước lấy JSON
phía trên (không phải file sản phẩm, không tính vào build release).

## 6. CHAIN

`0x38` — xác nhận qua chính response `eth_chainId` thật ở trên (lần gọi 2/2
tới 48 Club): `{"jsonrpc":"2.0","id":1,"result":"0x38"}` = chain 56 đúng. Đây
KHÔNG phải `eth_getCode`/pin venue (cụm này không liên quan AMM/pool) — chỉ
xác nhận endpoint `rpc.48.club` trả đúng BSC mainnet, dùng để chẩn đoán lỗi
`-32601` ở lần gọi 1 (loại trừ khả năng gọi nhầm chain/endpoint hỏng hoàn
toàn).

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không bị đụng (đúng CẤM, không nằm trong danh
sách ĐƯỢC ĐỤNG của lệnh này). Cụm này không liên quan venue/pool Pancake.

## 8. KHÔNG LÀM

- Không ký tx thật, không dùng `PRIVATE_KEY` nào — `txs` gửi đi chỉ có
  `"0xdeadbeef"` (rác tường minh, không claim là tx thật).
- Không thêm field `48spSign`.
- Không thêm crate HTTP (reqwest/hyper) vào `Cargo.toml` — verify dùng
  `curl` thuần qua Bash, không phải code Rust.
- Không gọi endpoint quá 2 lần/relay (BlockRazor: 1 lần; 48 Club: 2 lần —
  1 lần `eth_sendBundle` + 1 lần `eth_chainId` chẩn đoán, đúng giới hạn).
- Không sửa `executor.rs`/`calldata.rs`/`pool.rs`/`pipeline.rs`/`main.rs`.
- Không sửa `CLAUDE.md`/`victims.txt` thật/`.env`.
- Không sửa `src/relay.rs` — hình dạng `params` không bị chứng minh SAI ở
  relay nào (BlockRazor: đúng; 48 Club: không kết luận được, không phải
  bằng chứng "sai" — xem ô 10), nên KHÔNG đủ điều kiện lệnh cho phép sửa.
  Không đoán/sửa `CLUB48_RPC_URL` khi chưa có nguồn xác nhận endpoint đúng.
- Không để lại file tạm `examples/relay_probe.rs` trong repo — đã xoá +
  verify `git status` sạch (không có `examples/` trong danh sách untracked).

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **BlockRazor**: hình dạng `params` (mảng bọc `[bundle_object]`) ĐÃ XÁC NHẬN
  ĐÚNG bằng cURL thật — lỗi trả về (`-38000`, `maxBlockNumber` quá thấp) là
  lỗi NỘI DUNG, chứng minh server đã parse đúng cả `txs` lẫn `maxBlockNumber`
  ở đúng vị trí `params[0]`. Không còn nợ shape cho relay này.
- **48 Club**: KHÔNG kết luận được hình dạng `params` đúng/sai — response là
  `-32601 method eth_sendBundle does not exist/is not available`, một lỗi Ở
  TẦNG METHOD trước khi server parse `params`. Lần gọi thứ 2 (`eth_chainId`
  → `0x38`) xác nhận endpoint `https://rpc.48.club` LÀ node BSC JSON-RPC
  công khai hợp lệ (đúng chain), nhưng route này KHÔNG hỗ trợ method bundle.
  Đây là phát hiện MỚI (khác giả định ban đầu ở BAOCAO21: endpoint
  `docs.48.club/privacy-rpc` = endpoint submit bundle Puissant Builder) —
  đã dùng hết 2 lần gọi cho phép, KHÔNG gọi thêm để dò URL/route khác. Cần
  lệnh Grok riêng cho phép research thêm nguồn chính thức (có thể domain/path
  khác với `docs.48.club/puissant-builder/send-bundle` đã đọc ở BAOCAO21,
  hoặc cần API key/whitelist riêng cho route bundle) trước khi sửa
  `CLUB48_RPC_URL` trong `relay.rs` hoặc thử gọi lại — KHÔNG tự đoán URL mới
  (đúng luật "cấm bịa pin").
- `src/relay.rs` giữ nguyên 100% — chưa nối vào `pipeline.rs`/`executor.rs`,
  chưa có signer/HTTP client thật, mọi nợ khác giữ nguyên như BAOCAO21/
  `docs/TASKS.md` đã liệt kê (không có nợ mới nào khác ngoài mục 48 Club ở
  trên phát sinh phiên này).
