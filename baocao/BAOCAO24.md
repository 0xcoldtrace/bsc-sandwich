# BAOCAO24

## 1. LÁT

`relay-finalize` — gộp 3 việc không liên quan code logic:
(A) `club48-endpoint-fix` — sửa `CLUB48_RPC_URL` sang endpoint đúng đã verify
cURL thật; (B) `private-relay-personal-url-test` — test 3 URL cá nhân của Chủ
trong `PRIVATE_TX_URL` (`.env`), redact khi báo cáo; (C)
`config-comment-diacritics` — toàn bộ comment `config.toml` sang tiếng Việt
có dấu, không đổi value.

## 2. LỆNH NHẬN

```
ĐỌC: CLAUDE.md, docs/STATE.md (mục "relay-schema-verify-verify"), docs/TASKS.md,
      baocao/BAOCAO23.md, src/relay.rs, config.toml, .env (chỉ đọc, không in
      nguyên văn ra bất kỳ đâu).

LÁT: relay-finalize (gộp 3 việc, đúng tinh thần "một phiên = một cụm",
     không liên quan code logic nên gộp an toàn):

     (A) club48-endpoint-fix — sửa CLUB48_RPC_URL trong src/relay.rs từ
     https://rpc.48.club (SAI, đã xác nhận -32601 ở BAOCAO23) sang
     https://puissant-builder.48.club/ (Puissant Builder thật). Nguồn:
     https://docs.48.club/puissant-builder (đọc 2026-09-15). Verify bằng
     curl payload rác txs: ["0xdeadbeef"] (như BAOCAO23), tối đa 2 lần gọi.

     (B) private-relay-personal-url-test — NẾU .env có field PRIVATE_TX_URL
     (danh sách phẩy, có thể chưa có — nếu thiếu thì ghi MISSING, KHÔNG fail
     cả lệnh): với MỖI URL trong đó, gọi curl payload rác giống (A) để xem
     endpoint cá nhân của Chủ có nhận đúng method bundle không (so với public
     endpoint). KHÔNG log/in/ghi nguyên văn URL cá nhân vào BẤT KỲ đâu — khi
     dán request/response vào BAOCAO, PHẢI thay phần định danh riêng bằng
     ***REDACTED***, chỉ giữ phần domain gốc. Tối đa 1 lần gọi/URL. KHÔNG lưu
     URL cá nhân vào bất kỳ file nào trong repo.

     (C) config-comment-diacritics — sửa TOÀN BỘ comment (dòng bắt đầu #)
     trong config.toml sang tiếng Việt có dấu đầy đủ, KHÔNG đổi bất kỳ
     key=value/thứ tự field nào. Nội dung thay thế CHÍNH XÁC theo bản lệnh
     Grok đưa (6 khối comment: 5.2, 5.3, tax-cache-inject, 7.3, pair-mode,
     universal-pair-scan, explicit-mode-flags).

ĐƯỢC ĐỤNG: src/relay.rs (CHỈ CLUB48_RPC_URL + doc-comment liên quan),
           config.toml (CHỈ dòng #, không đổi value nào), docs/STATE.md,
           docs/TASKS.md, baocao/BAOCAO24.md

CẤM: CLAUDE.md, victims.txt thật, .env (CHỈ ĐỌC), DEX_REGISTRY.md, bật cờ
     live, ký tx thật/dùng PRIVATE_KEY thật, gửi raw tx hex THẬT, thêm field
     48spSign, thêm crate HTTP vào Cargo.toml, sửa BLOCKRAZOR_RPC_URL, sửa
     shape params, đổi bất kỳ key=value nào trong config.toml, sửa
     executor.rs/calldata.rs/pool.rs/pipeline.rs/main.rs, LỘ URL cá nhân Chủ
     nguyên văn ở bất kỳ đâu.

ĐẠT CẦN DÁN: (A)+(B) request/response thật đã redact, (C) file mới, cargo
test + cargo build --release, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO24.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

## 3. FILE ĐỔI

- `src/relay.rs`:
  - Đổi `CLUB48_RPC_URL` từ `"https://rpc.48.club"` sang
    `"https://puissant-builder.48.club/"` (dòng khai báo hằng số), thêm
    doc-comment giải thích lý do đổi (nguồn + kết quả cURL thật).
  - Sửa doc-comment module (mục "48 Club") ghi lại endpoint đúng, tham chiếu
    xuống doc-comment tại `CLUB48_RPC_URL`.
  - Thêm 1 test mới `club48_rpc_url_pinned_to_verified_puissant_builder_endpoint`
    pin giá trị hằng số mới (không sửa/xoá test cũ nào).
- `config.toml` — sửa 7 khối comment (dòng bắt đầu `#`) sang tiếng Việt có
  dấu đầy đủ: cụm `5.2`, `5.3`, `tax-cache-inject`, `7.3`, `pair-mode`,
  `universal-pair-scan`, `explicit-mode-flags`. KHÔNG đổi 1 `key=value` nào
  (xác nhận bằng cách so chuỗi giá trị trước/sau — không lệch).
