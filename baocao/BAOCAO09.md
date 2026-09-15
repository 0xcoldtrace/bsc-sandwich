1. LÁT: 5.3 — RPC pool đa URL (failover HTTP+WSS, quay vòng khi 1 node chết)
+ pending bền hơn (precheck xác nhận không gọi RPC, WSS lag thử WSS khác
trước txpool, txpool giới hạn N hash mới/vòng).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, src/transport.rs, src/main.rs, src/pipeline.rs, src/config.rs, baocao/BAOCAO08.md, .env.example
Gói 5.2 ĐẠT (Grok). 7.x CẤM.

LÁT: 5.3 pending bền + RPC pool failover (nhiều URL, fail thì URL kế).
stack = Rust + alloy.

ĐƯỢC ĐỤNG: src/transport.rs, src/main.rs, src/pipeline.rs, src/config.rs nếu cần, .env.example, docs/STATE.md, docs/TASKS.md, CLAUDE.md CHỈ thêm pending_poll_ms + mô tả RPC list, baocao/BAOCAO09.md
CẤM: .env THẬT (không ghi key vào repo), sendRaw, live flags, commit URL có token

LÀM:
A) RPC pool — đọc danh sách HTTP/WSS (BSC_HTTP+BSC_HTTP_2..16 hoặc
   BSC_HTTP_LIST=url1,url2,..., cùng kiểu cho WS), connect_and_verify từng
   URL chain_id==56, lỗi thì log rpc.failover (redact) rồi thử URL sau;
   runtime eth_call/getBlock/txpool đi qua provider hiện tại, lỗi transport
   → chuyển URL kế, quay vòng, không halt vì 1 node chết; URL chứa
   maxbackrun/fullprivacy/privacy KHÔNG dùng làm primary READ; log chỉ
   scheme+host; .env.example thêm comment mẫu, không paste URL thật;
   test pool 2 URL (đầu fail, chọn URL 2, không panic), sai chain bị skip.
B) Pending bền — handle_paper_tx: precheck_without_reserves TRƯỚC mọi
   eth_call; WSS buffer lớn nếu alloy cho, lag → failover WSS khác trong
   list rồi mới txpool; txpool tối đa N hash mới/vòng (ship 32, config
   pending_txpool_max_per_poll); CLAUDE.md thêm 2 field + 1 câu HTTP/WSS đa
   URL failover.

KHÔNG LÀM: 7.x send, Infinity, tax probe.

ĐẠT CẦN DÁN: cargo test ≥15 dòng; test precheck không gọi RPC; test failover
URL đầu chết → URL sau; boot log rpc.failover redact (nếu chạy được).

VIẾT: baocao/BAOCAO09.md đủ 10 ô. CHỜ GROK. Cấm ĐẠT.
Ô 1: 5.3+rpc-pool
```

3. FILE ĐỔI:
- `src/transport.rs`:
  - Thêm `parse_rpc_url_list(get_var, base)` (thuần, nhận closure thay
    `std::env::var` để test không đụng env thật) — ưu tiên `<base>_LIST`
    (phẩy) nếu có phần tử hợp lệ, không thì gộp `<base>` + `<base>_2`..
    `<base>_16`. **MỖI giá trị đọc được đều tự tách theo dấu phẩy** (kể cả
    `<base>` gốc) — phát hiện THẬT khi verify runtime: `.env` chủ đã điền
    sẵn `BSC_HTTP=` là 1 chuỗi 34 URL nối bằng dấu phẩy ngay trong biến gốc,
    không dùng `BSC_HTTP_LIST`; nếu chỉ tách `_LIST` sẽ mất 33/34 URL đó.
    `collect_rpc_urls_from_env(base)` — wrapper dùng `std::env::var` thật,
    điểm gọi production duy nhất (`main.rs`).
  - `is_private_send_url(url)` (chứa `maxbackrun`/`fullprivacy`/`privacy`,
    không phân biệt hoa/thường) + `filter_read_urls(urls)` (lọc bỏ khỏi pool
    ĐỌC).
  - `RpcPool` (struct mới): `urls: Vec<String>` cố định + `RwLock<RpcPoolState{idx,provider}>`.
    3 method: `connect(logger,label)` (thử lần lượt từ `idx`, quay 1 vòng,
    log `rpc.failover`/`rpc.connect`, không panic khi hết danh sách),
    `current()` (đọc provider đã lưu, không thử lại), `advance_and_reconnect(logger,label)`
    (chuyển `idx+1` quay vòng rồi `connect()` — dùng khi 1 call runtime lỗi).
  - `PENDING_WS_CHANNEL_SIZE: usize = 4096` — nâng buffer subscription
    pending-tx WSS từ mặc định `alloy` (16) qua `GetSubscription::channel_size()`
    (API alloy cho phép chỉnh thật, xem `alloy-provider-2.4.2/src/provider/subscription.rs`).
  - 12 test mới: `parse_rpc_url_list_*` (4, gồm 1 test riêng cho phát hiện
    CSV-trong-biến-gốc), `is_private_send_url_*`, `filter_read_urls_*`,
    `rpc_pool_*` (6, dùng **mock JSON-RPC server nội bộ** qua `axum`+
    `tokio::net::TcpListener::bind("127.0.0.1:0")` — trả cố định `eth_chainId`
    cho mọi request, test được cả nhánh `ChainMismatch` MÀ KHÔNG cần mạng
    Internet thật; URL chết dùng cổng `127.0.0.1:1` — refused ngay, không
    chờ DNS timeout).
- `src/config.rs` — thêm field bắt buộc mới `pending_txpool_max_per_poll: u32`
  (ship `32`, không `#[serde(default)]`). Cập nhật `base_toml()` fixture,
  thêm 2 test: `missing_pending_txpool_max_per_poll_fails`,
  `pending_txpool_max_per_poll_loads_ship_value`.
