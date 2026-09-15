1. LÁT: 0.1+0.2+0.3+web (khung Rust+tokio, VictimBook, state+logger, web dashboard axum) — repo trống ban đầu, tạo mới toàn bộ cây file trong một phiên.

2. LỆNH NHẬN:
ĐỌC: CLAUDE.md (toàn bộ). Repo trống -> TẠO cây file.
LÁT: 0.1+0.2+0.3 + web dashboard, stack Rust+tokio, RPC crate chốt alloy (ghi docs/STATE.md).
ĐƯỢC ĐỤNG: mọi file khung, src/, web/, docs/, baocao/, Cargo.toml, config.toml, vps.json.
CẤM: CLAUDE.md, .env, cờ live, send tx, bịa address V3/V4.
Yêu cầu chi tiết: cây file đủ, config.toml đủ field ship mặc định, VictimBook (parse+wei+hot-reload+dòng lỗi+trùng địa chỉ), State+logger (enum, halt/disarm/reset file, jsonl), web axum cùng binary tại 127.0.0.1:8787 với API health/status/victims/venues/hits/skips/control, test cargo test tối thiểu 6 case liệt kê trong lệnh.

3. FILE ĐỔI (tạo mới toàn bộ, repo trống trước phiên này):
- CLAUDE.md: giữ nguyên, không sửa.
- Cargo.toml, Cargo.lock (tự sinh), .gitignore (`.env state/ logs/ target/`)
- config.toml (đủ field CLAUDE.md yêu cầu, ship mặc định dry_run=true)
- vps.json, .env.example, README.md, DEX_REGISTRY.md
- docs/STATE.md (chốt RPC = alloy), docs/TASKS.md, docs/DOC_MAP.md
- baocao/README.md, baocao/BAOCAO01.md (file này)
- victims.example.txt, victims.txt (copy từ example, chưa phải ví thật của chủ)
- src/main.rs, src/config.rs, src/victims.rs, src/state.rs, src/logger.rs, src/executor.rs, src/venues.rs, src/web.rs
- web/index.html, web/style.css, web/app.js
- Crate thêm ngoài core (ghi theo yêu cầu "cùng phiên"): `alloy-primitives` (Address + EIP-55 parse cho victims.txt), `axum` + `tower-http` (web), `chrono` (timestamp log), `toml`/`serde`/`serde_json` (config/log), `anyhow` (main), `tempfile` (dev-dependency cho test).

4. LỆNH CHẠY:
```
cargo build
cargo test
cargo run            # đọc config.toml, bind 127.0.0.1:8787
curl http://127.0.0.1:PORT/api/health
curl http://127.0.0.1:PORT/api/status
```

5. OUTPUT THẬT:

`cargo test` (18/18 pass):
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-4b04a93bf0667247.exe)

running 18 tests
test state::tests::control_action_parse_unknown_is_none ... ok
test venues::tests::no_family_marked_pinned_yet ... ok
test victims::tests::bnb_to_wei_rejects_garbage ... ok
test victims::tests::bnb_to_wei_basic ... ok
test victims::tests::checksum_and_lowercase_same_wallet ... ok
test victims::tests::duplicate_address_last_line_wins ... ok
test victims::tests::garbage_lines_logged_and_skipped_no_panic ... ok
test logger::tests::tail_never_panics_on_missing_file ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test config::tests::dry_run_true_blocks_live_gate ... ok
test config::tests::chain_id_1_fails ... ok
test config::tests::missing_min_profit_bnb_fails ... ok
test executor::tests::halt_lock_blocks_live ... ok
test executor::tests::dry_run_blocks_live_even_if_everything_else_is_green ... ok
test config::tests::load_ok ... ok
test state::tests::halt_lock_roundtrip ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test logger::tests::log_and_tail_roundtrip ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo run` (dùng config.toml thật, port 8787 khai báo trong config):
```
bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
web dashboard bind tai http://127.0.0.1:8787 (dry_run=true)
```
Ghi chú thật: khi smoke-test lúc làm phiên này, cổng 8787 trên máy đang bị một
tiến trình `node.exe` KHÔNG liên quan (PID khác, có sẵn trên máy trước phiên
này) chiếm giữ, nên phần verify API bên dưới phải chạy tạm với
`web_port=18787` (copy config.toml, đổi đúng 1 dòng port) để tránh giả mạo kết
quả — code không đổi, chỉ đổi cổng khi test tay. `config.toml` thật vẫn giữ
`web_port = 8787` như yêu cầu.

Verify API thật (server chạy bằng config tạm cổng 18787):
```
GET /api/health   -> {"status":"ok"}
GET /api/status   -> {"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
                       "halt_lock":false,"last_block":null,
                       "live_gate":{"allow_live":false,"bot_armed":false,"chain_id_56":true,
                                    "not_dry_run":false,"not_halted":true},
                       "state":"WATCHING","uptime_sec":9}
