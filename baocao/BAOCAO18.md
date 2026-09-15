1. LÁT: `7.3`-nối-dây — đổi `src/main.rs::handle_paper_tx` từ gọi
`pipeline::decide_paper_v2(...)` sang `pipeline::decide_and_build_paper_v2(...)`
(thêm tham số `&app_state.logger`), đúng 1 dòng, không đổi logic đọc kết quả
phía sau. Đóng dứt điểm nợ "còn 1 dòng nối dây" ghi ở BAOCAO16 ô 10/BAOCAO17
ô 10.

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md, config.toml,
      baocao/BAOCAO16.md, baocao/BAOCAO17.md.

LÁT: 7.3-nối-dây — đổi src/main.rs::handle_paper_tx (dòng gọi hiện ở
     main.rs:777) từ pipeline::decide_paper_v2(...) sang
     pipeline::decide_and_build_paper_v2(...), thêm tham số &app_state.logger,
     KHÔNG đổi logic đọc kết quả phía sau. Đây là 1 dòng, không phải viết lại
     pipeline.

ĐƯỢC ĐỤNG: src/main.rs (chỉ dòng gọi + import nếu thiếu), docs/STATE.md,
           docs/TASKS.md, baocao/BAOCAO18.md

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, bật cờ live,
     sendRawTransaction dưới bất kỳ hình thức, đổi pair khỏi WBNB,
     hồi sinh chiều victim bán, sửa pipeline.rs/executor.rs/calldata.rs
     (API đã đóng, giữ nguyên).

LÀM:
- Đổi đúng lời gọi ở main.rs, build sạch.
- Test xác nhận live loop paper thật giờ có sinh event tx.build khi có
  victim tx hợp lệ mô phỏng (test tích hợp hoặc test hiện có mở rộng).
- Verify lại "0 sendRaw" vẫn đúng sau khi nối dây (test cũ chạy lại).

KHÔNG LÀM: gửi tx thật, đổi stack, pair không-WBNB, hồi sinh chiều bán.
NỢ: không để lại "còn 1 dòng nối dây" nữa — đóng dứt điểm nợ này.

ĐẠT CẦN DÁN: cargo test + cargo build --release, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO18.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

3. FILE ĐỔI:
- `src/main.rs` — dòng 777 (trong `handle_paper_tx`), đổi ĐÚNG 1 dòng:
  ```rust
  // TRƯỚC:
  pipeline::decide_paper_v2(&victims, &pairbook, &tax_cache, &cfg, &risk, &input)
  // SAU:
  pipeline::decide_and_build_paper_v2(&victims, &pairbook, &tax_cache, &cfg, &risk, &app_state.logger, &input)
  ```
  Không thêm import — `use bsc_sandwich::pipeline::{self, PipelineOutcome};` (dòng 8)
  đã có sẵn, gọi qua `pipeline::decide_and_build_paper_v2` là đủ, hàm này
  `pub` sẵn từ BAOCAO16 (`src/pipeline.rs:363`). Logic đọc kết quả phía sau
  (`log_outcome_v2`, đếm `skip_counts`) giữ NGUYÊN, không sửa gì thêm — đúng
  chữ ký trả về vẫn `(PipelineOutcome, &'static str)` y hệt `decide_paper_v2`
  cũ.
- `docs/STATE.md` — thêm mục `7.3-nối-dây` (cuối file).
- `docs/TASKS.md` — dòng `7.3` cập nhật trạng thái + xoá nợ "1 dòng nối dây"
  khỏi mục Nợ.
- `baocao/BAOCAO18.md` — MỚI (file này).

KHÔNG ĐỤNG: `src/pipeline.rs`, `src/executor.rs`, `src/calldata.rs` (đúng
CẤM — API giữ nguyên, không sửa 1 ký tự), `src/web.rs`, `src/logger.rs`,
`src/venues.rs`, `src/decoder.rs`, `src/sim_v2.rs`, `src/pairbook.rs`,
`src/tax.rs`, `src/transport.rs`, `src/victims.rs`, `src/state.rs`,
`src/config.rs`, `Cargo.toml`, `config.toml` thật, `DEX_REGISTRY.md`,
`CLAUDE.md`, `victims.txt` thật, `.env`.

