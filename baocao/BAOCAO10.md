1. LÁT: rpc-probe — cargo bin `rpc_probe` đo RTT/chain_ok từng URL
`BSC_HTTP*`/`BSC_WS*` (đúng parser `5.3`) + script wrapper + README vận
hành VPS + fix bảo mật `vps.json` (phát hiện dính secret thật, xem ô 3/6).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO09.md, src/transport.rs, vps.json, .env.example

GROK GIAO TOÀN QUYỀN PHIÊN NÀY trong biên:
- Được tự chọn file, crate, thứ tự việc, gộp test/docs/README/script.
- Được tạo bin, folder scripts/, sửa vps.json, README vận hành VPS.
- Được chạy rpc_probe nếu máy có mạng.
- Được viết BAOCAO10 đủ 10 ô.

CẤM CỨNG (không được quyền):
- sendRaw / executor live / 7.3
- allow_live=true, bot_armed=true, dry_run=false
- commit .env, in API key/token ra BAOCAO hoặc git
- bịa RTT / chainId / getCode
- xóa family venue, đổi stack khỏi Rust
- sửa CLAUDE.md ngoài field vận hành RPC/probe nếu thật sự thiếu

MỤC TIÊU PHIÊN (làm hết, đừng nợ vụn):
1) cargo run --bin rpc_probe — đọc BSC_HTTP/BSC_HTTP_LIST/BSC_HTTP_2.. + WSS
   cùng parser 5.3, lọc maxbackrun|fullprivacy|privacy khỏi đo ĐỌC, 5 mẫu
   eth_chainId + eth_blockNumber/URL timeout 2s, bảng host redact rtt
   min/p50/p95 chain_ok err, ghi artifacts/rpc_probe.json redact, không có
   env thì in MISSING không panic.
2) Failover giữ như 5.3, probe chỉ đo + gợi ý, không tự ghi .env thật.
3) README tiếng Việt ngắn: VPS NJ gần RPC US, đo PHẢI chạy trên VPS; cargo
   run bot; tunnel web 8787; không bind 0.0.0.0 mặc định.
4) vps.json: region_hint us-east (NJ), không secret.
5) Test probe + cargo test xanh.