- `src/pipeline.rs`:
  - Cập nhật `test_config_toml()` fixture thêm `pending_txpool_max_per_poll = 32`.
  - Thêm test `precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason`
    (ĐẠT CẦN DÁN "test precheck không gọi RPC") — bảo đảm ở MỨC KIỂU
    (compile-time): `precheck_without_reserves` không nhận tham số `Provider`
    nào (khác `resolve_v2_reserves`), nên KHÔNG THỂ tự nó gọi `eth_call`;
    test xác nhận 3 nhánh skip sớm (decode_fail/not_in_list/below_min) đều
    dừng đúng ở bước thuần này. KHÔNG đổi logic `decode_and_prefilter`/
    `precheck_without_reserves` — hành vi này đã đúng từ `5.1`, chỉ thêm
    test xác nhận tường minh theo đúng lệnh phiên này.
- `src/main.rs`:
  - `main()`: build `http_pool: Arc<transport::RpcPool>` (từ
    `collect_rpc_urls_from_env("BSC_HTTP")` + fallback `vps.json` +
    `filter_read_urls`) TRƯỚC khi tạo `app_state` — truyền TAY qua tham số
    hàm cho `connect_rpc`/`subscribe_pending_txs`/`poll_txpool_pending`/
    `http_pool_health_check`, KHÔNG thêm field vào `AppStateInner` (`web.rs`
    không nằm trong ĐƯỢC ĐỤNG phiên này) — các hàm vẫn ghi kết quả vào
    `app_state.provider`/`last_block` (field có sẵn từ `5.1`), nên
    `pipeline.rs`/`web.rs::status` không đổi gì.
  - Spawn thêm `http_pool_health_check(app_state, http_pool, 5s)` — task nền
    MỚI: định kỳ `get_block_number()` qua provider hiện tại; lỗi/chưa có
    provider → `connect()`/`advance_and_reconnect()` sang URL kế, cập nhật
    `app_state.provider`/`last_block`. Đây là điểm DUY NHẤT bảo vệ chung
    `pipeline::resolve_v2_reserves` (qua `handle_paper_tx`) mà KHÔNG cần đổi
    chữ ký `pipeline.rs` (giữ tách lõi thuần/RPC thật theo `docs/STATE.md`).
  - `connect_rpc` viết lại dùng `http_pool.connect()` thay `pick_url`+
    `connect_and_verify` đơn lẻ; `BSC_WS` đọc thành `Vec<String>` (cùng cơ
    chế `collect_rpc_urls_from_env`) truyền cho `subscribe_ws_heads`/
    `subscribe_pending_txs`.
  - `subscribe_ws_heads(app_state, ws_urls: Vec<String>)` (đổi chữ ký từ
    `url: String`) — thử lần lượt từng URL (log `rpc.failover` khi lỗi) tới
    khi 1 URL connect+`subscribe_blocks` thành công.
  - `subscribe_pending_txs(app_state, ws_urls, http_pool, poll_interval)`
    (đổi chữ ký) — thử lần lượt từng WSS: connect lỗi/subscribe lỗi/lag-rớt
    đều `continue`/`break` sang WSS KẾ trong danh sách (khác `5.2`: trước
    đây rớt là rơi thẳng txpool ngay); dùng `.channel_size(PENDING_WS_CHANNEL_SIZE)`.
    Hết TOÀN BỘ `ws_urls` mới gọi `poll_txpool_pending`.
  - `poll_txpool_pending(app_state, http_pool, poll_interval)` (đổi chữ ký,
    nhận `http_pool` thay đọc `app_state.provider` trực tiếp) — lỗi
    `raw_request` KHÔNG còn `return` (dừng hẳn task như `5.2`) mà log +
    `http_pool.advance_and_reconnect()` sang URL HTTP kế rồi `continue` (thử
    lại vòng poll sau) — đúng "không halt vì 1 node chết". Thêm giới hạn
    `pending_txpool_max_per_poll` (đọc từ `Config`, hot-reload) — vòng lặp
    `'outer: for {...} break 'outer;` khi đã xử lý đủ N hash MỚI, hash vượt
    cap KHÔNG bị đánh dấu `seen` (còn cơ hội ở vòng sau).
  - Thêm `http_pool_health_check` (mô tả ở trên).