4. LỆNH CHẠY:
```
cargo test 2>&1 | tail -25
cargo build --release 2>&1 | tail -10
cargo build --release 2>&1 | grep -i warn
grep -rn "send_raw_transaction\|sendRawTransaction" src/ --include="*.rs"
git diff --stat -- Cargo.lock Cargo.toml
```
Cộng verify runtime THẬT (boot binary thật, không phải mock) — xem ô 5 phần
dưới, dùng `.env` thật của chủ (chỉ ĐỌC để lấy `BSC_HTTP`/`BSC_WS`, không sửa
file) + config/victims SCRATCH ngoài repo (port `18795`, không đụng
`config.toml`/`victims.txt` thật), `state/`/`logs/` dùng đúng thư mục
gitignored thật của repo (đúng tiền lệ BAOCAO05-09).

5. OUTPUT THẬT:

```
$ cargo test 2>&1 | tail -25
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 173 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.10s

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

173 passed y hệt BAOCAO16/17 (không đổi số test nào — lệnh này không thêm
test unit mới vào `src/*.rs` lib crate vì `main.rs` là binary crate riêng,
không có test module ở đó; bằng chứng "sinh event `tx.build`" nằm ở phần
verify RUNTIME THẬT dưới, không phải `cargo test`, vì `handle_paper_tx` là
hàm `private` của binary `main.rs`, cần `Provider`/`RwLock<AppStateInner>`
thật — mock lại toàn bộ trong unit test sẽ trùng lặp đúng thứ BAOCAO16 đã
test (`build_and_log_paper_sandwich`/`decide_and_build_paper_v2` đã có 22
test xanh), còn cái CHƯA từng được chứng minh là "dây nối thật trong
`main.rs` có chạy" — nên chọn "test tích hợp" bằng binary thật, đúng khuôn
BAOCAO05/06/07/08/09 đã dùng nhiều lần).

```
$ cargo build --release 2>&1 | tail -10
    Finished `release` profile [optimized] target(s) in 0.47s
```
```
$ cargo build --release 2>&1 | grep -i warn
(rỗng — không có warning nào)
```
```
$ grep -rn "send_raw_transaction\|sendRawTransaction" src/ --include="*.rs"
src/executor.rs:4://! hàm gửi giao dịch (`send_raw_transaction` hay tương đương) nào tồn tại
src/executor.rs:94:/// (`B256`) — CHỈ ĐỌC/PARSE, KHÔNG `sign_transaction`/`send_raw_transaction`
src/executor.rs:116:// CÓ ĐƯỜNG NÀO dẫn tới ký/gửi (test `no_send_raw_transaction_call_anywhere_in_src`
src/executor.rs:117:// dưới xác nhận cả repo không có lời gọi `send_raw_transaction` nào).
src/executor.rs:549:    /// `send_raw_transaction`/`sendRawTransaction(...)` nào tồn tại trong repo
src/executor.rs:557:    fn no_send_raw_transaction_call_anywhere_in_src() {
src/executor.rs:560:        // "vi pham" (chuoi "send_raw_transaction(" lien tuc chi ton tai o day
src/executor.rs:590:        assert!(offending.is_empty(), "tim thay loi GOI send_raw_transaction trong code (cam tuyet doi phien nay): {offending:?}");
src/transport.rs:2://! không có hàm gửi giao dịch nào (không `send_raw_transaction`).
```
Toàn bộ kết quả chỉ là comment/tên test/chuỗi tìm kiếm trong chính test —
KHÔNG có lời gọi thật nào, kể cả `src/main.rs` (0 dòng khớp). Test
`no_send_raw_transaction_call_anywhere_in_src` (trong `173 passed` ở trên)
xác nhận lại y hệt qua `cargo test`.
```
$ git diff --stat -- Cargo.lock Cargo.toml
(rỗng — không có output, không thêm crate/feature nào)
```

**Verify RUNTIME THẬT — live loop thật giờ có sinh `tx.build`** (khác mọi
lần trước: đây là LẦN ĐẦU TIÊN `tx.build` xuất hiện từ chính `main.rs` thật,
không phải test đơn lẻ gọi `build_and_log_paper_sandwich` tay):

Setup: `config.toml`/`victims.txt` SCRATCH (port `18795`, ngoài repo, không
đụng file thật), `.env` thật của chủ (chỉ dùng `BSC_HTTP`/`BSC_WS` để boot,
không sửa/không in ra), `state/`+`logs/` dùng đúng thư mục gitignored thật
của repo (rỗng trước phiên này — `ls state logs` báo "No such file" — nên
không có dữ liệu cũ nào bị lẫn vào).

```
$ ./target/release/bsc_sandwich <scratch>/config_scratch.toml
bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
web dashboard bind tai http://127.0.0.1:18795 (dry_run=true)

$ curl -s http://127.0.0.1:18795/api/status
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":false,"last_block":121887412,"pending_source":"ws",
 "state":"WATCHING","uptime_sec":10}
```
RPC WSS thật của chủ kết nối được (`pending_source:"ws"`, `last_block` là
block BSC thật) — victim `0xaaaa...aaaa,0.01` (scratch, không phải
`victims.txt` thật) load đúng qua `/api/victims`.

Ghi `state/tax_inject.jsonl` (`0x55d398...97955,0,0` — token USDT thật, tax
0 bps) để mở khóa nhánh `honeypot_or_tax` (đúng luật `3.3`/tax-cache-inject:
chưa đo thì mặc định skip, đây là chủ/test tự điền tay qua API/file đã có
sẵn từ BAOCAO07, không phải hack mới), rồi ghi `state/inject_tx.jsonl` với 1
dòng CSV `from,value_wei,input_hex` (`swapExactETHForTokens`,
path=[WBNB,USDT], `value_wei` cố ý đặt RẤT lớn — `100000` BNB quy đổi wei —
chỉ để đảm bảo sandwich trên pool WBNB/USDT thật (rất sâu, sát 0 lợi nhuận
với victim nhỏ) có lợi nhuận đo được rõ ràng > 0 cho mục đích CHỨNG MINH dây
nối chạy; đây là paper/dry-run thuần túy, KHÔNG có BNB thật nào di chuyển,
không phải giả định số liệu — số lớn chỉ để bài test không bị chìm trong sai
số làm tròn số nguyên của pool quá sâu):

```
{"event":"tx.seen","from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","source":"inject",...}
{"event":"tx.build","front":{"label":"front_buy","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","value_wei":"5000000000000000000","calldata":"0x7ff36ab5..."},"back":{"label":"back_sell","to":"0x10ed43c718714eb63d5aa57b78b54704e256024e","value_wei":"0","calldata":"0x18cbafe5..."},"deadline":"1789411451","token":"0x55d398326f99059ff775485246999027b3197955","victim_from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","self_address_placeholder":true,...}
```

Đây là dòng `tx.build` THẬT sinh ra từ `main.rs::handle_paper_tx` (KHÔNG
phải test gọi tay `build_and_log_paper_sandwich`) — bằng chứng trực tiếp dây
nối `main.rs:777` hoạt động: `to` cả 2 tx = V2 Router đã pin
(`0x10ed43c718714eb63d5aa57b78b54704e256024e`, khớp `DEX_REGISTRY.md`),
`front.value_wei = 5000000000000000000` (đúng 5 BNB = `max_front_bnb` scratch,
bị chặn trần đúng như `effective_front_cap_wei`), `token` = USDT thật,
`self_address_placeholder:true` (đúng thiết kế `7.3`, chưa có địa chỉ ví
thật). 2 lần thử ĐẦU (trước khi kịp inject tax/trước khi đủ profit) ra
`unprofitable`/`honeypot_or_tax` — đúng luồng skip bình thường của pipeline,
KHÔNG phải lỗi dây nối, chỉ là setup test chưa đủ điều kiện lần đầu (tax
cache hết hạn giữa 2 block vì BSC block time nhanh hơn dự kiến — pool
WBNB/USDT thật advance ~95 block trong ~30s test).

`GET /api/tax` xác nhận cache có entry `fresh:true` ngay trước lần thành
công; halt qua `POST /api/control {"action":"halt"}` để dừng bot sạch cuối
phiên (`{"action":"halt","ok":true}`).

6. CHAIN: `chain_id=56` xác nhận qua `/api/status` (`"chain_id":56`).
`eth_getCode`: không cần cho phiên này (không đụng registry). RPC WSS thật
của chủ (`.env`) kết nối được, `last_block` tăng thật theo thời gian chạy
(`121887412` → `121887521` → cao hơn nữa lúc inject thành công) — verify
bằng số block thật tăng dần, không bịa. Router dùng trong `tx.build` đối
chiếu bằng mắt khớp `DEX_REGISTRY.md` (`0x10ED43C718714eb63d5aA57B78B54704E256024E`,
chỉ khác hoa/thường).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (ngoài
phạm vi `ĐƯỢC ĐỤNG` phiên này).

8. KHÔNG LÀM:
- Không sửa `pipeline.rs`/`executor.rs`/`calldata.rs` — đúng CẤM, `git diff`
  các file này rỗng (chỉ `main.rs` bị sửa, đúng 1 dòng).
- Không `sendRaw`/bật `allow_live`/`dry_run=false`/`bot_armed` trong
  `config.toml` thật — không đụng file này, chỉ dùng bản SCRATCH ngoài repo
  cho verify runtime.
- Không đụng `victims.txt`/`.env` thật — dùng bản scratch/chỉ đọc `.env` để
  lấy RPC URL boot binary, không sửa/in nội dung `.env`.
- Không hồi sinh chiều victim bán — `decide_and_build_paper_v2` (không đổi
  từ BAOCAO16) vẫn chỉ đọc nhánh `Simulated` từ `decide_paper_v2` (chỉ xử lý
  chiều mua từ BAOCAO14), không thêm nhánh nào.
- Không có lời gọi `sendRawTransaction`/tương đương nào — xác nhận LẠI bằng
  cả `grep` toàn `src/` (ô 5, 0 lời gọi thật) lẫn test
  `no_send_raw_transaction_call_anywhere_in_src` (chạy lại xanh, 173/173).
- Không tách test/docs ra phiên sau — `docs/STATE.md`/`docs/TASKS.md` đã cập
  nhật cùng phiên này (xem ô 3).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- `to` (địa chỉ nhận) vẫn là placeholder `Address::ZERO` (log runtime thật ở
  ô 5 không hiện field `to` cấp cao nhất trong ví dụ rút gọn nhưng
  `self_address_placeholder:true` xác nhận đúng — không đổi từ BAOCAO16, nợ
  cũ, cần `alloy-signer-local`/`k256` để dẫn xuất địa chỉ ví thật từ `B256`,
  ngoài phạm vi phiên này).
- `deadline` vẫn wall-clock (`SystemTime::now()` + buffer), không phải
  `block.timestamp` on-chain thật — nợ cũ từ BAOCAO16, không đổi.
- Chỉ build calldata V2 Router (đúng phạm vi `7.2`/`7.3`) — V3/UR/V4 vẫn
  không có hàm build calldata nào.
- `RiskGuard::record_result` vẫn CHƯA gọi tự động ở đâu (nợ cũ — cần
  executor LIVE thật `7.x` biết kết quả lỗ/lãi thật).
- Gửi tx thật (`sendRawTransaction`) vẫn HOÀN TOÀN CHƯA LÀM — không có hàm
  ký/gửi nào trong repo, xác nhận lại ô 5/8. Đây là việc DUY NHẤT còn lại để
  `7.3` hoàn tất thật sự (`7.1` gate + signer, `7.2` calldata, `7.3` build+log
  paper đều đã xong và giờ đã NỐI DÂY vào live loop thật — chỉ còn bước ký +
  `eth_sendRawTransaction` thật, cần lệnh Grok riêng, có cân nhắc rủi ro tiền
  thật, KHÔNG tự làm).
- Setup verify runtime phiên này dùng `value_wei` victim cố tình rất lớn
  (`100000` BNB quy đổi) để né sai số làm tròn của pool WBNB/USDT thật quá
  sâu — đây CHỈ là kỹ thuật test (paper/dry-run, không tiền thật di chuyển),
  không phải giả định profit thật cho victim nhỏ thực tế; với victim thực tế
  nhỏ (đúng `min_swap_bnb` cỡ `0.01-0.5` như `victims.txt` thật), phần lớn
  pool WBNB sâu sẽ tự nhiên rơi vào `unprofitable` (đúng hành vi mong muốn —
  bot không nên sandwich nếu lợi nhuận sau phí quá nhỏ), không phải lỗi.
