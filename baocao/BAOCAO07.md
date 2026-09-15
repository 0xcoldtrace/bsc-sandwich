1. LÁT: tax-cache-inject — API + file điền `tax_cache` lúc bot đang chạy
(POST /api/tax, state/tax_inject.jsonl, GET /api/tax + bảng web), config
`allow_tax_inject` (field mới, hot-reload), 1 test inject 0,0 → ra khỏi
honeypot_or_tax, cargo test xanh.

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md mục Tax, src/tax.rs, src/main.rs, src/pipeline.rs, baocao/BAOCAO06.md
Gói 5.1 ĐẠT có điều kiện (Grok): inject OK, pending WSS chưa chứng minh.

LÁT: tax cache vận hành — chủ/bot điền được lúc chạy, không phá luật "chưa đo thì skip".
stack = Rust.

ĐƯỢC ĐỤNG: src/tax.rs, src/main.rs, src/web.rs, src/pipeline.rs nếu cần, config.toml field mới, docs/*, baocao/BAOCAO07.md
CẤM: CLAUDE.md trừ 2 dòng field mới, sendRaw, bật live, bịa tax=0 hàng loạt

LÀM:
1) POST /api/tax {token, buy_tax, sell_tax} hoặc file state/tax_inject.jsonl
   token,buy_bps,sell_bps  (0,0 = zero-tax đã đo tay)
   ghi cache + measured_block hiện tại. Log tax.inject
2) Config allow_tax_inject=true (ship true paper). false thì API/file bỏ qua.
3) Không tự gọi measure_roundtrip_via_router trong loop (đã biết không bắt FoT).
4) Web: bảng cache token + bps + block.
5) Test: inject 0,0 → decide_paper cùng token ra Simulated (hoặc unprofitable), không honeypot_or_tax.
6) cargo test xanh.

KHÔNG LÀM: 7.x, Infinity PoolKey.

ĐẠT CẦN DÁN: cargo test ≥15 dòng; 1 inject tax 0,0 → reason khác honeypot_or_tax.

VIẾT: baocao/BAOCAO07.md đủ 10 ô. CHỜ GROK. Cấm ĐẠT.
```

3. FILE ĐỔI:
- `src/config.rs` — thêm field bắt buộc `allow_tax_inject: bool` (không
  `#[serde(default)]`, đúng luật "thiếu field = fail load"), không đưa vào
  mảng `checks` (chỉ 5 field ngưỡng BNB float mới validate âm/hữu hạn — đây
  là bool, không áp dụng). Cập nhật `base_toml()` test fixture + test mới
  `missing_allow_tax_inject_fails` (khuôn y hệt `missing_min_profit_bnb_fails`),
  `load_ok` assert thêm `cfg.allow_tax_inject`.
- `src/tax.rs` — thêm `use std::str::FromStr;` ở top-level (trước chỉ có
  trong `#[cfg(test)]`). Thêm 3 hàm/method mới:
  - `combine_roundtrip_bps(buy_bps, sell_bps) -> u32` — công thức tổn thất
    kép `1-(1-buy)(1-sell) = buy+sell-buy*sell` (KHÔNG cộng đơn giản, tránh
    đếm trùng phần giao 2 lần tax), `u128` trung gian + `saturating_sub` tự
    vệ input tay gõ vượt 100% (không panic do tràn số không dấu).
  - `TaxCache::inject_from_buy_sell_bps(token, buy_bps, sell_bps,
    measured_at_block)` — điểm GHI duy nhất, dùng chung bởi `web.rs` (API)
    VÀ `main.rs` (file watcher), tránh 2 nơi tính `combine_roundtrip_bps`
    khác nhau.
  - `TaxCache::entries() -> impl Iterator<Item=(&Address,&TaxMeasurement)>`
    — cùng khuôn `VictimBook::entries`, phục vụ bảng web.
  - `parse_tax_inject_line(line) -> Result<(Address,u32,u32), String>` —
    parse `token,buy_bps,sell_bps` (CSV, cùng khuôn
    `transport::parse_inject_line`). 8 test mới (combine x3, parse x4,
    inject/entries x2).
- `src/web.rs` — thêm `use alloy::primitives::Address;` +
  `use std::str::FromStr;`. Route mới `GET/POST /api/tax`:
  - `tax_cache_list` (GET) — trả `{allow_tax_inject, current_block,
    tax_cache_blocks, entries:[{token,roundtrip_tax_bps,measured_at_block,fresh}]}`.
  - `tax_inject` (POST, body `{token, buy_tax, sell_tax}` — ĐƠN VỊ basis
    point, xem giải thích quyết định ở `docs/STATE.md`) — kiểm
    `cfg.allow_tax_inject` trước, `false` → `{"ok":false,"error":...}`
    KHÔNG ghi cache; `true` → gọi `inject_from_buy_sell_bps`, log
    `tax.inject` (`source:"api"`).
- `src/main.rs` — thêm hàm nền `watch_tax_inject_file` (cùng khuôn
  `watch_inject_file` đã có từ `5.1`: poll `state/tax_inject.jsonl` mỗi 2s,
  chỉ đọc dòng MỚI, file ngắn hơn đọc lại từ đầu, không panic) — đọc lại
  `cfg.allow_tax_inject` MỖI dòng (field hot-reload, không cache đầu vòng
  lặp); `false` → log `tax.inject_skipped`, KHÔNG ghi cache; `true` → gọi
  `inject_from_buy_sell_bps`, log `tax.inject` (`source:"file"`). Spawn task
  này ngay sau `watch_inject_file` trong `main()`.
- `config.toml` — thêm `allow_tax_inject = true` (ship paper) kèm comment
  giải thích cổng này KHÔNG liên quan cổng live.
- `src/pipeline.rs` — thêm 1 test
  `victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero` dùng ĐÚNG
  `TaxCache::inject_from_buy_sell_bps(token, 0, 0, block)` (không `insert`
  tay như test cũ `4.1`) để chứng minh đường đi THẬT `POST /api/tax`/file sẽ
  dùng — cùng fixture Case A, kết quả PHẢI `Simulated`, panic nếu
  `HoneypotOrTax`. Cập nhật `test_config_toml()` thêm dòng
  `allow_tax_inject = true`.
- `CLAUDE.md` — ĐÚNG 2 dòng field mới (mục "Config — thiếu field = fail
  load"): thêm `allow_tax_inject` vào cuối danh sách field bắt buộc, thêm
  `allow_tax_inject=true` vào dòng Ship. Không sửa gì khác trong file.
- `web/index.html` — thêm section "Tax cache (inject thủ công)": bảng
  token/roundtrip_tax_bps/measured_at_block/fresh?, chú thích sửa qua
  API/file (không sửa trên web, đúng quy ước victims).
- `web/app.js` — `renderTaxCache(data)` + gọi trong `refresh()`
  (`GET /api/tax`).
- `docs/STATE.md` — thêm mục "Tax cache inject" (kiến trúc, quyết định đơn
  vị bps cho cả API lẫn file, verify runtime thật).
- `docs/TASKS.md` — thêm dòng roadmap "Tax cache inject" XONG, cập nhật mục
  Nợ (tách phần `5.1` không còn nhắc "chưa có cách chủ tự điền cache" —
  chuyện đó ĐÃ xong ở cụm này — thêm bullet riêng cho tax-cache-inject nhắc
  rõ VẪN CHƯA đo tax tự động, VẪN CÒN NỢ hợp đồng probe).
- KHÔNG đụng: `src/executor.rs` (không `send_raw_transaction`), `src/pool.rs`
  (không đụng V4/Infinity `PoolKey`), `victims.txt`/`.env` thật, cờ
  `allow_live`/`bot_armed`/`dry_run`.

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero -- --nocapture
```
Runtime smoke test THẬT (dùng `.env` thật của chủ — chỉ ĐỌC `BSC_HTTP`, KHÔNG
sửa `.env`/`config.toml`/`victims.txt` thật; config/victims scratch NGOÀI
repo, port khác `8787`; `state/`/`logs/` là thư mục gitignored thật của repo,
đúng tiền lệ BAOCAO05/06):
```
export $(grep -m1 '^BSC_HTTP=' .env)
target/debug/bsc_sandwich.exe <scratch_config.toml, web_port=18791> &
curl http://127.0.0.1:18791/api/tax                                   # rong
curl -X POST http://127.0.0.1:18791/api/tax -d '{"token":"0xc...","buy_tax":0,"sell_tax":0}'
curl http://127.0.0.1:18791/api/tax                                   # co 1 dong
# sua allow_tax_inject=false trong config scratch, doi config_reload_sec
curl -X POST http://127.0.0.1:18791/api/tax -d '...'                  # ok:false
echo "0x...,25,25" >> state/tax_inject.jsonl                          # sau khi bat lai true
curl http://127.0.0.1:18791/api/tax
```

5. OUTPUT THẬT:

`cargo test` (108 passed, 2 ignored — 2 ignored cần RPC sống như cũ, +11 test
mới so BAOCAO06 [1 config + 8 tax + 1 pipeline, khớp đúng số hàm/nhánh mới],
không giảm test nào; ≥15 dòng cuối thật):
```
running 110 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test decoder::tests::decode_too_short_calldata_is_decode_fail ... ok
test config::tests::missing_allow_tax_inject_fails ... ok
test config::tests::load_ok ... ok
test tax::tests::combine_roundtrip_bps_zero_zero_is_zero ... ok
test tax::tests::combine_roundtrip_bps_accounts_for_double_counted_cross_term ... ok
test tax::tests::combine_roundtrip_bps_garbage_input_over_100_percent_no_panic ... ok
test tax::tests::parse_tax_inject_line_valid ... ok
test tax::tests::parse_tax_inject_line_rejects_wrong_field_count ... ok
test tax::tests::parse_tax_inject_line_rejects_bad_token ... ok
test tax::tests::parse_tax_inject_line_rejects_bad_bps ... ok
test tax::tests::inject_from_buy_sell_bps_zero_zero_is_fresh_immediately ... ok
test tax::tests::entries_lists_all_injected_tokens ... ok
test pipeline::tests::victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero ... ok
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ignored
test sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt ... ignored
(... 94 test còn lại của config/decoder/executor/logger/pool/sim_v2/sim_v3/
state/tax/transport/venues/victims/pipeline đều pass, không lặp lại hết cho
gọn — xem `cargo test` chạy trực tiếp để thấy đủ 110 dòng)

test result: ok. 108 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero -- --nocapture`:
```
running 1 test
test pipeline::tests::victim_a_reaches_sim_after_inject_from_buy_sell_bps_zero_zero ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out; finished in 0.00s
```
Test này dùng ĐÚNG `TaxCache::inject_from_buy_sell_bps(token, 0, 0, 995)`
(đường đi thật `POST /api/tax`/file sẽ gọi) trên fixture Case A (`4.1`) — kết
quả `PipelineOutcome::Simulated(_)`, `panic!` nếu rơi lại `HoneypotOrTax`.

Runtime THẬT (binary chạy, KHÔNG phải unit test — `BSC_HTTP` thật từ `.env`
chủ, port scratch `18791`, không đụng file thật của repo):
```
GET /api/tax (truoc inject):
{"allow_tax_inject":true,"current_block":121850629,"entries":[],"tax_cache_blocks":30}

POST /api/tax {"token":"0xcccc...cccc","buy_tax":0,"sell_tax":0}:
{"measured_at_block":121850629,"ok":true,"token":"0xcccc...cccc"}

GET /api/tax (sau inject):
{"allow_tax_inject":true,"current_block":121850629,
 "entries":[{"fresh":true,"measured_at_block":121850629,
   "roundtrip_tax_bps":0,"token":"0xcccc...cccc"}],"tax_cache_blocks":30}
```
Đổi `allow_tax_inject = true -> false` trong config scratch, đợi qua
`config_reload_sec=15` (hot-reload runtime, không restart):
```
POST /api/tax {"token":"0xdddd...dddd","buy_tax":10,"sell_tax":10}:
{"error":"allow_tax_inject=false trong config.toml, API bi bo qua","ok":false}
```
`logs/bot.jsonl` (rút gọn, chứng minh cả 2 đường API/file):
```
{"buy_bps":0,"event":"tax.inject","measured_at_block":121850629,"sell_bps":0,
 "source":"api","token":"0xcccc...cccc", ...}
{"event":"tax.inject_skipped","reason":"allow_tax_inject=false",
 "token":"0xeeee...eeee", ...}    # dong ghi luc allow_tax_inject=false, bi bo qua
{"buy_bps":25,"event":"tax.inject","measured_at_block":121850629,"sell_bps":25,
 "source":"file","token":"0xffff...ffff", ...}   # dong ghi SAU khi bat lai true
```
Dòng `0xeeee...eeee` (ghi lúc `allow_tax_inject=false`) KHÔNG có trong
`GET /api/tax` sau đó dù đã bật lại cờ — đúng thiết kế "đọc lúc false thì bỏ
VĨNH VIỄN dòng đó, không tự re-apply" (ghi rõ ô 10). Dòng `0xffff...ffff`
(ghi SAU khi bật lại) xuất hiện với `roundtrip_tax_bps=50` — khớp
`combine_roundtrip_bps(25,25)=50` (công thức tổn thất kép, không phải cộng
đơn giản `50`... ở đây trùng số vì `25*25/10000=0` làm tròn xuống, xem test
`combine_roundtrip_bps_accounts_for_double_counted_cross_term` với input lớn
hơn `100,100` mới thấy rõ khác biệt `199` so với cộng đơn giản `200`). Đã kill
process sau khi verify (qua `Stop-Process`, PowerShell — `kill` bash không
đúng PID thật trên Windows/MSYS, ghi lại để phiên sau khỏi mất công dò lại),
xoá `state/tax_inject.jsonl` test khỏi repo (file gitignored, không vào git).

6. CHAIN: `0x38` xác nhận qua `eth_chainId` thật lúc `connect_and_verify` +
`eth_blockNumber` thật (`last_block=121850629`, số THẬT lấy từ
`bsc-dataseed1.bnbchain.org` qua `.env` chủ). Cụm này KHÔNG thêm `eth_call`
nào mới (không resolve pool/reserve mới) — `TaxCache` là cấu trúc in-memory
thuần Rust, không có on-chain call nào trong `inject_from_buy_sell_bps`/
`combine_roundtrip_bps`/`parse_tax_inject_line`. `measured_at_block` lấy từ
`app_state.last_block` đã có sẵn từ `2.1`/`5.1` (WSS heads hoặc
`eth_blockNumber` lúc boot), không bịa số block.

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không pin
address mới, không thêm contract nào).

8. KHÔNG LÀM: không có `7.x`/executor gửi tx thật (`send_raw_transaction` vẫn
không tồn tại trong repo); không đụng V4/Infinity `PoolKey` (`pool.rs` không
đổi, vẫn `hooks_unread` mọi token); không tự gọi
`measure_roundtrip_via_router` trong bất kỳ vòng lặp nào (đúng lệnh "đã biết
không bắt FoT" — hàm này vẫn CHỈ gọi được thủ công/test, quyết định `3.3`
giữ nguyên); không bịa `tax=0` hàng loạt (mọi giá trị inject trong test/verify
runtime đều do lệnh chủ chỉ định rõ, `combine_roundtrip_bps` không tự gán mặc
định `0` cho token không có dòng — `TaxCache::get_fresh` vẫn trả `None` cho
token chưa từng inject, `honeypot_or_tax` vẫn là default); không bật
`allow_live`/`dry_run=false`/`bot_armed`; không sửa `.env`/`victims.txt`
thật; không sửa `CLAUDE.md` ngoài đúng 2 dòng field mới đã liệt kê ở ô 3.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `measure_roundtrip_via_router` VẪN không đo được fee-on-transfer thật (giới
  hạn toán học đã chứng minh từ `3.3`, không đổi ở cụm này) — cụm này chỉ mở
  đường chủ/tool NGOÀI bot tự đo rồi điền tay/API, KHÔNG giải quyết bài toán
  "đo tax thật" gốc. Cần hợp đồng "probe" 1 `eth_call` (kỹ thuật
  honeypot-detector chuẩn) nếu chủ muốn tự động hoá, vẫn NGOÀI PHẠM VI mọi
  phiên tới giờ.
- 1 dòng `state/tax_inject.jsonl` đọc được LÚC `allow_tax_inject=false` bị bỏ
  qua VĨNH VIỄN (không tự re-apply khi cờ bật lại sau đó, vì
  `watch_tax_inject_file` chỉ đọc dòng MỚI theo `last_len`, không nhớ lại
  dòng cũ bị skip) — verify runtime thật đã xác nhận đúng hành vi này (ô 5),
  nhưng có thể gây nhầm lẫn nếu chủ không để ý ("tôi đã ghi dòng rồi sao
  không thấy cache?" — vì ghi lúc cờ tắt). Có thể cần UX rõ hơn (vd log
  warning nổi bật hơn, hoặc buffer lại dòng bị skip để tự áp dụng khi cờ bật)
  nếu chủ thấy phiền — chưa làm, chỉ ghi nhận.
- `TaxInjectBody`/file KHÔNG validate `buy_bps`/`sell_bps` có hợp lý không
  (vd > 10_000 = > 100%, dữ liệu tay gõ sai) — `combine_roundtrip_bps` chỉ tự
  vệ KHÔNG PANIC (`saturating_sub`), không từ chối/cảnh báo input vô lý như
  vậy. Cache vẫn ghi số `roundtrip_tax_bps` bị clamp bởi phép tính (không
  phải bị chặn ở tầng input) — nếu chủ cần validate chặt hơn (từ chối
  `buy_bps > 10_000`), cần cụm riêng.
- Pending THẬT qua WSS vẫn CHƯA được node BSC thật xác nhận (kế thừa từ
  BAOCAO06, `.env` chủ vẫn chưa điền `BSC_WS` — không đổi ở cụm này).
- `resolve_v2_reserves` vẫn gộp mọi lỗi RPC + "không có pool" thành `no_pool`
  (kế thừa từ BAOCAO06, không đổi ở cụm này).
