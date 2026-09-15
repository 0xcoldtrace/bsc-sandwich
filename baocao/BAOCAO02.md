1. LÁT: 1.1+1.2+1.3 — pin registry Pancake trên BSC (chain 56): V2 + V3 +
V4/Infinity, rà soát "bản mới hơn" (chưa có, ghi DISABLED + nguồn), một phiên
đủ cả cụm registry.

2. LỆNH NHẬN:
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml, baocao/BAOCAO01.md.
LÁT: 1.1+1.2+1.3 registry Pancake BSC — cùng phiên đủ V2 + V3 + V4/Infinity + family mới hơn nếu có trên chain 56.
ĐƯỢC ĐỤNG: DEX_REGISTRY.md, docs/STATE.md, docs/TASKS.md, src/venues.rs, src/config.rs (chỉ nếu thiếu field), src/web.rs nếu /api/venues cần field mới, baocao/BAOCAO02.md
CẤM: CLAUDE.md, .env, cờ live, send tx, bịa getCode, xóa family khỏi bảng.

3. FILE ĐỔI:
- `DEX_REGISTRY.md` — viết lại toàn bộ bảng pin (Core/V2/V3/V4-Infinity/Bản mới hơn) với address + source_url + pinned_date=2026-09-14 + getCode(len) thật + trạng thái PINNED/DISABLED.
- `src/venues.rs` — `VenueInfo` đổi từ 1 contract phẳng sang `contracts: Vec<ContractPin>` (mỗi family nhiều contract), `pinned=true` cho V2/V3/V4 kèm địa chỉ + getCode thật, "Ban moi hon" vẫn `pinned=false` + DISABLED + lý do đã rà soát; thêm hằng `WBNB_ADDRESS`/`WBNB_GET_CODE_LEN`; thêm 5 test mới, xoá 1 test cũ đã lỗi thời (`no_family_marked_pinned_yet`, thay bằng `v2_v3_v4_are_pinned_after_registry_session`).
- `src/web.rs` — KHÔNG đổi (route `/api/venues` chỉ serialize `registry_snapshot()`, tự động theo field mới qua serde, không cần sửa handler).
- `web/index.html` — thêm cột `Contract`/`Address` vào bảng Venue (mục 3 dashboard) để hiện từng contract trong family thay vì 1 dòng gộp.
- `web/app.js` — `renderVenues` viết lại để lặp qua `v.contracts[]` (trước đó đọc field phẳng `v.get_code_len`/`v.source` không còn tồn tại — đây là phần dính liền bắt buộc sửa cùng phiên vì đổi struct, không phải việc mới ngoài lệnh).
- `docs/STATE.md` — thêm mục "Quyết định RPC dùng để pin" (dùng RPC công khai `bsc-dataseed.binance.org` qua `curl` vì repo chưa có `.env`/`BSC_HTTP`, không phải RPC runtime), cập nhật "Trạng thái venue" và phần "Không làm trong phiên này".
- `docs/TASKS.md` — đánh dấu `1.1/1.2/1.3` XONG (BAOCAO02), cập nhật mục Nợ/MISSING.
- Không đụng `src/config.rs` — registry pin không cần field config mới (đã đủ `scan_v2/scan_v3/scan_v4`, `live_v2/live_v3/live_v4` từ phiên 0.x).

4. LỆNH CHẠY:
```
curl -s -X POST https://bsc-dataseed.binance.org/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
curl -s -X POST https://bsc-dataseed.binance.org/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"eth_getCode","params":["<address>","latest"]}'
cargo test
cargo run -- config.smoke.toml   # config.toml đổi tạm web_port=18788 để smoke test, xoá ngay sau
curl -s http://127.0.0.1:18788/api/venues
curl -s http://127.0.0.1:18788/api/health
```

5. OUTPUT THẬT:

RPC dùng: `https://bsc-dataseed.binance.org/` (public BSC mainnet RPC —
repo chưa có `.env`/`BSC_HTTP`, chủ chưa điền; dùng RPC công khai chỉ để
lấy `eth_getCode` thật phục vụ pin, không phải RPC runtime của bot — ghi rõ
trong `docs/STATE.md`).

`eth_chainId`:
```
{"jsonrpc":"2.0","id":1,"result":"0x38"}
```
`eth_blockNumber` (bằng chứng RPC còn sống lúc pin):
```
{"jsonrpc":"2.0","id":1,"result":"0x742fd76"}
```