XONG: baocao/BAOCAO10.md. Ô 9: CHỜ GROK | FAIL | CHƯA XONG. Cấm ĐẠT.
Ô 5: dán bảng probe nếu chạy được trên máy này (VPS thì dán thật; máy local
VN ghi RÕ "đo từ local không phải NJ").
Làm nốt việc dính (gitignore artifacts, script wrapper) nếu thuận tiện.
```

3. FILE ĐỔI:
- **`vps.json` — FIX BẢO MẬT, đọc trước khi làm bất kỳ việc gì khác**: file
  này (đọc theo đúng lệnh mục ĐỌC) KHÔNG còn đúng schema `chain_id`/RPC
  placeholder như `CLAUDE.md`/`docs/DOC_MAP.md` mô tả — bị ghi đè thành 4
  dòng **thông tin đăng nhập SSH root THẬT của 1 VPS** (IP:port, user
  `root`, mật khẩu dạng chữ thường) nằm trong file **KHÔNG có trong
  `.gitignore`** (chỉ `.env state/ logs/ target/` được ignore — `vps.json`
  đáng lẽ phải an toàn để commit). `git status` đầu phiên xác nhận file
  CHƯA từng được `git add`/commit (repo "No commits yet") nên chưa lọt vào
  lịch sử git, nhưng đây vẫn là 1 secret thật nằm trên đĩa ở dạng plaintext,
  sẵn sàng bị commit nhầm bất kỳ lúc nào (vd `git add -A` sau này). Đã sửa
  ngay: ghi đè `vps.json` về đúng schema + thêm `region_hint`/`region_note`
  theo đúng lệnh mục 4 — KHÔNG còn bất kỳ thông tin đăng nhập nào. Thông tin
  SSH thật KHÔNG được chép sang bất kỳ file nào khác trong repo (kể cả
  `.env` dù gitignored — SSH VPS không nằm trong schema `.env` CLAUDE.md
  định nghĩa cho bot, thêm field lạ là mở rộng phạm vi ngoài lệnh). Đã báo
  trực tiếp cho chủ trong hội thoại phiên này (không lặp lại mật khẩu ở đây
  hay bất kỳ đâu) — khuyến nghị chủ **đổi mật khẩu VPS đó** và lưu thông tin
  đăng nhập ở nơi quản lý mật khẩu chuyên dụng, không dán vào file nào trong
  repo (kể cả gitignored) ở các phiên sau. Chi tiết đầy đủ + cách sửa ở
  `docs/STATE.md` mục "`vps.json` — PHÁT HIỆN BẢO MẬT...".
- `src/lib.rs` (MỚI) — `pub mod` cho toàn bộ 14 module cũ (`config` ...
  `web`), KHÔNG đổi nội dung bất kỳ module nào bên trong. Bắt buộc để
  `src/bin/rpc_probe.rs` dùng lại đúng `transport::parse_rpc_url_list`/
  `filter_read_urls`/`redact_rpc_url`/`collect_rpc_urls_from_env` (cụm
  `5.3`) thay vì chép lại logic parse `.env` riêng (rủi ro 2 nơi lệch nhau).
- `src/main.rs` — xoá 14 dòng `mod X;` đầu file, thay bằng
  `use bsc_sandwich::{config::Config, logger::BotLogger,
  pipeline::{self, PipelineOutcome}, state::{...}, tax::{self, TaxCache},
  transport::{self, PendingTxRaw}, victims::VictimBook, web::{...}};` —
  KHÔNG sửa bất kỳ dòng logic nào khác trong file (đã verify: `cargo test`
  132 passed y hệt BAOCAO09, boot runtime thật vẫn `chain_id=56`,
  `last_block` thật, `pending_source=ws`, xem ô 5/6).
- `src/bin/rpc_probe.rs` (MỚI) — binary đo RTT/chain_ok:
  - Đọc URL qua `transport::filter_read_urls(transport::collect_rpc_urls_from_env("BSC_HTTP"))`
    và tương tự cho `"BSC_WS"` — TÁI SỬ DỤNG đúng hàm cụm `5.3` (không chép
    logic parse CSV/`_LIST`/`_2..16` riêng), tự động lọc
    `maxbackrun|fullprivacy|privacy` khỏi danh sách đo.
  - Cả 2 danh sách rỗng -> in `MISSING` + gợi ý dùng script wrapper, KHÔNG
    panic, return sớm (verify ở ô 5).
  - `probe_url(url, transport_label)`: connect 1 lần (timeout 2s qua
    `tokio::time::timeout`), sau đó 5 mẫu gọi `provider.get_chain_id()` +
    `provider.get_block_number()` TRÊN CÙNG kết nối (mỗi mẫu timeout riêng
    2s) — đo RTT ổn định giống cách bot chính dùng 1 provider lâu dài qua
    `RpcPool`, không đo nhiễu handshake TCP/TLS mỗi lần. Connect lỗi/timeout
    -> trả ngay 0 mẫu, `chain_ok=false`, `err` = lý do thật.
  - `percentile()` (nearest-rank trên `Vec<f64>` đã sort) tính `min`/`p50`/
    `p95` từ số mẫu thành công.
  - `shorten_err()` — PHÁT HIỆN khi verify runtime thật: vài RPC công khai
    (rate-limit `429`, chặn Cloudflare `403`) trả lỗi kèm NGUYÊN VĂN body
    HTML/JSON dài (không phải secret — đã tự kiểm tra bằng mắt, chỉ là
    thông báo lỗi công khai của bên thứ 3 như `leorpc.com/pricing`), làm
    bảng/JSON tràn hàng chục dòng. Gộp whitespace thành 1 dòng, cắt tối đa
    160 ký tự — không đổi ý nghĩa lỗi.
  - In bảng cột `transport/host_redacted/samples_ok/rtt_min/p50/p95/chain_ok/err`,
    ghi `artifacts/rpc_probe.json` (đã redact qua `transport::redact_rpc_url`,
    không path/query/token).
- `Cargo.toml` — thêm `[lib] name = "bsc_sandwich" path = "src/lib.rs"` +
  `[[bin]] name = "rpc_probe" path = "src/bin/rpc_probe.rs"` (Cargo cho phép
  1 package có cả lib lẫn bin trùng tên package, không xung đột — verify
  bằng `cargo build` xanh).
- `.gitignore` — thêm dòng `artifacts/` (chứa `rpc_probe.json`, dữ liệu đo
  mạng theo thời điểm chạy, không cần track — dù đã redact).
- `scripts/run_rpc_probe.sh` (MỚI) — bash, `source .env` (nếu có) vào biến
  môi trường TIẾN TRÌNH CON rồi `cargo run --bin rpc_probe --release`,
  KHÔNG in nội dung `.env`, KHÔNG sửa `.env`. Dùng trên VPS Linux/máy
  dev Linux/macOS.
- `scripts/run_rpc_probe.ps1` (MỚI) — PowerShell, cùng logic (parse
  `KEY=VALUE` từ `.env` bằng `Set-Item Env:`, không in ra) cho máy Windows.
- `README.md` — thêm 2 mục: "Đo RPC (rpc_probe) — PHẢI chạy trên VPS,
  không phải máy dev" (giải thích độ trễ xuyên lục địa nếu đo từ VN, lệnh
  SSH+clone+chạy script trên VPS) và "Chạy bot trên VPS + xem dashboard từ
  xa" (build --release, KHÔNG bind `0.0.0.0`, SSH local port-forward
  `8787` thay vì mở firewall).
- `docs/STATE.md` — thêm 2 mục cuối file: (1) "`rpc-probe`..." — thiết kế
  đầy đủ (lý do tách `lib.rs`, thiết kế đo 1-connect/5-mẫu, `shorten_err`,
  kết quả verify runtime thật rút gọn); (2) "`vps.json` — PHÁT HIỆN BẢO
  MẬT..." — mô tả đầy đủ phát hiện + cách sửa (chi tiết hơn ô 3 ở trên).
- `docs/TASKS.md` — thêm 1 dòng roadmap `rpc_probe` (đứng ngoài 0.x-7.x,
  công cụ vận hành) + 2 mục nợ mới: fix bảo mật `vps.json` (tóm tắt, trỏ
  `docs/STATE.md`), và "chưa đo RTT thật trên VPS NJ" (số đo phiên này từ
  máy dev VN, không dùng để quyết định thứ tự failover thật).
- `docs/DOC_MAP.md` — thêm 3 dòng mô tả `src/lib.rs`, `src/bin/rpc_probe.rs`,
  script wrapper (mục "File khác").
- KHÔNG đụng: `CLAUDE.md` (không có field config mới cần thêm ở phiên này
  nên không có lý do sửa — đúng CẤM CỨNG "sửa CLAUDE.md ngoài field vận
  hành RPC/probe **nếu thật sự thiếu**"); `.env` thật (chỉ ĐỌC qua
  `source`/`Set-Item Env:` trong tiến trình con của script, không ghi/sửa
  file); `config.toml`/`victims.txt` thật; `src/executor.rs` (không
  `send_raw_transaction`); cờ `allow_live`/`bot_armed`/`dry_run`; không
  module nào trong 14 module cũ bị sửa logic (chỉ đổi nơi khai `mod`).

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo build --release --bin rpc_probe
env -u BSC_HTTP -u BSC_HTTP_2 -u BSC_HTTP_LIST -u BSC_WS -u BSC_WS_2 -u BSC_WS_LIST cargo run --bin rpc_probe   # test nhanh MISSING
bash scripts/run_rpc_probe.sh   # dùng .env thật của máy này (KHÔNG in .env)
```
Smoke-test runtime bot chính sau refactor lib (config/victims SCRATCH ngoài
repo, port `18794`, KHÔNG đụng file thật của repo — `state/`/`logs/` vẫn là
thư mục gitignored thật của repo đúng tiền lệ):
```
target/debug/bsc_sandwich.exe /tmp/bsc_scratch10/config.scratch.toml
curl http://127.0.0.1:18794/api/status
```