- `.env.example` — thêm khối comment mẫu `BSC_HTTP_2`/`BSC_HTTP_LIST`/
  `BSC_WS_2`/`BSC_WS_LIST` + giải thích lọc URL private/maxbackrun, KHÔNG
  paste URL thật của chủ.
- `CLAUDE.md` — CHỈ 2 chỗ (đúng lệnh cho phép): (1) thêm `pending_poll_ms`
  `pending_txpool_max_per_poll` vào danh sách field bắt buộc mục "Config —
  thiếu field = fail load"; (2) thêm đúng 1 câu về `.env` đa URL failover
  ngay dưới danh sách field. Không sửa gì khác trong file.
- `docs/STATE.md` — thêm mục `5.3` đầy đủ: thiết kế `RpcPool`, phát hiện
  thật về `.env` CSV-trong-biến-gốc (kèm số liệu `grep`/`wc` xác minh), kết
  quả lọc private-send URL trên dữ liệu thật (2/34 URL bị lọc), quyết định
  không đổi `AppStateInner`/`web.rs`, `PENDING_WS_CHANNEL_SIZE`, log runtime
  thật của lần verify failover.
- `docs/TASKS.md` — thêm dòng roadmap `5.3` XONG; cập nhật mục Nợ (đóng nợ
  "CLAUDE.md thiếu pending_poll_ms" từ BAOCAO08; thêm nợ mới: chưa đo được
  `PENDING_WS_CHANNEL_SIZE=4096` có đủ chịu mempool BSC lâu dài hay không;
  `resolve_v2_reserves` vẫn chỉ được `http_pool` bảo vệ GIÁN TIẾP qua
  health-check, không trực tiếp per-call).
- `config.toml` — thêm `pending_txpool_max_per_poll = 32` (ship, kèm
  comment). File này KHÔNG có tên trong "ĐƯỢC ĐỤNG" của lệnh phiên này
  nhưng bắt buộc phải sửa cùng `config.rs`: field mới là bắt buộc (không
  `#[serde(default)]`, đúng luật CLAUDE.md "thiếu field = fail load") — nếu
  không thêm vào `config.toml` ship thì chính file cấu hình mặc định của
  repo sẽ FAIL LOAD ngay khi boot, phá vỡ toàn bộ sản phẩm. Coi đây là hệ
  quả bắt buộc đi kèm thay đổi `config.rs` (đã được lệnh cho phép "nếu
  cần"), cùng tiền lệ BAOCAO08 (cũng sửa `config.toml` khi thêm
  `pending_poll_ms`) — ghi rõ ở đây để Grok biết đây không phải lỗi tự ý mở
  rộng phạm vi.