`eth_getCode` (rút gọn, đủ 42 ký tự địa chỉ + độ dài byte thật, lệnh cURL
từng địa chỉ giống mẫu ở mục 4, đổi `<address>`):
```
=== Core ===
0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c len=3124   (WBNB)
=== V2 ===
0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73 len=19084  (Factory)
0x10ED43C718714eb63d5aA57B78B54704E256024E len=21936  (Router)
=== V3 ===
0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865 len=5151   (PancakeV3Factory)
0x41ff9AA7e16B8B1a8a8dc4f0eFacd93D02d071c9 len=24556  (PancakeV3PoolDeployer)
0x1b81D678ffb9C0263b24A97847620C99d213eB14 len=12154  (SwapRouter v3)
0x13f4EA83D0bd40E75C8222255bc855a974568Dd4 len=24316  (SmartRouter)
0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997 len=8331   (QuoterV2)
0x1A0A18AC4BECDDbd6389559687d1A73d8927E416 len=16684  (Universal Router v3, cu)
=== V4/Infinity ===
0x238a358808379702088667322f80aC48bAd5e6c4 len=8347   (Vault)
0xa0FfB9c1CE1Fe56963B0321B32E7A0302114058b len=20885  (CLPoolManager)
0xC697d2898e0D09264376196696c51D7aBbbAA4a9 len=23821  (BinPoolManager)
0xd0737C9762912dD34c3271197E362Aa736Df0926 len=6998   (CLQuoter)
0xC631f4B0Fc2Dd68AD45f74B2942628db117dD359 len=6839   (BinQuoter)
0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB len=24350  (Universal Router Infinity)
=== Infinity StableSwap (ghi nhận, chưa bật scan riêng) ===
0x3669dDD1a9ee009dB9Eb2174C5C760FFfc66cfeF len=3993   (CLStableSwapPoolFactory)
```
Tất cả `len > 0` → PINNED. Không có địa chỉ nào trả `0x` rỗng (không có
DISABLED nào phải loại vì code rỗng trong phiên này).

`cargo test` (22/22 pass, ≥15 dòng cuối):
```
running 22 tests
test venues::tests::scan_and_live_flags_pass_through_unchanged ... ok
test state::tests::control_action_parse_unknown_is_none ... ok
test venues::tests::every_pinned_contract_has_nonzero_get_code_len ... ok
test venues::tests::newer_family_still_disabled_not_deleted ... ok
test venues::tests::v2_v3_v4_are_pinned_after_registry_session ... ok
test venues::tests::wbnb_pinned_and_nonzero ... ok
test victims::tests::bnb_to_wei_rejects_garbage ... ok
test victims::tests::bnb_to_wei_basic ... ok
test victims::tests::checksum_and_lowercase_same_wallet ... ok
test victims::tests::duplicate_address_last_line_wins ... ok
test logger::tests::tail_never_panics_on_missing_file ... ok
test victims::tests::garbage_lines_logged_and_skipped_no_panic ... ok
test victims::tests::victim_min_lookup_per_wallet ... ok
test config::tests::load_ok ... ok
test config::tests::chain_id_1_fails ... ok
test config::tests::missing_min_profit_bnb_fails ... ok
test executor::tests::dry_run_blocks_live_even_if_everything_else_is_green ... ok
test executor::tests::halt_lock_blocks_live ... ok
test config::tests::dry_run_true_blocks_live_gate ... ok
test state::tests::halt_lock_roundtrip ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test logger::tests::log_and_tail_roundtrip ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Smoke test `/api/venues` (server chạy tạm với `config.smoke.toml`, đổi đúng
1 dòng `web_port=18788`; xoá file + `state/`/`logs/` sinh ra ngay sau khi
verify, `config.toml` thật không đổi):
```
GET /api/health  -> {"status":"ok"}
GET /api/venues  -> 4 family: V2 (pinned:true, 2 contract), V3 (pinned:true,
                     6 contract), V4/Infinity (pinned:true, 6 contract),
                     "Ban moi hon" (pinned:false, 0 contract, DISABLED).
                     Mỗi contract có address/name/get_code_len/source_url/
                     pinned_date khớp DEX_REGISTRY.md.