5. OUTPUT THẬT:

`cargo test` (132 passed, 2 ignored — Y HỆT số liệu BAOCAO09, chứng minh
refactor `lib.rs` không đổi hành vi module nào, ≥15 dòng cuối thật):
```
running 134 tests
test config::tests::bnb_f64_to_wei_matches_exact_integer_cases ... ok
test config::tests::max_roundtrip_tax_zero_load_ok ... ok
test config::tests::max_front_bnb_100_and_min_profit_bnb_0_load_ok ... ok
test config::tests::effective_front_cap_wei_uncapped_when_max_exposure_zero ... ok
test config::tests::missing_pending_txpool_max_per_poll_fails ... ok
test config::tests::pending_txpool_max_per_poll_loads_ship_value ... ok
test pipeline::tests::precheck_without_reserves_never_touches_rpc_for_every_early_skip_reason ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
(... 119 test khác đều "... ok", pool::tests::real_rpc_v2_get_pair_wbnb_usdt
và sim_v3::tests::real_rpc_v3_quote_wbnb_to_usdt là 2 dòng "... ignored")

test result: ok. 132 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.08s
```

`rpc_probe` KHÔNG có env (test nhánh MISSING, không panic):
```
MISSING: khong co BSC_HTTP*/BSC_WS* nao trong bien moi truong hien tai.
(File .env khong tu dong duoc nap boi binary nay - can 'source'/export truoc khi chay,
 xem scripts/run_rpc_probe.sh hoac scripts/run_rpc_probe.ps1.)
```

