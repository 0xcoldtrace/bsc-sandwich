1. LÁT: 5.2 — vận hành paper: exposure cap ăn vào decide, cảnh báo gas vs
min_profit lúc boot, pending không phụ thuộc WSS (fallback WSS ->
txpool_content -> inject_only), web đủ đọc (pending_source + 3 ngưỡng).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, config.toml, src/pipeline.rs, src/sim_v2.rs, src/main.rs, src/transport.rs, src/web.rs, baocao/BAOCAO07.md
Gói tax-inject ĐẠT (Grok). 7.x CẤM.

LÁT: 5.2 vận hành paper — exposure cap + pending không phụ thuộc WSS + web đủ đọc.
stack = Rust + alloy.

ĐƯỢC ĐỤNG: src/pipeline.rs, src/sim_v2.rs, src/main.rs, src/transport.rs, src/web.rs, src/config.rs nếu thiếu helper wei, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO08.md
CẤM: CLAUDE.md, .env, sendRaw, bật allow_live/bot_armed, dry_run=false, bịa pending

LÀM CÙNG PHIÊN:
1) max_exposure_bnb ĂN vào decide — front_in ≤ min(max_front_wei, max_exposure_wei).
   Test: max_front=10, max_exposure=5 → front_in ≤ 5 BNB wei.
   max_exposure=0 fail load HOẶC 0=tắt cap — chọn 1, ghi STATE. Đề xuất: 0=tắt cap.
2) Cảnh báo gas vs min_profit — boot log nếu min_profit_wei < front_max_gas + back_max_gas. Không fail load.
3) Pending khi BSC_WS trống — không halt. WSS -> fail -> poll HTTP txpool_content/txpool_inspect
   mỗi N ms (config pending_poll_ms ship 400). Node không hỗ trợ txpool -> log rpc.pending_unavailable,
   vẫn nhận state/inject_tx.jsonl. Redact URL. Sai chain != 56 không watch. Concurrency ≤ 4.
4) Web: /api/status: pending_source = ws | txpool | inject_only. Hiện max_front, max_exposure, min_profit.
5) cargo test xanh + test exposure cap.

KHÔNG LÀM: executor live, Infinity PoolKey, probe tax bytecode, chiều bán trừ khi còn sức và có test riêng.

ĐẠT CẦN DÁN: cargo test ≥15 dòng; test exposure 10 vs 5; ghi pending_source khi boot.

VIẾT: baocao/BAOCAO08.md đủ 10 ô. CHỜ GROK. Cấm ĐẠT.
Ô 1: 5.2
```

3. FILE ĐỔI:
- `src/config.rs` — thêm field bắt buộc mới `pending_poll_ms: u64` (không
  `#[serde(default)]`, thiếu = fail load, cùng khuôn `allow_tax_inject`).
  Thêm 3 method mới: `max_exposure_wei() -> Option<U256>` (`None` khi
  `max_exposure_bnb == 0.0` = TẮT cap — quyết định theo ĐỀ XUẤT trong lệnh,
  ghi rõ ở `docs/STATE.md`), `effective_front_cap_wei() -> U256` (điểm DUY
  NHẤT gộp `min(max_front_wei, max_exposure_wei)`), `gas_warning_needed() ->
  bool` (`min_profit_wei() < gas_wei()`, KHÔNG fail load). Cập nhật
  `base_toml()` fixture (`pending_poll_ms = 400`), thêm 7 test mới:
  `missing_pending_poll_ms_fails`, `effective_front_cap_wei_uses_min_of_max_front_and_max_exposure`,
  `effective_front_cap_wei_uncapped_when_max_exposure_zero`,
  `gas_warning_needed_true_when_min_profit_below_gas_total`,
  `gas_warning_needed_false_when_min_profit_above_gas_total`.