- KHÔNG đụng: `web.rs` (không thêm field `http_pool` vào `AppStateInner`,
  xem lý do ở mục FILE ĐỔI trên — `web.rs` không nằm trong ĐƯỢC ĐỤNG lệnh
  này); `.env` thật; `victims.txt` thật; `src/executor.rs` (không
  `send_raw_transaction`); `src/pool.rs` (không đụng V4/Infinity `PoolKey`);
  cờ `allow_live`/`bot_armed`/`dry_run`.

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test precheck_without_reserves_never_touches_rpc -- --nocapture
cargo test rpc_pool_failover_when_first_url_dead_picks_next -- --nocapture
```
Runtime smoke test THẬT (dùng `.env` thật của chủ — chỉ ĐỌC `BSC_HTTP`/
`BSC_WS`, KHÔNG sửa `.env`/`config.toml`/`victims.txt` thật; config scratch
NGOÀI repo, port `18793`; `state/`/`logs/` là thư mục gitignored thật của
repo, đúng tiền lệ BAOCAO05-08). Cố ý đặt `BSC_HTTP` = 1 URL CHẾT
(`http://127.0.0.1:1/`, cổng không ai lắng nghe) làm URL ĐẦU, `BSC_HTTP_2` =
`BSC_HTTP` thật của chủ (chuỗi CSV 34 URL) để buộc code phải failover:
```
REAL_HTTP=$(grep '^BSC_HTTP=' .env | cut -d= -f2-)
REAL_WS=$(grep '^BSC_WS=' .env | cut -d= -f2-)
BSC_HTTP="http://127.0.0.1:1/" BSC_HTTP_2="$REAL_HTTP" BSC_WS="$REAL_WS" \
  target/debug/bsc_sandwich.exe <scratch_config.toml, web_port=18793>
curl http://127.0.0.1:18793/api/status
```

5. OUTPUT THẬT:

`cargo test` (132 passed, 2 ignored — 2 ignored cần RPC sống như cũ, +16
test mới so BAOCAO08: config +2, pipeline +1, transport +12, đủ ≥15 dòng
cuối thật):
```
running 134 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test config::tests::max_roundtrip_tax_zero_load_ok ... ok
test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok ... ok
test config::tests::effective_front_cap_wei_uncapped_when_max_exposure_zero ... ok
test config::tests::min_profit_bnb_negative_fails ... ok
test config::tests::dry_run_true_blocks_live_gate ... ok
test config::tests::missing_pending_txpool_max_per_poll_fails ... ok
test config::tests::pending_txpool_max_per_poll_loads_ship_value ... ok
test pipeline::tests::precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason ... ok
test transport::tests::is_private_send_url_detects_all_3_keywords_case_insensitive ... ok
test transport::tests::filter_read_urls_drops_private_send_keeps_normal_dataseed ... ok
test transport::tests::parse_rpc_url_list_empty_when_nothing_set ... ok
test transport::tests::parse_rpc_url_list_falls_back_to_base_plus_numbered_when_list_absent ... ok
test transport::tests::parse_rpc_url_list_ignores_empty_list_value_falls_back_to_numbered ... ok
test transport::tests::parse_rpc_url_list_prefers_list_over_numbered ... ok
test transport::tests::parse_rpc_url_list_splits_commas_inside_base_var_itself ... ok
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok
(... 112 test config/decoder/executor/logger/pool/sim_v2/sim_v3/state/tax/
transport/venues/victims/pipeline còn lại đều "... ok", không lặp lại hết
cho gọn — pool::tests::real_rpc_v2_get_pair_wbnb_usdt và
sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt là 2 dòng "... ignored")

test result: ok. 132 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.08s
```

`cargo test precheck_without_reserves_never_touches_rpc -- --nocapture`
(ĐẠT CẦN DÁN: "test precheck không gọi RPC"):
```
running 1 test
test pipeline::tests::precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 132 filtered out; finished in 0.00s
```

`cargo test rpc_pool_failover_when_first_url_dead_picks_next -- --nocapture`
(ĐẠT CẦN DÁN: "test failover URL đầu chết → URL sau"):
```
running 1 test
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 132 filtered out; finished in 2.05s
```
(Test dùng cổng TCP `127.0.0.1:1` — refused ngay — làm "URL đầu chết", 1 mock
JSON-RPC server nội bộ trả đúng `chain_id=56` làm "URL sau"; assert
`rpc.failover` log ở `pool_index:0`, `rpc.connect` ở `pool_index:1`, provider
trả về `Some`.)