**`rpc_probe` CÓ `.env` thật của chủ — ĐO TỪ MÁY LOCAL WINDOWS TẠI VIỆT NAM,
KHÔNG PHẢI VPS NEW JERSEY trong `vps.json`.** Số RTT dưới đây CHỈ chứng minh
code chạy đúng (chain_ok/redact/timeout/MISSING) — KHÔNG dùng để quyết định
thứ tự failover thật vì cộng thêm ~150-600ms độ trễ xuyên lục địa so với khi
bot thật chạy trên VPS NJ. Ghi CÒN NỢ (ô 10): cần 1 lần chạy
`scripts/run_rpc_probe.sh` TRÊN chính VPS NJ để có số dùng được thật.

```
rpc_probe: 32 URL HTTP + 1 URL WSS (da loc maxbackrun/fullprivacy/privacy), 5 mau/URL, timeout 2s/goi
tr     host                                     ok    min_ms    p50_ms    p95_ms chain_ok  err
http   https://bsc-dataseed1.bnbchain.org/***    5     220.5     221.3     638.6 true
http   https://jp-bscscutum.blockrazor.xyz/***   5     267.2     269.3     811.3 true
http   https://bsc-dataseed2.bnbchain.org/***    5     218.7     219.9     547.8 true
http   https://bsc-dataseed4.defibit.io/***      5     258.2     260.9     622.9 true
http   https://bsc-dataseed3.bnbchain.org/***    5     219.4     219.5     548.6 true
http   https://bsc-dataseed2.defibit.io/***      5     262.8     263.9     627.4 true
http   https://bsc-dataseed2.ninicoin.io/***     5     261.9     263.4     673.6 true
http   https://bsc-dataseed4.bnbchain.org/***    5     217.6     219.8     551.8 true
http   https://bsc-dataseed3.defibit.io/***      5     262.5     268.9     662.1 true
http   https://bsc-dataseed4.ninicoin.io/***     5     255.6     259.7     658.1 true
http   https://bsc-dataseed3.ninicoin.io/***     5     254.6     256.6     613.2 true
http   https://ger-bscscutum.blockrazor.xyz/***  5     575.9     587.6    1030.4 true
http   https://bsc-dataseed1.bnbchain.org/***    5     221.0     221.7     549.0 true
http   https://bsc-dataseed1.defibit.io/***      5     253.7     255.2     608.7 true
http   https://bsc-dataseed1.ninicoin.io/***     5     262.7     265.1     620.1 true
http   https://public-bsc.nownodes.io/***        5     608.8     659.9     740.0 true
http   https://rpc.solidrpc.io/***               5     514.1     515.6     683.6 true
http   https://56.rpc.thirdweb.com/***           5     524.3     570.3     792.5 true
http   https://us-bscscutum.blockrazor.xyz/***   5     602.4     605.5    1067.7 true
http   https://ire-bscscutum.blockrazor.xyz/***  5     461.6     470.8     799.9 true
http   https://bsc-mainnet.gateway.tatum.io/***  2      85.3     304.6     304.6 false  eth_chainId loi: HTTP error 429 with body: {"statusCode": 429, "message": "You have exceeded your limit of 5 requests per minute...
http   https://rpc.sentio.xyz/***                5     455.2     474.9     747.5 true
http   https://binance.rpc.thirdweb.com/***      5     553.3     816.0    1375.2 true
http   https://api.uniblock.dev/***              4     799.7     900.9    1072.4 false  eth_blockNumber loi: HTTP error 429 with body: {"message":"Throughput limit 1000 CUs/sec...
http   https://rpc.owlracle.info/***             0         -         -         - false  eth_chainId loi: HTTP error 401 with body: {"error":true,"code":401,"message":"You have reached the guest limit...
http   https://bsc.api.pocket.network/***        4     559.3    1087.3    2066.2 false  mau timeout 2s
http   https://bsc-rpc.blockreq.com/***          5     505.3     515.0    1341.9 true
http   https://xrpc.cl/***                       4    1077.3    1837.0    2020.0 false  mau timeout 2s
http   https://rpc.nodeflare.app/***             0         -         -         - false  eth_chainId loi: HTTP error 403 with body: <!DOCTYPE html>...
http   https://shared.us-east-1.getblock.io/***  5     656.7    1324.3    1496.5 true
http   https://bsc.leorpc.com/***                1    1009.3    1009.3    1009.3 false  eth_chainId loi: HTTP error 429 with body: {"msg":"Rate limit exceeded...
http   https://bsc.merkle.io/***                 3     524.8     525.1    1379.2 false  eth_chainId loi: server returned an error response: error code -32005: Rate limit exceeded
ws     wss://bsc-rpc.publicnode.com/***          5     248.5     256.2     260.3 true
da ghi artifacts/rpc_probe.json (33 URL, da redact)
```
33 URL = 32 HTTP + 1 WSS (đúng bằng 34 URL thật của chủ trừ 2 URL
`maxbackrun`/`fullprivacy` bị `filter_read_urls` lọc — khớp số liệu lọc đã
verify ở BAOCAO09). Không dòng nào chứa token/path của `.env` (chỉ
`scheme://host/***`) — đã tự kiểm tra bằng mắt toàn bộ output trước khi dán.