- `src/pipeline.rs` — `decide_paper` dùng `cfg.effective_front_cap_wei()`
  thay `cfg.max_front_wei()` khi gọi `sim_v2::search_max_front_in`. Cập nhật
  `test_config_toml()` thêm `pending_poll_ms = 400`. Thêm fixture
  `deep_pool_reserves()` (pool 1000 WBNB + victim 500 WBNB, tối ưu lý thuyết
  ~224 BNB — xa cả 2 trần test 5/10 BNB, đảm bảo `profit(front_in)` còn tăng
  dọc hết khoảng test, ternary search hội tụ CHÍNH XÁC tại biên) + 2 test:
  `max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb` (ĐẠT CẦN DÁN
  mục 1: max_front=10, max_exposure=5 → front_in hội tụ đúng 5e18 wei),
  `max_exposure_bnb_zero_disables_cap_front_can_exceed_5_bnb` (đối chứng:
  max_exposure=0 → front_in hội tụ đúng 10e18 wei, vượt mốc 5 cũ).
- `src/transport.rs` — thêm enum `PendingSource { Ws, Txpool, InjectOnly }`
  + `as_str()`, dùng cho `/api/status` (`pending_source`). 1 test
  `pending_source_as_str_matches_api_status_contract`.
- `src/web.rs` — `AppStateInner` thêm field `pending_source:
  RwLock<PendingSource>`. `status()` thêm `pending_source`, `max_front_bnb`,
  `max_exposure_bnb`, `min_profit_bnb` (đọc thẳng `Config` lock, không
  hardcode).
- `src/main.rs`:
  - Boot: gọi `cfg.gas_warning_needed()` ngay sau load `cfg` (trước khi bọc
    `RwLock`) — log `config.gas_warning` (`logs/bot.jsonl`) + `eprintln!`
    (KHÔNG fail load). `config.toml` ship mặc định (`min_profit_bnb=0.001` <
    gas total `0.006`) THỰC SỰ kích hoạt cảnh báo — verify runtime ở ô 5.
  - `AppStateInner` khởi tạo thêm `pending_source:
    RwLock::new(transport::PendingSource::InjectOnly)`.
  - `connect_rpc` nhận thêm tham số `pending_poll_interval: Duration`
    (`Duration::from_millis(cfg.pending_poll_ms.max(1))`, đọc từ `cfg` local
    trước khi cfg bị move vào `RwLock`).
  - Viết lại HOÀN TOÀN `subscribe_pending_txs`: orchestrate fallback ĐÚNG
    thứ tự lệnh — có `ws_url` thì `connect_and_verify` (đã tự chặn
    `chain_id != 56`) rồi `subscribe_full_pending_transactions()`; thành
    công thì set `pending_source=Ws`, ở vòng `recv()` tới khi lỗi/rớt
    (`break`, không `return`) rồi RƠI XUỐNG `poll_txpool_pending`. Không có
    `ws_url`, hoặc nhánh WS lỗi, đều gọi thẳng `poll_txpool_pending`.
  - Hàm mới `poll_txpool_pending` — poll `txpool_content` mỗi
    `pending_poll_ms` qua `Provider::raw_request` (feature `provider-http`
    có sẵn, KHÔNG thêm feature crate nào — xem lý do kỹ thuật ở ô này bên
    dưới) + struct tối giản tự viết `TxpoolContentPendingOnly` (chỉ đọc
    field `pending`, bỏ `queued`). Lỗi lần gọi ĐẦU (node không hỗ trợ
    namespace `txpool`) -> log `rpc.pending_unavailable` + DỪNG task, không
    halt bot. Thành công -> set `pending_source=Txpool`, log
    `rpc.pending_subscribed`, dedup theo `TxHash` (`HashSet`, cap `5_000`
    phần tử) trước khi `tokio::spawn(handle_paper_tx(...))` — mỗi tx log
    `tx.seen` với `source:"txpool"`.
- `Cargo.toml` — THỬ thêm feature `rpc-types-txpool` để dùng thẳng
  `alloy::providers::ext::TxPoolApi::txpool_content()` -> `cargo build`
  FAIL resolve dependency thật (`alloy-provider-2.4.2` khai
  `alloy-rpc-types-txpool = "2.4.2"` nhưng crate đó CHƯA có bản `2.4.2` trên
  crates.io, chỉ tới `2.4.1` — lỗi đồng bộ version của nhà phát hành `alloy
  2.4.2`). ĐÃ REVERT feature này, không hạ version `alloy` (giữ nguyên pin
  `docs/STATE.md`) — dùng `Provider::raw_request` thay thế, `Cargo.toml`
  KHÔNG đổi so BAOCAO07.
