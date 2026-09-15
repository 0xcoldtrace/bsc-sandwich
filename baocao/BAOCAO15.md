1. LÁT: `7.2` — pin calldata router thật cho V2 Router đã pin
(`DEX_REGISTRY.md`): dựng lại calldata front-run/back-run THẬT (encode
`swapExactETHForTokens`/`swapExactTokensForETH`) từ `DecodedSwap` hiện có,
sẵn sàng cho executor `7.3` dùng — CHƯA gửi tx thật phiên này, chỉ build
`Vec<u8>` calldata + test roundtrip encode->decode->so khớp bit-for-bit bằng
chính `decoder::decode_swap_calldata`. Làm nốt test/docs dính cụm trong cùng
phiên.

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml,
      baocao/BAOCAO14.md.

LÁT: 7.2 — pin calldata router thật (thay vì chỉ decode path token/WBNB như
     hiện tại) cho V2 Router đã pin ở DEX_REGISTRY.md. Mục tiêu: từ
     DecodedSwap hiện có, dựng lại calldata front-run/back-run thật (encode
     swapExactETHForTokens / swapExactTokensForETH) sẵn sàng cho executor
     7.3 dùng — vẫn CHƯA gửi tx thật phiên này, chỉ build calldata + test
     roundtrip decode->encode->decode khớp bit-for-bit.
     Làm nốt việc dính (test/docs) trong cùng phiên, không nợ vụn.
stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/pipeline.rs, src/sim_v2.rs (nếu cần hàm phụ trợ), file mới
           src/calldata.rs (nếu hợp lý), docs/STATE.md, docs/TASKS.md,
           baocao/BAOCAO15.md

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, bật cờ
     live/dry_run=false/bot_armed, sendRaw bất kỳ, đổi pair khỏi WBNB,
     thêm lại bất kỳ logic chiều bán nào (đã bỏ hẳn ở BAOCAO14 — cấm hồi
     sinh dưới mọi hình thức).
```

3. FILE ĐỔI:
- `src/calldata.rs` — **MỚI**. `sol! { interface IPancakeV2Router02 { ... } }`
  khai đúng 2 chữ ký hàm V2 Router đã pin (khớp nguyên văn chữ ký
  `decoder.rs` dùng tính selector): `swapExactETHForTokens(uint256,address[],address,uint256)`
  (payable), `swapExactTokensForETH(uint256,uint256,address[],address,uint256)`.
  2 hàm public:
  - `encode_front_buy(wbnb, token, amount_out_min, to, deadline) -> Vec<u8>`
    — `path=[wbnb, token]`, `amountIn` KHÔNG nằm trong calldata (đúng ABI
    thật, `msg.value` mới là `amountIn`).
  - `encode_back_sell(token, wbnb, amount_in, amount_out_min, to, deadline)
    -> Vec<u8>` — `path=[token, wbnb]`, `amountIn` NẰM TRONG calldata.
  4 test: `encoded_selectors_match_decoder_well_known_constants` (đối chiếu
  4 byte đầu calldata `sol!` sinh ra khớp hằng số well-known
  `decoder::tests::well_known_selectors_match` đã tự verify riêng),
  `roundtrip_front_buy_encode_then_decode_matches_bit_for_bit`,
  `roundtrip_back_sell_encode_then_decode_matches_bit_for_bit`,
  `roundtrip_front_buy_nonzero_amount_out_min` — cả 3 roundtrip đều gọi
  THẲNG `decoder::decode_swap_calldata` (hàm `pipeline.rs` dùng cho mọi tx
  thật) trên calldata vừa encode, so khớp `selector_name`/`amount_in`/
  `amount_out_min`/`path.token_a`/`path.token_b`/`to`/`deadline`.
- `src/lib.rs` — thêm `pub mod calldata;` (bắt buộc để module mới được biên
  dịch/test — file này không nằm trong danh sách ĐƯỢC ĐỤNG gốc nhưng là
  wiring cơ học cần thiết, không thêm logic gì khác, chỉ 1 dòng khai module,
  giống cách `rpc-probe` từng thêm module qua file này ở phiên trước).
- `Cargo.toml` — thêm `"sol-types"` vào mảng `features` của dependency
  `alloy` ĐÃ PIN (không đổi version `"2"`) để dùng được macro `alloy::sol!`
  (file này cũng không nằm trong ĐƯỢC ĐỤNG gốc nhưng bắt buộc để bật tính
  năng lệnh yêu cầu — xem lý do "không thêm crate mới" ở ô 5/docs/STATE.md:
  `git diff --stat Cargo.lock` RỖNG sau khi build lại, xác nhận không có
  crate mới nào vào `Cargo.lock`, chỉ bật feature có sẵn trong cây phụ
  thuộc).
- `docs/STATE.md` — thêm mục "`7.2` — Pin calldata router thật, encode
  front-buy/back-sell" (cuối file): quyết định dùng `sol!` thay vì tự đóng
  gói byte tay (lý do rủi ro offset khi encode), chứng minh không thêm crate
  mới, ghi rõ `amount_out_min=0` (chưa có field slippage vì `config.rs` ngoài
  phạm vi ĐƯỢC ĐỤNG) và `deadline` là tham số caller cung cấp (chưa tự tính
  `block.timestamp`, việc của `7.3`), ghi rõ CHƯA wire vào `pipeline.rs`/
  `main.rs`/`executor.rs`.
- `docs/TASKS.md` — dòng `7.2` đổi từ "CHƯA LÀM" sang "XONG" (BAOCAO15) với
  mô tả phạm vi rút gọn; mục nợ cuối file thêm đoạn "`7.2` ĐÃ ĐÓNG ở
  BAOCAO15" liệt kê rõ phần còn lại cho `7.3` (nối `SandwichQuote` vào tham
  số, slippage config thật, tính deadline thật).
- `baocao/BAOCAO15.md` — MỚI (file này).

KHÔNG ĐỤNG: `src/pipeline.rs`, `src/sim_v2.rs` (không cần sửa — `calldata.rs`
là module độc lập, chưa wire vào 2 file này ở phiên này, xem ô 10),
`src/main.rs`, `src/web.rs`, `src/executor.rs`, `src/config.rs`,
`config.toml`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md`, `CLAUDE.md`.