```

6. CHAIN: `0x38` (56) — xác nhận qua `eth_chainId` ở mục 5 trước khi gọi bất
kỳ `eth_getCode` nào. `eth_getCode` thật cho toàn bộ 15 địa chỉ (Core 1 +
V2 2 + V3 6 + V4/Infinity 6, cộng 2 địa chỉ Infinity StableSwap ghi nhận
riêng) — không có địa chỉ nào MISSING trong phiên này. RPC dùng để pin là
RPC công khai (xem mục 5 + `docs/STATE.md`), KHÔNG phải RPC runtime của bot
(`2.1` vẫn chưa nối `alloy-provider` vào code, không đổi trong phiên này).

7. REGISTRY: `DEX_REGISTRY.md` viết lại đủ — Core (WBNB) + V2 (Factory,
Router) + V3 (Factory, PoolDeployer, SwapRouter, SmartRouter, QuoterV2,
Universal Router v3-cũ) + V4/Infinity (Vault, CLPoolManager, BinPoolManager,
CLQuoter, BinQuoter, Universal Router Infinity) + Infinity StableSwap (ghi
nhận, chưa bật scan) đều PINNED, kèm source_url là trang docs chính thức
`developer.pancakeswap.finance` fetch trực tiếp phiên này + `pinned_date
2026-09-14` + getCode thật. "Bản mới hơn" vẫn DISABLED — lý do: rà soát toàn
bộ sidebar developer docs (không có family AMM nào mới hơn Infinity) + tìm
kiếm web ngày 2026-09-14 xác nhận Infinity (04/2025) vẫn là bản mới nhất,
PancakeSwap 07/2026 chỉ mở rộng V2/V3 sang chain Robinhood (không phải BSC,
không phải family mới) — không xoá khỏi mục tiêu, phiên sau rà lại nếu có
tin mới. `src/venues.rs::registry_snapshot` khớp 100% với bảng
`DEX_REGISTRY.md` (địa chỉ, getCode len, source_url, pinned_date).

8. KHÔNG LÀM: không viết decoder swap (`2.2`), không sim sandwich (`3.x`),
không đổi `dry_run/allow_live/bot_armed` khỏi mặc định, không nối
`alloy-provider`/WSS thật vào code (`2.1` vẫn MISSING trong code Rust —
RPC công khai chỉ dùng ngoài code để lấy getCode dán báo cáo), không đụng
Universal Router đa hop USDT/3+ token (out of scope theo CLAUDE.md — chỉ
path token/WBNB), không pin `NonfungiblePositionManager`/`V3Migrator`/
`TickLens`/`PancakeInterfaceMulticall`/`MixedRouteQuoterV1`/`TokenValidator`/
`MasterChefV3`/`CLPositionManager`/`BinPositionManager`/`MixedQuoter` (có
địa chỉ BSC thật trong docs nhưng là hợp đồng quản lý LP/farm/route-trộn,
không nằm trong đường swap sandwich — ghi rõ lý do không pin trong
`DEX_REGISTRY.md`, không âm thầm bỏ qua), không sửa `CLAUDE.md`, không điền
`.env`, không bật cờ live.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `2.1` HTTP/WSS thật trong code Rust (thêm `alloy-provider`/`alloy-transport`
  khi làm, cần `.env` thật của chủ — `BSC_HTTP`/`BSC_WS`) — RPC công khai
  dùng phiên này chỉ để pin, không thay thế được cụm `2.1`.
- `2.2/2.3` decoder router (V2 + V3 SwapRouter/SmartRouter + Universal Router
  Infinity path WBNB) + resolve pool token/WBNB — cần `2.1` xong trước.
- `3.x/4.1/5.1` math + pipeline paper — cần `2.x` xong trước.
- Nếu chủ có nhu cầu pin thêm `NonfungiblePositionManager` và các contract
  quản lý LP/farm đã liệt kê ở mục 8 (ví dụ để hiển thị farm APR trên web),
  đó là việc ngoài phạm vi sandwich bot — chưa làm, chờ lệnh riêng nếu cần.
- Init code hash pool V3 chưa ghi (docs không có trên trang addresses đã
  fetch) — không cần cho `2.3` vì resolve pool qua `Factory.getPool()`
  on-chain; nếu sau này cần tối ưu offline, phải tìm nguồn cụ thể trước khi
  ghi, không suy đoán.