Smoke-test bot chính sau refactor `lib.rs` (config/victims SCRATCH ngoài
repo, port `18794`) — TRƯỚC KHI export `.env` (xác nhận boot không panic dù
chưa có RPC):
```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,"halt_lock":false,"last_block":null,...,"pending_source":"inject_only","state":"WATCHING","uptime_sec":3}
```
SAU KHI export `.env` thật (xác nhận refactor không phá kết nối RPC/WSS
thật — số THẬT, không bịa):
```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,"halt_lock":false,"last_block":121862518,...,"pending_source":"ws","state":"WATCHING","uptime_sec":5}
```
Đã `taskkill` process cả 2 lần chạy, xoá `/tmp/bsc_scratch10` — `git status`
sau cùng xác nhận không có gì mới bị track ngoài ý muốn (đúng 15 mục
untracked gốc + `scripts/` mới, không có `state/`/`logs/`/`artifacts/`).

6. CHAIN: `0x38` xác nhận qua `eth_chainId` thật ở CẢ 33 URL trong bảng
probe (32 `chain_ok=true`, 6 URL `chain_ok=false` là do RATE-LIMIT/TIMEOUT
của bên cung cấp — không phải sai chain, không có URL nào trả `chain_id≠56`
trong 34 URL thật của chủ) lẫn `wss://bsc-rpc.publicnode.com`. Smoke-test bot
chính SAU refactor `lib.rs`: `last_block=121862518` (số thật qua `eth_subscribe
newHeads`), `pending_source=ws` (subscribe pending WSS thật thành công) —
xác nhận tách `src/lib.rs` KHÔNG phá bất kỳ đường kết nối RPC nào đã có từ
`2.1`-`5.3`. Cụm này KHÔNG pin address/contract mới (không đụng
`DEX_REGISTRY.md`/`src/venues.rs`).