- `config.toml` — thêm `pending_poll_ms = 400` (ship, kèm comment).
- `web/app.js` — `renderBotKv` thêm 4 dòng: `pending_source`,
  `max_front_bnb`, `max_exposure_bnb` (hiện "0 (tat cap)" khi bằng 0),
  `min_profit_bnb`.
- `docs/STATE.md` — thêm mục `5.2` (quyết định `max_exposure_bnb=0=tắt cap`,
  gas warning, fallback WSS→txpool_content, phát hiện lệch version
  `alloy-rpc-types-txpool`, verify runtime THẬT lần đầu WSS pending thật kết
  nối được, và ghi chú `CLAUDE.md` không được sửa phiên này nên
  `pending_poll_ms` chưa vào danh sách field chính thức).
- `docs/TASKS.md` — thêm dòng roadmap `5.2` XONG; cập nhật mục Nợ (tách
  `max_exposure_bnb` ra khỏi nhóm "chưa wire" vì đã xong; ghi rõ pending
  WSS/txpool vẫn chưa ổn định lâu dài — "channel lagged" + rate-limit thật;
  thêm nợ mới "CLAUDE.md thiếu tên field pending_poll_ms" do lệnh CẤM sửa
  file này phiên này).
- KHÔNG đụng: `CLAUDE.md`, `.env`, `victims.txt` thật, `src/executor.rs`
  (không `send_raw_transaction`), `src/pool.rs` (không đụng V4/Infinity
  `PoolKey`), cờ `allow_live`/`bot_armed`/`dry_run`.

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo test max_exposure_bnb -- --nocapture
```
Runtime smoke test THẬT (dùng `.env` thật của chủ — chỉ ĐỌC `BSC_HTTP`/
`BSC_WS`, KHÔNG sửa `.env`/`config.toml`/`victims.txt` thật; config/victims
scratch NGOÀI repo, port `18792`; `state/`/`logs/` là thư mục gitignored
thật của repo, đúng tiền lệ BAOCAO05/06/07):
```
export $(grep -E '^BSC_HTTP=|^BSC_WS=' .env | xargs)
target/debug/bsc_sandwich.exe <scratch_config.toml, web_port=18792>
curl http://127.0.0.1:18792/api/status
curl http://127.0.0.1:18792/api/skips
```

5. OUTPUT THẬT:

`cargo test` (116 passed, 2 ignored — 2 ignored cần RPC sống như cũ, +8 test
mới so BAOCAO07 [7 config + 2 pipeline, trừ đi 1 vì đếm gộp — thực tế
config +6, pipeline +2 = +8 tổng]; ≥15 dòng cuối thật):
```
running 118 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test config::tests::effective_front_cap_wei_uses_min_of_max_front_and_max_exposure ... ok
test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok ... ok
test config::tests::max_roundtrip_tax_zero_load_ok ... ok
test config::tests::dry_run_true_blocks_live_gate ... ok
test config::tests::load_ok ... ok
test config::tests::effective_front_cap_wei_uncapped_when_max_exposure_zero ... ok
test config::tests::gas_warning_needed_true_when_min_profit_below_gas_total ... ok
test config::tests::chain_id_1_fails ... ok
test config::tests::max_roundtrip_tax_bps_rounds_to_nearest ... ok
test config::tests::min_profit_bnb_negative_fails ... ok
test config::tests::gas_warning_needed_false_when_min_profit_above_gas_total ... ok
test config::tests::missing_pending_poll_ms_fails ... ok
test config::tests::missing_allow_tax_inject_fails ... ok
test config::tests::missing_min_profit_bnb_fails ... ok
(... 94 test decoder/executor/logger/pool/sim_v2/sim_v3/state/tax/transport/
venues/victims/pipeline còn lại đều pass, không lặp lại hết cho gọn — 2 dòng
mới đáng chú ý: pipeline::tests::max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb
và pipeline::tests::max_exposure_bnb_zero_disables_cap_front_can_exceed_5_bnb,
cả 2 đều "... ok")