- `docs/STATE.md` — thêm mục mới `## relay-finalize` (giữa `relay-schema-verify`
  và `explicit-mode-flags`), ghi đầy đủ (A) verify endpoint mới + (B) kết quả
  test 3 `PRIVATE_TX_URL` cá nhân (đã redact) + kết luận.
- `docs/TASKS.md` — sửa dòng bảng `relay-bundle-builder` (thêm BAOCAO24),
  thêm dòng bảng mới `relay-finalize` (XONG), sửa mục nợ
  `relay-schema-verify` thành ĐÃ ĐÓNG + thêm nợ mới phát sinh từ (B).
- `baocao/BAOCAO24.md` — file này.

KHÔNG đụng: `CLAUDE.md`, `victims.txt` thật, `.env` (chỉ đọc qua biến môi
trường lúc chạy, không sửa/không commit), `DEX_REGISTRY.md`, `Cargo.toml`,
`executor.rs`, `calldata.rs`, `pool.rs`, `pipeline.rs`, `main.rs`, không đổi
cờ live, không ký/gửi tx thật.

## 4. LỆNH CHẠY

```
# (A) verify endpoint mới
curl -sS -i -X POST https://puissant-builder.48.club/ -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_sendBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}'
curl -sS -i -X POST https://puissant-builder.48.club/ -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_chainId","params":[]}'

# (B) test 3 URL cá nhân từ .env (script tạm trong scratchpad ngoài repo,
# tự redact danh tính riêng trước khi in ra bất kỳ đâu)
bash probe_private_relay.sh   # đọc PRIVATE_TX_URL từ .env, curl 1 lần/URL

# (C) diff comment
grep -n "^#" config.toml

cargo test
cargo build --release
```

## 5. OUTPUT THẬT

### (A) club48-endpoint-fix — cURL thật tới endpoint mới

Lần gọi 1/2 (`eth_sendBundle` rác):
```
$ curl -sS -i -X POST https://puissant-builder.48.club/ -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_sendBundle","params":[{"maxBlockNumber":12345778,"txs":["0xdeadbeef"]}]}'

HTTP/1.1 200 OK
content-type: application/json
vary: Origin
date: Mon, 14 Sep 2026 20:05:57 GMT
content-length: 108

{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"rlp: value size exceeds available input length"}}
```

Lần gọi 2/2 (`eth_chainId` sanity check):
```
$ curl -sS -i -X POST https://puissant-builder.48.club/ -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"eth_chainId","params":[]}'

HTTP/1.1 200 OK
content-type: application/json
vary: Origin
date: Mon, 14 Sep 2026 20:06:08 GMT
content-length: 41

{"jsonrpc":"2.0","id":1,"result":"0x38"}
```

**Đối chiếu với giả định cũ**: endpoint cũ `https://rpc.48.club` trả
`-32601 method eth_sendBundle does not exist/is not available` (BAOCAO23) —
lỗi Ở TẦNG METHOD. Endpoint mới trả `-32000 rlp: value size exceeds available
input length` — lỗi NỘI DUNG (server đã chấp nhận method `eth_sendBundle`,
đã parse đúng `params[0].txs`, cố decode chuỗi rác `"0xdeadbeef"` như RLP và
thất bại vì đó không phải bytes RLP hợp lệ). Đây là bằng chứng RÕ RÀNG khác
loại lỗi — xác nhận endpoint mới ĐÚNG là route submit bundle Puissant
Builder. `eth_chainId` → `0x38` xác nhận đúng chain 56.

### (B) private-relay-personal-url-test — 3 URL trong `PRIVATE_TX_URL` (đã redact)

`.env` có field `PRIVATE_TX_URL` (không rỗng — KHÔNG phải MISSING), 3 URL
phẩy. Test mỗi URL 1 lần bằng cùng payload rác `eth_sendBundle`/
`txs:["0xdeadbeef"]` ở trên. Danh tính riêng (subdomain hash/rpc id) thay
`***REDACTED***`, chỉ giữ domain gốc:

**URL 1** (redacted: `https://***REDACTED***.rpc.48.club`):
```
HTTP/1.1 200 OK
Content-Type: application/json
X-Node: RPC-SG-2
X-Powered-By: https://x.com/48club_official
Server: cloudflare

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_sendBundle does not exist/is not available"}}
```

**URL 2** (redacted: `https://***REDACTED***.bsc-rpc.com`):
```
HTTP/1.1 200 OK
Content-Type: application/json
X-Node: RPC-SG-2
X-Powered-By: https://x.com/48club_official
Server: cloudflare

{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"the method eth_sendBundle does not exist/is not available"}}
```