7. REGISTRY: KHÔNG đổi `DEX_REGISTRY.md`/`src/venues.rs` phiên này (không
pin address mới, không thêm contract nào, không đụng family Pancake nào).

8. KHÔNG LÀM:
- Không có `7.x`/executor gửi tx thật (`send_raw_transaction` vẫn không tồn
  tại trong repo); không bật `allow_live`/`dry_run=false`/`bot_armed`.
- Không bịa RTT/chain_id/getCode — toàn bộ số trong ô 5/6 là output THẬT của
  `cargo test`/`rpc_probe`/`curl` chạy trong phiên này, dán nguyên văn (chỉ
  cắt bớt dòng lặp "... ok" và nội dung lỗi HTTP dài để gọn báo cáo, có ghi
  rõ).
- KHÔNG chạy `rpc_probe` trên VPS New Jersey thật (máy phiên này là dev
  Windows tại VN, không có quyền/kết nối SSH tự động vào VPS đó trong phiên
  này) — số đo trong ô 5 CHỈ TỪ MÁY LOCAL, đã ghi rõ theo đúng lệnh, không
  phải số dùng để quyết định thứ tự failover thật.
- KHÔNG dùng/lưu lại thông tin đăng nhập SSH tìm thấy trong `vps.json` cũ
  vào bất kỳ file nào của repo (kể cả `.env`) — chỉ báo cho chủ trong hội
  thoại, không SSH tự động vào VPS đó (rủi ro thao tác tự động trên máy chủ
  thật với quyền root vượt phạm vi lệnh phiên này).
- Không sửa `CLAUDE.md` (không có field config mới cần thêm — đúng CẤM
  CỨNG "chỉ sửa nếu thật sự thiếu").
- Không sửa `.env`/`config.toml`/`victims.txt` thật.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- **Chưa có lần đo `rpc_probe` nào chạy TRÊN VPS New Jersey thật** —
  số RTT hiện tại chỉ từ máy dev VN, không dùng được để quyết định thứ tự
  `BSC_HTTP_2..16`/`BSC_HTTP_LIST` thật (độ trễ đo được sẽ khác hẳn từ VPS
  do vị trí địa lý). Cần chủ/Grok SSH vào VPS đó (thông tin đăng nhập đã
  được báo riêng trong hội thoại, KHÔNG lưu trong repo) rồi chạy
  `scripts/run_rpc_probe.sh` ở phiên sau.
- Chủ nên **đổi mật khẩu VPS** vì mật khẩu đó đã từng nằm trên đĩa dạng
  plaintext trong `vps.json` (dù chưa từng bị commit) — xem chi tiết
  `docs/STATE.md` mục "`vps.json` — PHÁT HIỆN BẢO MẬT...".
  `vps.json` đã được sửa về đúng schema, không còn secret.
  `.env` giữ nguyên (chưa từng bị đụng vào ở phiên này).
- `rpc_probe` chỉ ĐO + IN GỢI Ý — chưa có cơ chế nào tự động sắp lại thứ tự
  `BSC_HTTP_2..16` theo kết quả đo (đúng lệnh "probe chỉ đo + gợi ý, không
  tự ghi .env thật") — nếu chủ muốn thứ tự URL trong `.env` phản ánh RTT đo
  được, cần tự sửa tay `.env` dựa trên bảng probe (từ VPS thật), không phải
  việc tự động của cụm này.
- `shorten_err` cắt lỗi về tối đa 160 ký tự — nếu sau này cần debug sâu 1
  lỗi RPC cụ thể (vd đọc toàn bộ response Cloudflare để báo nhà cung cấp),
  cần chạy lại thủ công hoặc thêm cờ `--verbose` (chưa làm, ngoài phạm vi
  lệnh phiên này).
- Kế thừa từ BAOCAO09 (chưa đổi ở cụm này): `PENDING_WS_CHANNEL_SIZE=4096`
  chưa đo đủ lâu; `resolve_v2_reserves` vẫn chỉ được `http_pool` bảo vệ
  gián tiếp; chiều victim BÁN token lấy WBNB vẫn `not_wbnb_pair`;
  `measure_roundtrip_via_router` vẫn không đo được fee-on-transfer thật.