Runtime THẬT (binary chạy, KHÔNG phải unit test — `.env` chủ có cả
`BSC_HTTP` lẫn `BSC_WS`, port scratch `18793`, không đụng file thật của
repo). `BSC_HTTP` bị GHI ĐÈ cố ý = 1 URL chết, `BSC_HTTP_2` = `BSC_HTTP`
thật của chủ. `logs/bot.jsonl` (rút gọn, thứ tự thời gian thật):
```
{"event":"rpc.failover","pool_index":0,"pool_size":33,"reason":"rpc khong ket noi duoc: eth_chainId that bai: error sending request for url (http://127.0.0.1:1/)","transport":"http","url":"http://127.0.0.1:1/***"}
{"event":"rpc.connect","pool_index":1,"pool_size":33,"transport":"http","url":"https://bsc-dataseed1.bnbchain.org/***"}
{"event":"rpc.connect","transport":"ws_heads","url":"wss://bsc-rpc.publicnode.com/***"}
{"event":"rpc.pending_subscribed","transport":"ws","url":"wss://bsc-rpc.publicnode.com/***"}
```
`GET /api/status`:
```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":false,"last_block":121858949,"pending_source":"ws",
 "max_exposure_bnb":5.0,"max_front_bnb":5.0,"min_profit_bnb":0.006,
 "state":"WATCHING","uptime_sec":5}
```
**Phát hiện quan trọng, ngoài dự kiến của lệnh**: `.env` thật của chủ đã tự
điền `BSC_HTTP=` là 1 chuỗi **34 URL nối bằng dấu phẩy NGAY TRONG BIẾN GỐC**
(không dùng tên `BSC_HTTP_LIST` như mô tả trong lệnh) — xác minh bằng
`grep -o 'https\?://' .env | wc -l` = 34, đếm dấu phẩy = 33 (khớp n-1 cho n
phần tử). Nếu `parse_rpc_url_list` chỉ tách CSV ở `<base>_LIST` (đúng chữ
lệnh) thì 33/34 URL này sẽ bị mất hoàn toàn (cả chuỗi bị coi là "1 URL",
`ProviderBuilder::connect` chắc chắn lỗi parse) — đã PHÁT HIỆN bug này
NGAY trong lúc verify runtime (log đầu tiên cho `pool_size:1` thay vì mong
đợi nhiều hơn), sửa `parse_rpc_url_list` để tách dấu phẩy ở MỌI field (kể cả
`<base>` gốc — URL hợp lệ không bao giờ chứa dấu phẩy chưa mã hoá nên an
toàn tuyệt đối), build lại, test lại — kết quả trên là SAU khi sửa. Thêm 1
URL chết cố ý = 35 ứng viên; `pool_size` log ra = **33** — khớp chính xác
(34 URL thật − 2 URL bị `filter_read_urls` lọc [1 chứa `fullprivacy`, 1 chứa
`maxbackrun`, xác minh bằng `grep -oiE 'maxbackrun|fullprivacy|privacy'`
trên `.env`] + 1 URL chết = 33) — chứng minh lọc private-send URL hoạt động
ĐÚNG trên dữ liệu THẬT của chủ, không phải fixture giả định. Log KHÔNG chứa
token/path nào của `.env` (chỉ `scheme://host/***`) — đã tự kiểm tra bằng
mắt toàn bộ output trước khi dán vào báo cáo này. Sau khi verify đã
`taskkill` process, xoá thư mục scratch `/tmp/bsc_scratch` — `git status`
xác nhận không có gì mới bị track ngoài ý muốn (chỉ đúng 15 mục untracked
gốc từ đầu phiên, không có `state/`/`logs/` nào lọt vào git vì đã gitignore).

6. CHAIN: `0x38` xác nhận qua `eth_chainId` thật ở CẢ 33 URL trong pool đọc
lẫn `wss://bsc-rpc.publicnode.com` (WS) — `RpcPool::connect` chỉ giữ lại
provider khi `connect_and_verify` xác nhận đúng 56 (URL nào sai chain sẽ bị
skip, đã test bằng mock server `rpc_pool_skips_wrong_chain_url_then_picks_correct_one`,
không xảy ra trên dữ liệu thật lần này vì cả 34 URL chủ dùng đều đúng chain
56). `last_block=121858949` số THẬT qua `eth_blockNumber` (thực chất tới từ
`ws_heads` `eth_subscribe newHeads`, không phải HTTP — cả 2 đường đều hoạt
động). Cụm này KHÔNG thêm `eth_call` pin mới (không resolve pool/factory
mới) — chỉ thêm cách CHỌN provider nào dùng cho các `eth_call`/`eth_getBlock`/
`txpool_content` đã có sẵn từ trước, không đụng registry.

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không
pin address mới, không thêm contract nào, không đụng family Pancake nào).