GET /api/victims  -> {"count":2,"error_lines":0,"last_reload_sec_ago":9,
                       "victims":[{"address":"0x1111...1111","min_swap_bnb":0.01},
                                  {"address":"0x2222...2222","min_swap_bnb":0.5}]}
GET /api/venues   -> 4 family (V2,V3,V4/Infinity,Ban moi hon) deu pinned:false, DISABLED+source
GET /api/skips    -> 12 reason enum, tat ca 0
GET /api/hits     -> jsonl that (bot.start, victim.reload) doc tu logs/bot.jsonl
POST /api/control {"action":"halt"} -> {"action":"halt","ok":true}, ghi state/halt.lock that
GET /             -> HTTP 200 (index.html tu web/ qua ServeDir)
```
Sau khi verify xong đã dừng tiến trình test và xoá `state/`, `logs/` do smoke-test
sinh ra (rác cục bộ, đã trong `.gitignore`) để repo sạch cho lần chạy thật đầu
tiên của chủ.

6. CHAIN: 0x38 (56) — MISSING thật (chưa có kết nối RPC nào trong phiên này,
`2.1 HTTP/WSS` chưa làm). Không có `eth_getCode`/`eth_call` nào được gọi.
Đảm bảo duy nhất ở mức code: `Config::load` từ chối mọi `chain_id != 56` (test
`chain_id_1_fails`), không dùng chain nào khác 56 trong toàn bộ repo.

7. REGISTRY: `DEX_REGISTRY.md` — 0/4 family pinned. WBNB, V2 Factory/Router ở
mức "Pin dự kiến" (theo CLAUDE.md) nhưng CHƯA có `eth_getCode` thật nên đánh dấu
"DỰ KIẾN, chưa getCode phiên này", không tính là pinned. V3, V4/Infinity, bản
mới hơn: DISABLED + lý do "chưa tra được docs chính thức trong phiên này"
(chưa có tác vụ web fetch nào chạy) — không xoá khỏi bảng, không im lặng bỏ.
`src/venues.rs::registry_snapshot` khớp với bảng này, phục vụ `/api/venues`.

8. KHÔNG LÀM (đúng như lệnh cấm): không nối WSS/HTTP RPC thật, không viết
decoder swap, không pin V3/V4/WBNB/V2 thật (chỉ ghi "dự kiến"), không có hàm
gửi transaction nào (`src/executor.rs` chỉ có cổng kiểm tra điều kiện, có test
xác nhận không tồn tại đường gửi tx thật), không đổi `dry_run/allow_live/
bot_armed` khỏi mặc định false/true theo CLAUDE.md, không sửa CLAUDE.md,
không điền `victims.txt` bằng ví thật của chủ (chỉ copy từ example).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `1.1+1.2+1.3`: pin thật V2/V3/V4-Infinity/bản mới hơn (cần web fetch docs
  PancakeSwap chính thức + `eth_getCode` thật trên chain 56 — phiên này chưa
  có RPC nên chưa gọi được).
- `2.1`: nối HTTP/WSS thật (thêm alloy-provider/alloy-transport khi làm, ghi
  version cụ thể vào docs/STATE.md lúc đó).
- `2.2/2.3`: decoder router + resolve pool — cần `1.x` xong trước.
- `3.x/4.1/5.1`: math + pipeline paper — cần `2.x` xong trước.
- Môi trường: cổng 8787 trên máy hiện bị một `node.exe` không liên quan chiếm
  (không phải do bot này) — chủ cần tự giải phóng cổng hoặc đổi `web_port`
  trong `config.toml` trước khi chạy `cargo run` thật trên máy này.