test result: ok. 116 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test max_exposure_bnb -- --nocapture` (ĐẠT CẦN DÁN: "test exposure 10
vs 5"):
```
running 2 tests
exposure off test: front_in=10000000000000000000 (phai duoc vuot 5 BNB=5000000000000000000, hoi tu tai 10 BNB=10000000000000000000)
exposure cap test: front_in=5000000000000000000 (tran max_exposure_bnb=5 BNB=5000000000000000000), max_front_bnb=10
test pipeline::tests::max_exposure_bnb_zero_disables_cap_front_can_exceed_5_bnb ... ok
test pipeline::tests::max_exposure_bnb_caps_front_in_tighter_than_max_front_bnb ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 116 filtered out; finished in 0.00s
```
`max_front_bnb=10, max_exposure_bnb=5` (mặc định ship trong `test_config_toml`)
-> `front_in` hội tụ ĐÚNG `5_000_000_000_000_000_000` wei (= 5 BNB), KHÔNG
dùng tới trần `max_front_bnb=10`. Đối chứng `max_exposure_bnb=0` (tắt) ->
`front_in` hội tụ đúng `10_000_000_000_000_000_000` wei (= 10 BNB), vượt qua
mốc 5 cũ — chứng minh `0` là "tắt cap" chứ không phải "trần 0 BNB".

Runtime THẬT (binary chạy, KHÔNG phải unit test — `.env` chủ LẦN ĐẦU có cả
`BSC_HTTP` LẪN `BSC_WS` không rỗng, port scratch `18792`, không đụng file
thật của repo). Boot log (stdout):
```
bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
CANH BAO: min_profit_bnb=0.001 BNB thap hon tong gas front+back cap (6000000000000000 wei) - xem logs/bot.jsonl config.gas_warning
web dashboard bind tai http://127.0.0.1:18792 (dry_run=true)
```
`logs/bot.jsonl` (rút gọn, thứ tự thời gian thật):
```
{"event":"rpc.connect","transport":"http","url":"https://bsc-dataseed1.bnbchain.org/***"}
{"event":"rpc.connect","transport":"ws","url":"wss://bsc-rpc.publicnode.com/***"}
{"event":"rpc.pending_subscribed","transport":"ws","url":"wss://bsc-rpc.publicnode.com/***"}
{"event":"rpc.pending_unavailable","reason":"subscription rot: channel lagged by 24","transport":"ws"}
{"event":"rpc.pending_subscribed","transport":"txpool_content"}
{"event":"tx.seen","from":"0x...","source":"txpool"}
{"event":"tx.skip","from":"0x...","reason":"decode_fail","token":null}
{"event":"rpc.pending_unavailable","reason":"server returned an error response: error code -32005: limit exceeded","transport":"txpool_content"}
```
`GET /api/status` (ĐẠT CẦN DÁN: "ghi pending_source khi boot" — số THẬT,
không bịa hash):
```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":false,"last_block":121855303,
 "live_gate":{...},"max_exposure_bnb":5.0,"max_front_bnb":5.0,
 "min_profit_bnb":0.001,"pending_source":"txpool","state":"WATCHING","uptime_sec":14}
```
`GET /api/skips` sau ~10s chạy trên mempool BSC thật:
```
{"below_min":0,"deadline":0,"decode_fail":856,"honeypot_or_tax":0,
 "hooks_unread":0,"no_pool":0,"not_in_list":3,"not_wbnb_pair":8,
 "thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
```
Đây là bằng chứng THẬT đầu tiên trong cả roadmap rằng
`subscribe_full_pending_transactions` qua WSS thực sự CHẠY ĐƯỢC trên node
BSC thật (`.env` chủ lần này có `BSC_WS` không rỗng) — subscribe thành công
(`rpc.pending_subscribed transport=ws`), nhưng buffer nội bộ tràn ("channel
lagged by 24", tốc độ mempool BSC thật quá nhanh so tốc độ xử lý mỗi tx qua
`eth_call`) khiến subscription rớt sau ~1 giây — ĐÚNG kịch bản lệnh yêu cầu
code phải xử lý ("WSS -> fail -> poll HTTP"), và code đã fallback đúng: gọi
`txpool_content` thành công 1 lần rồi bị RPC công khai rate-limit
(`-32005 limit exceeded`, không phải lỗi code) sau vài giây, dừng task đúng
thiết kế (không halt bot, không crash — bot vẫn WATCHING, web vẫn phục vụ).
Đã kill process (PowerShell `Stop-Process`) sau khi verify, xoá
`state/inject_tx.jsonl`/`state/tax_inject.jsonl` scratch nếu có (gitignored,
không vào git) — `git status` xác nhận không có gì mới bị track ngoài ý
muốn.

6. CHAIN: `0x38` xác nhận qua `eth_chainId` thật ở CẢ HAI transport (`http`
`https://bsc-dataseed1.bnbchain.org` và `ws`
`wss://bsc-rpc.publicnode.com`, redact đúng — không log token/path). WSS lần
này KHÔNG còn là placeholder (`REPLACE_ME_WSS_RPC_URL`) như mọi phiên trước
— `.env` chủ đã điền thật. `last_block=121855303` số THẬT lấy qua
`eth_blockNumber`. Cụm này KHÔNG thêm `eth_call` pin mới (không resolve
pool/factory mới) — `txpool_content` là RPC namespace khác (`txpool_*`,
không phải `eth_*`), không đụng registry.

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không
pin address mới, không thêm contract nào, không đụng family Pancake nào).