**URL 3** (redacted: `https://bsc.blockrazor.xyz/***REDACTED***`):
```
HTTP/1.1 200 OK
Content-Type: application/json
Server: cloudflare
Vary: Origin

{"jsonrpc":"2.0","id":1,"error":{"code":-38000,"message":"the maxBlockNumber should be lager than currentBlockNum"}}
```

**Kết luận**:
- URL 1, URL 2: cùng lỗi `-32601` như endpoint 48 Club CÔNG KHAI cũ
  (`rpc.48.club`) → đây là endpoint privacy-rpc CÁ NHÂN (gửi tx đơn lẻ,
  KHÔNG phải route submit bundle) — KHÔNG dùng được cho `eth_sendBundle`.
  (URL 2 có domain khác `48.club` nhưng header `X-Powered-By` trả về vẫn
  chỉ tới `48club_official` — cùng hạ tầng, khác domain alias.)
- URL 3: cùng lỗi nội dung `-38000` như endpoint BlockRazor CÔNG KHAI (BAOCAO23)
  → endpoint cá nhân BlockRazor này HỖ TRỢ ĐÚNG `eth_sendMevBundle`, hoạt
  động như public endpoint.
- Tổng kết: 1/3 URL cá nhân (BlockRazor) dùng được cho bundle relay; 2/3
  (kiểu 48club) KHÔNG dùng được — là loại endpoint khác (privacy tx đơn).

### (C) config-comment-diacritics — xác nhận value không đổi, chỉ comment đổi

```
$ grep -n "^#" config.toml
1:# BSC Sandwich Bot — config.toml
2:# Thiếu field bất kỳ = fail load (xem src/config.rs).
3:# Ship mặc định: dry_run, không live, không gửi tx thật.
26:# Cụm 5.2: cadence (ms) fallback poll txpool_content khi BSC_WS trễ/rớt kết
27:# nối (không liên quan victims_reload_sec/config_reload_sec, đó là
28:# ngưỡng hot-reload, đây là cadence đọc mempool qua HTTP).
31:# Cụm 5.3: số hash pending-tx MỚI (chưa xử lý) tối đa mỗi vòng poll
32:# txpool_content — chặn 1 vòng poll spawn quá nhiều handle_paper_tx khi
33:# mempool đông.
51:# Cụm tax-cache-inject: cho phép POST /api/tax + state/tax_inject.jsonl ghi
52:# TaxCache lúc bot đang chạy (KHÔNG liên quan cổng live). false = API/file bị
53:# bỏ qua, giữ nguyên luật "chưa đo -> honeypot_or_tax".
56:# Cụm 7.3 (BAOCAO16): build calldata paper (front-buy/back-sell) từ SandwichQuote.
57:# deadline = wall-clock hiện tại + buffer này (giây) — KHÔNG phải block.timestamp
58:# on-chain thật (executor.rs không có Provider). slippage_bps áp vào amount_out_min
59:# = expected * (10000-bps) / 10000. Cả 2 field CHỈ dùng để build/log calldata paper,
60:# không liên quan cổng live/dry_run.
64:# Cụm pair-mode: PairBook theo dõi POOL (token/WBNB) chỉ định trực tiếp qua
65:# pairs.txt, khác victims.txt (theo dõi ĐỊA CHỈ VÍ). min_swap là NGƯỠNG
66:# GLOBAL áp dụng cho MỌI pool trong pairs.txt (không có min riêng từng pool).
71:# Cụm universal-pair-scan: true = quét MỌI pool WBNB (không chỉ pool khai
72:# trong pairs.txt), dùng LẠI ngưỡng pairs_min_swap_bnb (không thêm ngưỡng
73:# riêng). Default false = AN TOÀN, giữ nguyên hành vi cũ (not_in_list) — Chủ
74:# phải tự bật, tăng tải RPC/CPU (mọi pool WBNB đều thành candidate sim).
77:# Cụm explicit-mode-flags: cờ TƯỜNG MINH bật/tắt từng nhánh candidate trong
78:# decide_paper_v2 (wallet=victims.txt, pair=pairs.txt, universal=mọi pool WBNB
79:# ở trên). Default CẢ HAI = true — đây là hành vi GỐC đang chạy thật, KHÔNG
80:# được đổi thành false (sẽ vô tình TẮT bot đang vận hành). Muốn chỉ dùng ĐÚNG
81:# 1 mode: đặt 2 field còn lại (kể cả pair_scan_universal) = false trong file
82:# này — hot-reload theo config_reload_sec, không cần build/restart. Thứ tự ưu
83:# tiên khi nhiều hơn 1 mode bật cùng lúc GIỮ NGUYÊN: wallet > pair > universal
84:# > not_in_list. Code KHÔNG ép buộc chỉ-1-mode — Chủ tự quản lý tổ hợp.
```
`config.toml` không nằm dưới git tracking (untracked từ đầu repo, `git diff`
không ra gì) — dùng `grep -n "^#"` để dán TOÀN BỘ comment đã sửa thay cho
`git diff`. Toàn bộ 24 dòng `key=value` (không phải comment) không đổi 1 ký
tự nào — chỉ 7 khối comment nêu trên bị sửa dấu.