4. LỆNH CHẠY:
```
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -5
```

5. OUTPUT THẬT:
```
$ cargo test 2>&1 | tail -25
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 159 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.10s

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
```
$ cargo build --release 2>&1 | tail -5
   Compiling alloy-core v1.7.3
   Compiling alloy v2.4.2
   Compiling bsc_sandwich v0.1.0 (C:\Users\Admin\Documents\bsc-sandwich)
    Finished `release` profile [optimized] target(s) in 22.45s
```

Đối chiếu số lượng test: BAOCAO14 có 155 passed. Phiên này thêm module mới
`calldata.rs` với đúng 4 test, không xoá test nào ở file khác → 155 + 4 = 159,
khớp đúng số `cargo test` in ra ở trên. Không `FAILED` nào, 2 `ignored` giữ
nguyên (test RPC thật `#[ignore]` từ trước, không đổi phiên này).

4 test riêng của `calldata.rs`, lọc chạy riêng để xác nhận:
```
$ cargo test 2>&1 | grep -E "^test calldata::tests"
test calldata::tests::encoded_selectors_match_decoder_well_known_constants ... ok
test calldata::tests::roundtrip_front_buy_nonzero_amount_out_min ... ok
test calldata::tests::roundtrip_front_buy_encode_then_decode_matches_bit_for_bit ... ok
test calldata::tests::roundtrip_back_sell_encode_then_decode_matches_bit_for_bit ... ok
```

Xác nhận không thêm crate mới vào `Cargo.lock` (chỉ bật feature `sol-types`
có sẵn trong cây phụ thuộc `alloy-core`/`alloy-contract`):
```
$ git diff --stat Cargo.lock
(rỗng — không có output)
```

`cargo build --release 2>&1 | grep -i warn` → rỗng (không có `warning` nào,
xác nhận không sót hàm/biến chết nào bị compiler cảnh báo).

Lần build ĐẦU TIÊN sau khi thêm feature `sol-types` từng FAIL 4 test do lỗi
tay của phiên này (địa chỉ test `self_addr()` có độ dài hex lẻ —
`"0x1111111111111111111111111111111111beef1"` sai định dạng), báo lỗi
`OddLength` khi parse `Address::from_str` — đã sửa thành địa chỉ hợp lệ
`0x2222222222222222222222222222222222222222` (đúng 40 hex char, cùng kiểu
địa chỉ test dùng sẵn trong `decoder.rs`), build lại xanh 159/159 như trên —
không phải lỗi logic encode/decode, chỉ lỗi fixture test tay.