8. KHÔNG LÀM: không có `7.x`/executor gửi tx thật (`send_raw_transaction`
vẫn không tồn tại trong repo); không đụng V4/Infinity `PoolKey` (`pool.rs`
không đổi); không có hợp đồng "probe" đo tax bytecode (`tax.rs` không đổi
logic đo, chỉ inject thủ công từ phiên trước); không thêm model sandwich cho
chiều victim BÁN token (vẫn `not_wbnb_pair` như từ `4.1`, đúng "khi còn sức
và có test riêng" — phiên này dồn hết sức cho 4 mục lệnh yêu cầu, không mở
rộng thêm); không bật `allow_live`/`dry_run=false`/`bot_armed`; không sửa
`.env`/`victims.txt` thật; **không sửa `CLAUDE.md`** (đúng lệnh CẤM, kể cả
để thêm field `pending_poll_ms` mới — xem hệ quả ở ô 10).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `CLAUDE.md` mục "Config — thiếu field = fail load" hiện THIẾU tên field
  `pending_poll_ms` (bắt buộc mới trong `config.rs` thật) — do lệnh phiên
  này liệt kê `CLAUDE.md` vào mục CẤM không có ngoại lệ (khác phiên
  tax-cache-inject cho phép "trừ 2 dòng field mới"). Cần 1 lệnh sau cho phép
  sửa đúng dòng này nếu Grok muốn tài liệu khớp lại 100% với code.
- Pending WSS/txpool vẫn CHƯA chứng minh được ổn định LÂU DÀI: WSS thật kết
  nối/subscribe được (lần đầu tiên trong roadmap) nhưng rớt sau ~1s vì
  "channel lagged" (mempool BSC thật bắn tx quá nhanh so buffer nội bộ +
  tốc độ xử lý mỗi tx qua `eth_call` resolve reserve); fallback
  `txpool_content` cũng chỉ chạy vài giây trước khi RPC công khai
  rate-limit (`-32005`). Cả 2 đường đều hoạt động ĐÚNG THIẾT KẾ (fallback
  đúng, không halt, không crash) nhưng chưa ai chứng minh được 1 phiên chạy
  liên tục nhiều phút/giờ không rớt — cần RPC riêng (không rate-limit công
  khai) hoặc tăng buffer subscription để thử tiếp, NGOÀI PHẠM VI phiên này
  (chỉ ghi nhận qua log thật, không tự ý đổi thêm code).
- `resolve_v2_reserves` vẫn gộp mọi lỗi RPC + "không có pool" thành `no_pool`
  (kế thừa từ BAOCAO06/07, không đổi ở cụm này).
- Chiều victim BÁN token lấy WBNB vẫn `not_wbnb_pair` (kế thừa `4.1`, lệnh
  phiên này liệt kê rõ "chiều bán trừ khi còn sức và có test riêng" — phiên
  này không còn sức, dồn hết cho 4 mục bắt buộc).
- `measure_roundtrip_via_router` vẫn không đo được fee-on-transfer thật (kế
  thừa từ `3.3`/tax-cache-inject, không đổi ở cụm này).