### `cargo test` (≥15 dòng cuối)

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

test result: ok. 206 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.13s

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
206 passed = 205 (BAOCAO23) + 1 test mới (`club48_rpc_url_pinned_to_verified_puissant_builder_endpoint`).

### `cargo build --release`

```
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `release` profile [optimized] target(s) in 22.00s
```

## 6. CHAIN

`0x38` — xác nhận qua `eth_chainId` thật tới endpoint mới
`https://puissant-builder.48.club/` (lần gọi 2/2 mục A): `{"jsonrpc":"2.0","id":1,"result":"0x38"}`
= chain 56 đúng. Không phải `eth_getCode`/pin venue — cụm này không liên
quan AMM/pool Pancake.

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không bị đụng (không nằm trong danh sách ĐƯỢC
ĐỤNG của lệnh này, cụm này không liên quan venue/pool Pancake).

## 8. KHÔNG LÀM

- Không ký tx thật, không dùng `PRIVATE_KEY` nào — mọi payload chỉ có
  `"0xdeadbeef"` rác.
- Không thêm field `48spSign`.
- Không thêm crate HTTP (reqwest/hyper) vào `Cargo.toml` — verify dùng
  `curl` thuần qua Bash.
- Không gọi endpoint mới quá 2 lần (đúng 2 lần: `eth_sendBundle` rác +
  `eth_chainId`). Không gọi mỗi URL cá nhân quá 1 lần (đúng 3 lần, 1 lần/URL).
- Không sửa `BLOCKRAZOR_RPC_URL` (không đụng tới).
- Không sửa shape `params` (`build_48club_send_bundle_request`/
  `build_blockrazor_send_mev_bundle_request` không đổi logic — chỉ hằng số
  URL đổi).
- Không đổi bất kỳ `key=value` nào trong `config.toml` (xác nhận bằng
  `grep`/đối chiếu ô 5).
- Không sửa `executor.rs`/`calldata.rs`/`pool.rs`/`pipeline.rs`/`main.rs`.
- Không sửa `CLAUDE.md`/`victims.txt` thật/`.env`.
- Không lộ URL cá nhân nguyên văn — đã `grep` toàn repo tìm 2 chuỗi định danh
  riêng lấy từ `PRIVATE_TX_URL` (subdomain hash + rpc id số) trước khi viết
  báo cáo này: 0 kết quả ngoài chính `.env` (file gitignored, không phải file
  mới tạo phiên này) — xác nhận không lọt vào `src/`, `docs/`, `baocao/`, hay
  bất kỳ file commit nào khác trong repo. Script tạm dùng để curl (đọc
  `.env`, tự redact trước khi in) chỉ tồn tại trong thư mục scratchpad NGOÀI
  repo, không commit.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **48 Club (endpoint công khai)**: ĐÃ ĐÓNG — `CLUB48_RPC_URL` giờ đúng,
  verify cURL thật xác nhận method + shape `params` đều đúng.
- **48 Club (2 URL cá nhân của Chủ trong `PRIVATE_TX_URL`)**: KHÔNG hỗ trợ
  `eth_sendBundle` — đây là loại endpoint privacy-rpc cá nhân (gửi tx đơn lẻ
  riêng tư), khác route Puissant Builder submit bundle. Nếu Chủ muốn submit
  bundle qua tài khoản 48 Club cá nhân (có thể có ưu đãi/whitelist riêng),
  cần Chủ tự hỏi 48 Club support về endpoint/route cá nhân đúng cho
  `eth_sendBundle` — Code không bịa/đoán URL cá nhân mới.
- `src/relay.rs` vẫn CHƯA nối vào `pipeline.rs`/`executor.rs`, chưa có
  signer/HTTP client thật — không đổi so với nợ cũ ở BAOCAO21/BAOCAO23.
  `PRIVATE_TX_URL` (khái niệm private mempool submit tx đơn) và 2 hằng số
  relay bundle trong `relay.rs` là 2 khái niệm KHÔNG trộn lẫn — chưa có code
  nào dùng `PRIVATE_TX_URL` trong `main.rs` hiện tại (ngoài phạm vi phiên
  này, chỉ verify cURL tay).
- Không có nợ mới nào khác phát sinh phiên này ngoài 2 mục trên.