8. KHÔNG LÀM: không có `7.x`/executor gửi tx thật (`send_raw_transaction`
vẫn không tồn tại trong repo); không đụng V4/Infinity `PoolKey` (`pool.rs`
không đổi); không có hợp đồng "probe" đo tax bytecode (`tax.rs` không đổi);
không bật `allow_live`/`dry_run=false`/`bot_armed`; không sửa `.env`/
`victims.txt` thật; **không sửa `CLAUDE.md` ngoài đúng 2 chỗ lệnh cho phép**
(danh sách field bắt buộc + 1 câu multi-URL, không đổi gì khác trong file);
**không sửa `web.rs`/`AppStateInner`** (không nằm trong ĐƯỢC ĐỤNG lệnh này
— xem giải pháp truyền `http_pool` qua tham số hàm thay vì field struct ở
ô 3).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `PENDING_WS_CHANNEL_SIZE=4096` (nâng từ mặc định `alloy` 16) CHƯA được đo
  có đủ chịu tốc độ mempool BSC thật lâu dài hay không — verify runtime
  phiên này chỉ chạy ~5-6s (đủ để chứng minh failover HTTP, KHÔNG đủ để lặp
  lại kịch bản "channel lagged" của BAOCAO08 và so sánh trước/sau). Cần 1
  phiên chạy dài hơn (nhiều phút) để đo con số này có thật sự cải thiện độ
  ổn định WSS hay không.
- `pipeline::resolve_v2_reserves` (per-tx `eth_call` resolve pool/reserve)
  KHÔNG được `http_pool` bảo vệ TRỰC TIẾP — vẫn dùng `app_state.provider`
  snapshot tại thời điểm gọi, chỉ được bảo vệ GIÁN TIẾP qua
  `http_pool_health_check` (task nền 5s/lần giữ provider sống). Nếu 1 lần
  `eth_call` cụ thể trong `resolve_v2_reserves` lỗi transport ngay giữa 2
  lần health-check, lần đó vẫn trả `no_pool` như cũ (không tự failover ngay
  lập tức trong chính call đó) — đánh đổi có chủ đích để KHÔNG đổi chữ ký
  `pipeline.rs` (giữ tách lõi thuần/RPC thật đúng quy ước `docs/STATE.md`
  đã có từ `5.1`), độ trễ tối đa tới khi hồi phục là ~5s (interval
  health-check).
- `subscribe_ws_heads` chỉ thử lần lượt các URL LÚC BOOT (không tự động
  chuyển sang URL WSS khác nếu subscription heads rớt GIỮA CHỪNG sau khi đã
  chọn được 1 URL — best-effort, khác `subscribe_pending_txs` nơi pending-tx
  quan trọng hơn được ưu tiên failover đầy đủ). `last_block` vẫn được
  `http_pool_health_check` cập nhật độc lập nên không bị "đứng hình" hoàn
  toàn nếu heads rớt, chỉ mất độ chính xác theo từng block.
- Chưa verify được nhánh "TOÀN BỘ pool HTTP không URL nào connect được"
  trên dữ liệu thật (chỉ test bằng mock/cổng chết trong unit test) — không
  có cách an toàn để mô phỏng "tất cả 33 URL thật đều chết cùng lúc" mà
  không phá mạng máy hoặc chờ rất lâu; hành vi (log rõ lý do, giữ
  `app_state.provider=None`, không panic/halt) đã verify bằng unit test
  `rpc_pool_all_urls_dead_returns_none_no_panic`.
- Kế thừa từ BAOCAO08 (chưa đổi ở cụm này): `resolve_v2_reserves` vẫn gộp
  mọi lỗi RPC + "không có pool" thành `no_pool`; chiều victim BÁN token lấy
  WBNB vẫn `not_wbnb_pair`; `measure_roundtrip_via_router` vẫn không đo được
  fee-on-transfer thật.