6. CHAIN: Không đụng chain/pin phiên này (không sửa `DEX_REGISTRY.md`/
`src/venues.rs`). `chain_id: 56` không đổi. `eth_getCode`: MISSING (không
cần cho cụm này — `calldata.rs` là module thuần build bytes, không gọi
RPC/on-chain nào; V2 Router address dùng trong test là hằng số văn bản, đối
chiếu lại `DEX_REGISTRY.md` bằng mắt: đúng `0x10ED43C718714eb63d5aA57B78B54704E256024E`
đã pin, không phải địa chỉ mới cần `eth_getCode`).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (không
nằm trong phạm vi ĐƯỢC ĐỤNG của lệnh này).

8. KHÔNG LÀM:
- Không `sendRaw`/bật `allow_live`/`dry_run=false`/`bot_armed` — không đụng
  `config.toml` phiên này (file không nằm trong ĐƯỢC ĐỤNG).
- Không đổi pair khỏi WBNB — cả 2 hàm encode đều cố định 1 đầu path là
  `wbnb` (tham số truyền vào, caller luôn truyền `WBNB_ADDRESS` đã pin, xem
  test dùng `venues::WBNB_ADDRESS`).
- Không hồi sinh chiều victim bán dưới bất kỳ hình thức nào — `calldata.rs`
  không đụng `pipeline.rs`/`sim_v2.rs` nào liên quan chiều bán đã xoá ở
  BAOCAO14 (không sửa 2 file đó phiên này).
- Không nhảy sang `7.3` (executor gửi tx thật) — phiên này CHỈ build calldata
  thuần, không ký/gửi gì, không có `Signer`/`sendRawTransaction` nào trong
  `calldata.rs`.
- Không thêm field `config.toml`/`config.rs` mới (vd slippage) — `amount_out_min`
  là tham số hàm do caller tự truyền, không hardcode và không đọc `Config`.
- Không wire `calldata.rs` vào `pipeline.rs`/`main.rs`/`executor.rs` — đúng
  phạm vi lệnh ("sẵn sàng cho executor `7.3` dùng", không yêu cầu nối ngay).
- Không tách test/docs ra phiên sau — 4 test + `docs/STATE.md`/`docs/TASKS.md`
  đã cập nhật cùng phiên này.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `calldata.rs::encode_front_buy`/`encode_back_sell` CHƯA được gọi từ bất kỳ
  đâu trong `pipeline.rs`/`main.rs`/`executor.rs` — đây là việc của `7.3`:
  nối `sim_v2::SandwichQuote` (`front_in` cho `amount_in` của front-buy,
  `front_out` cho `amount_in` của back-sell) vào 2 hàm này khi build tx thật.
- `amount_out_min` hiện là tham số hàm, phiên này dùng `0` ở mọi test/call —
  chưa có field slippage thật trong `config.toml`/`config.rs` (ngoài phạm vi
  ĐƯỢC ĐỤNG lệnh này) để tính số khác `0`. Cần lệnh Grok riêng nếu muốn thêm
  field đó.
- `deadline` hiện là tham số hàm caller tự cung cấp — `calldata.rs` không tự
  gọi RPC lấy `block.timestamp` nào (module thuần, không có `Provider`).
  `7.3` cần tự tính `deadline` hợp lý (vd `block.timestamp + buffer`) khi
  build tx thật.
- Chưa có ABI cho V3/UR/V4 ở cụm calldata này — lệnh chỉ giao V2 Router,
  không mở rộng thêm venue nào khác phiên này (không nằm trong phạm vi
  `ĐƯỢC ĐỤNG`).
- Các nợ khác của BAOCAO13/14 không liên quan cụm này (RiskGuard::record_result
  chưa gọi tự động, deploy script chưa chạy thật lên VPS mới, buffer WSS
  4096 chưa đo dài hạn) giữ nguyên trạng thái CÒN NỢ như `docs/TASKS.md` đã
  ghi — không thuộc phạm vi lệnh phiên này.
