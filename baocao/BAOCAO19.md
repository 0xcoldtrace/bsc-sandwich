1. LÁT: `v4-pool-resolve` — thay `src/pool.rs::resolve_infinity_pool` từ
luôn trả `hooks_unread` (mọi token) sang THẬT SỰ quét sự kiện `Initialize`
qua `eth_getLogs` trên `CLPoolManager`/`BinPoolManager` đã pin, dựng lại
`PoolKey` thật + tính `PoolId` đúng công thức, cho cặp token/WBNB.

(Lưu ý: giữa phiên có 1 lệnh khác — `universal-pair-scan` — gửi chồng lên
qua tin nhắn. Đã hỏi lại chủ, chủ chọn làm nốt `v4-pool-resolve` trước, để
`universal-pair-scan` cho phiên sau/BAOCAO20. BAOCAO19 này CHỈ chứa
`v4-pool-resolve`, không có gì của `universal-pair-scan`.)

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md (mục "Resolve pool V4/Infinity"), docs/TASKS.md,
      DEX_REGISTRY.md, config.toml, baocao/BAOCAO18.md, src/pool.rs.

LÁT: v4-pool-resolve — thay `src/pool.rs::resolve_infinity_pool` từ luôn trả
     `hooks_unread` sang THẬT SỰ tìm `PoolKey` (currency0, currency1, hooks,
     poolManager, fee, parameters) cho cặp token/WBNB bằng cách quét sự kiện
     `Initialize` (log) qua `eth_getLogs` trên CLPoolManager
     (`0xa0FfB9c1CE1Fe56963B0321B32E7A0302114058b`) và BinPoolManager
     (`0xC697d2898e0D09264376196696c51D7aBbbAA4a9`) đã pin trong
     DEX_REGISTRY.md — đọc event ABI thật từ
     github.com/pancakeswap/infinity-core (PoolManager.sol/ICLPoolManager.sol/
     IBinPoolManager.sol, source thật, KHÔNG bịa signature event). Lọc log có
     currency0/currency1 khớp {token, WBNB} (2 chiều), decode `PoolKey` +
     tính `PoolId` đúng công thức trong PoolKey.sol (không đoán). Nếu KHÔNG
     tìm thấy log nào cho cặp đó → vẫn `hooks_unread` (skip pool đó, đúng
     luật CLAUDE.md, không tắt bot/family). Nếu tìm thấy nhiều pool cùng cặp
     (nhiều fee tier/hook khác nhau) → trả TẤT CẢ, để lát sau (sim) tự chọn
     max profit, không tự ý chọn 1 cái ở tầng resolve.
stack = Rust. Không npm/viem. Không thêm crate mới nếu tránh được (alloy đã
đủ eth_getLogs); nếu thật sự cần crate mới, ghi rõ lý do trong docs/STATE.md.

ĐƯỢC ĐỤNG: src/pool.rs, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO19.md,
           Cargo.toml (chỉ nếu bắt buộc, ghi rõ lý do)

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md (chỉ ĐỌC, không sửa
     — nếu phát hiện cần pin thêm block range khởi tạo Vault/PoolManager thì
     ghi vào docs/STATE.md, không tự sửa registry), bật cờ live, sendRaw,
     đổi pair khỏi WBNB, đụng src/sim_v3.rs/sim_v2.rs/executor.rs/pipeline.rs
     (việc SIM giá V4 dùng PoolKey mới này là cụm KHÁC, phiên sau).

LÀM:
- Hàm thật quét eth_getLogs thật (giới hạn block range hợp lý, không quét
  toàn chain từ block 0 — nếu cần biết block deploy PoolManager để làm mốc
  bắt đầu, tra eth_getCode/etherscan hoặc ghi MISSING nếu không tra được,
  không bịa số).
- Test roundtrip: log Initialize event giả lập (fixture) → decode đúng
  PoolKey → PoolId tính đúng công thức.
- Nếu RPC thật khả dụng phiên này, verify RUNTIME THẬT: quét thử 1 cặp
  WBNB/token có thật trên BSC đã biết có pool Infinity (chủ có thể không có
  gợi ý sẵn — nếu không tìm được ví dụ thật nào để verify sống, ghi rõ
  MISSING trong ô 5, không bịa kết quả).

KHÔNG LÀM: sim/quote giá V4 (đó là cụm khác), gửi tx thật, đổi stack, pair
không-WBNB, bật cờ live.
NỢ: nếu verify runtime thật không tìm được ví dụ pool Infinity thật nào để
test sống, ghi rõ trong CÒN NỢ, không giả lập kết quả runtime.

ĐẠT CẦN DÁN: cargo test + cargo build --release, ≥15 dòng output cuối.

VIẾT: baocao/BAOCAO19.md đủ 10 ô. Chữ: CHỜ GROK | FAIL | CHƯA XONG.
Cấm chữ ĐẠT.
```

3. FILE ĐỔI:
- `src/pool.rs` — thêm `InfinityPoolFamily` (enum Cl/Bin), `InfinityPoolMatch`
  (struct PoolKey đầy đủ + pool_id + family + pool_manager), `compute_pool_id`
  (pub, tính `PoolId = keccak256(192 byte PoolKey)`), `decode_initialize_log`
  (private, giải mã 1 log thật -> `InfinityPoolMatch`, tự đối chiếu PoolId
  tính lại với `id` on-chain, không khớp thì bỏ log), `scan_initialize_logs`
  (private async, chia block range thành chunk `5_000`, lọc topic0 + topic2/
  topic3 = `{token, WBNB}` cả 2 chiều trong 1 `Filter`), và thay hẳn
  `resolve_infinity_pool` từ `fn(Address) -> Result<(), PoolSkipReason>`
  (sync, luôn `Err(HooksUnread)`) sang
  `async fn(provider, cl_pool_manager, bin_pool_manager, token, from_block,
  to_block) -> Result<Result<Vec<InfinityPoolMatch>, PoolSkipReason>, String>`
  — đúng khuôn `resolve_v2_pair`/`resolve_v3_pool` (outer Result = lỗi RPC,
  inner = domain). Xoá test cũ `infinity_pool_resolve_is_hooks_unread_this_session`
  (không còn khớp API mới), thêm 12 test mới (8 test thuần + 1 test enum +
  3 test `#[ignore]` RPC thật) — chi tiết ở docs/STATE.md mục
  `v4-pool-resolve`.
- `docs/STATE.md` — thêm mục `v4-pool-resolve` ở cuối file: nguồn event ABI
  đọc trực tiếp từ `infinity-core`, chứng minh `PoolId` khớp bit-for-bit dữ
  liệu chain thật, API mới, 2 mục MISSING (block deploy PoolManager, ví dụ
  WBNB thật để verify sống) kèm đầy đủ bằng chứng đã thử.
- `docs/TASKS.md` — thêm dòng cụm mới vào bảng ("MỘT PHẦN", link BAOCAO19),
  cập nhật đoạn nợ `2.3` cũ (không còn "luôn hooks_unread") và đoạn nợ
  `sim_v3.rs` (ghi rõ `pool.rs` giờ CÓ THỂ tìm PoolKey nhưng chưa ai gọi).
- `baocao/BAOCAO19.md` — MỚI (file này).

KHÔNG ĐỤNG: `CLAUDE.md`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md` (chỉ
đọc), `src/sim_v3.rs`/`sim_v2.rs`/`executor.rs`/`pipeline.rs`/`main.rs`/
`web.rs`/`decoder.rs`/`venues.rs`/`config.rs`/`transport.rs`, `Cargo.toml`
(KHÔNG thêm crate/feature nào — `eth_getLogs`/`Filter`/`Log` đã có sẵn từ
feature `rpc-types-eth` bật từ phiên `2.1+2.2+2.3`).

4. LỆNH CHẠY:
```
cargo test 2>&1 | tail -30
cargo build --release 2>&1 | tail -10
cargo build --release 2>&1 | grep -i warn
grep -rn "send_raw_transaction|sendRawTransaction" src/ --include="*.rs"
git diff --stat -- Cargo.lock Cargo.toml
cargo test --lib pool:: -- --ignored --nocapture
```

5. OUTPUT THẬT:

```
$ cargo test 2>&1 | tail -30
test victims::tests::victim_min_lookup_per_wallet ... ok
test pipeline::tests::decide_and_build_paper_v2_logs_tx_build_when_simulated ... ok
test transport::tests::rpc_pool_empty_list_returns_none_no_panic ... ok
test victims::tests::reload_respects_interval_with_injected_clock ... ok
test transport::tests::rpc_pool_advance_and_reconnect_wraps_around ... ok
test transport::tests::rpc_pool_skips_wrong_chain_url_then_picks_correct_one ... ok
test transport::tests::rpc_pool_failover_log_redacts_token_in_query ... ok
test transport::tests::rpc_pool_failover_when_first_url_dead_picks_next ... ok
test transport::tests::rpc_pool_all_urls_dead_returns_none_no_panic ... ok

test result: ok. 180 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 4.10s

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

`180 passed` (trước phiên này ở BAOCAO18 là `173 passed`) — đúng +7 test mới
trong `pool.rs` (12 test mới thêm − xoá 1 test cũ = +11... thực tế đếm được
+7 vì 3 trong 12 test mới là `#[ignore]` không tính vào "passed", và 1 test
cũ bị xoá: 173 − 1 (xoá) + 9 (test thuần mới, không kể 3 ignore) = 181? Số
thật chạy ra là 180 — xem lại: test thuần mới thêm = 8 (`cl_initialize_topic0...`,
`bin_initialize_topic0...`, `compute_pool_id_matches_real_onchain_event`,
`decode_initialize_log_roundtrip_on_real_onchain_log`,
`decode_initialize_log_rejects_when_id_does_not_match_recomputed_pool_id`,
`decode_initialize_log_rejects_too_few_topics`,
`decode_initialize_log_rejects_short_data`, `infinity_pool_family_as_str`),
xoá 1 test cũ (`infinity_pool_resolve_is_hooks_unread_this_session`):
173 − 1 + 8 = 180, khớp đúng số thật in ra. `4 ignored` (trước là `2
ignored`) = 2 cũ + 2 mới (`real_rpc_scan_finds_known_historical_cl_initialize_log`,
`real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window`).

```
$ cargo build --release 2>&1 | tail -10
    Finished `release` profile [optimized] target(s) in 0.49s
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
src/executor.rs:562:        let needle_snake: String = format!("{}{}", "send_raw_transaction", "(");
src/executor.rs:590:        assert!(offending.is_empty(), ...);
src/transport.rs:2://! không có hàm gửi giao dịch nào (không `send_raw_transaction`).
```
Toàn bộ chỉ là comment/tên test — 0 lời gọi thật, `src/pool.rs` (file duy
nhất sửa phiên này) không xuất hiện trong kết quả grep này.
```
$ git diff --stat -- Cargo.lock Cargo.toml
(rỗng — không thêm crate/feature nào, đúng CẤM)
```

**Verify RUNTIME THẬT (RPC sống, không phải mock)** —
`cargo test --lib pool:: -- --ignored --nocapture`:
```
running 3 tests
real scan found pool_id = 0xb7fdb401951747b812b65ebdefb3aae879fc7cf4027de366b5011832d6e2080b
test pool::tests::real_rpc_scan_finds_known_historical_cl_initialize_log ... ok
V2 getPair(USDT, WBNB) real pair = 0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae
test pool::tests::real_rpc_v2_get_pair_wbnb_usdt ... ok
khong tim thay pool WBNB/USDT Infinity trong cua so nay - dung luat, skip pool do
test pool::tests::real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 181 filtered out; finished in 1.09s
```

`real_rpc_scan_finds_known_historical_cl_initialize_log`: gọi lại chính
`scan_initialize_logs` (hàm private, không phải mock) qua RPC công khai
`bsc-rpc.publicnode.com`, quét đúng block `121882370-121882385` trên
`CLPoolManager` pinned — tìm lại đúng 1 log `Initialize` THẬT (tx
`0xca9952e5...`, cặp USDT/`0x65e7a1...`), decode ra `pool_id` khớp CHÍNH XÁC
giá trị `id` on-chain thật (`0xb7fdb401...`, xác nhận lại lần nữa qua RPC
sống — trước đó đã verify tĩnh ở test `compute_pool_id_matches_real_onchain_event`).

`real_rpc_resolve_infinity_pool_runs_end_to_end_on_recent_window`: gọi
THẲNG hàm public `resolve_infinity_pool` (API mới) với cửa sổ ~9000 block
gần nhất tính động từ `get_block_number()` thật — chạy xong KHÔNG lỗi RPC,
trả đúng `Err(HooksUnread)` (không tìm thấy pool WBNB/USDT Infinity trong
cửa sổ này — kết quả THẬT, không phải giả lập; xem MISSING ô 10 để biết vì
sao không tìm được ví dụ WBNB thật nào trong tầm với).

Trước đó trong quá trình làm (không phải phần "ĐẠT CẦN DÁN" nhưng là bằng
chứng cốt lõi của cụm này — dán tóm tắt, log gốc đầy đủ ở `docs/STATE.md`):
đã dùng `curl` gọi `eth_getLogs` thật (không qua code Rust) trên
`bsc-rpc.publicnode.com`, tìm thấy log `Initialize` thật tại block
`121882377`, tính `PoolId` bằng Python (`pycryptodome.keccak`) theo đúng
công thức `PoolKey.sol`/`PoolId.sol` đọc trực tiếp từ GitHub — ra
`0xb7fdb401951747b812b65ebdefb3aae879fc7cf4027de366b5011832d6e2080b`, KHỚP
BIT-FOR-BIT với `id` (topic1) log trả về. Đây là bước xác nhận công thức
ĐÚNG trước khi viết code Rust, không phải suy đoán.

6. CHAIN: `0x38` (56) xác nhận qua `provider.get_chain_id()` trong
`real_rpc_v2_get_pair_wbnb_usdt` (test cũ, chạy lại xanh) và ngầm định qua
`ProviderBuilder::connect` thành công + `eth_getLogs`/`eth_blockNumber` trả
kết quả hợp lệ trong 2 test mới trên `bsc-rpc.publicnode.com`. `getCode`:
không cần cho phiên này (không sửa registry). `eth_getLogs` thật xác nhận
`CLPoolManager` (`0xa0FfB9c1CE1Fe56963B0321B32E7A0302114058b`, khớp
`DEX_REGISTRY.md`) có log `Initialize` thật trên chain — pin ĐÚNG.

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (chỉ
đọc trong phiên này, đúng CẤM).

8. KHÔNG LÀM:
- Không đụng `sim_v3.rs`/`sim_v2.rs`/`executor.rs`/`pipeline.rs`/`main.rs` —
  `resolve_infinity_pool` mới CHƯA được gọi ở đâu ngoài `pool.rs` (grep xác
  nhận, xem docs/STATE.md).
- Không sim/quote giá V4 — chỉ resolve `PoolKey`, không gọi `CLQuoter`/
  `BinQuoter`.
- Không gửi tx thật, không bật cờ live, không đổi pair khỏi WBNB.
- Không thêm crate/feature mới — `git diff --stat -- Cargo.toml Cargo.lock`
  rỗng (ô 5).
- Không sửa `DEX_REGISTRY.md` — chỉ đọc địa chỉ đã pin sẵn.
- Đã dọn 4 file JSON rác (`bin_logs.json`, `cl_logs_recheck.json`,
  `cl_init_logs.json`, `cl_logs_10k.json`) tạo ra trong lúc `curl` thử
  nghiệm ngoài repo git — không commit, không để lại trong working tree.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- **MISSING — block deploy `CLPoolManager`/`BinPoolManager`**: đã thử
  `eth_getCode` tại block cũ (lỗi "missing trie node" — node không archive),
  BscScan API V1+V2 (từ chối free access cho chain BSC), trang web BscScan
  (Cloudflare chặn), repo GitHub `infinity-core-bsc-base` (không có
  `broadcast/` commit) — không tra được, không bịa số. `resolve_infinity_pool`
  vì vậy KHÔNG có `from_block` mặc định, caller phải tự truyền. Xem chi tiết
  đầy đủ ở `docs/STATE.md` mục `v4-pool-resolve`.
- **MISSING — verify runtime thật với 1 cặp WBNB/token Infinity CÓ THẬT**:
  đã thử 8 RPC công khai khác nhau; `bsc-rpc.publicnode.com` cho quét tới
  `10.000` block lùi từ tip (free-tier), quét đủ 16 log `Initialize` CL thật
  trong cửa sổ đó — KHÔNG log nào ghép WBNB (nhiều pool ghép NATIVE BNB
  `address(0)` thay vì WBNB — phát hiện phụ, ghi ở docs/STATE.md). Nhiều khả
  năng pool Infinity/WBNB được tạo từ lúc ra mắt (04/2025), quá xa cửa sổ
  free-tier miễn phí hiện có. Bù lại, công thức `PoolId`/decode ĐÃ verify
  chính xác tuyệt đối với 1 log thật khác (không phải WBNB) + `#[ignore]`
  test gọi thẳng API public qua RPC sống chạy xanh (nhánh "không tìm thấy").
  Nhánh "tìm thấy thật với WBNB" CHƯA được test bằng RPC sống — chỉ test
  bằng logic thuần (fixture dữ liệu thật, không phải RPC call).
- **CHƯA wire vào sim/pipeline** (đúng CẤM, để dành cụm khác): `sim_v3.rs`/
  `pipeline.rs` chưa gọi `resolve_infinity_pool` mới — cần 1 cụm riêng để
  gọi `CLQuoter`/`BinQuoter` bằng `PoolKey` tìm được, tự chọn max profit khi
  có nhiều pool cùng cặp, rồi mới nối vào `decide_paper`/`decide_paper_v2`.
- Field `pair_scan_universal`/`universal-pair-scan` (lệnh khác gửi chồng
  giữa phiên) CHƯA làm gì — chủ đã xác nhận để phiên sau (BAOCAO20), không
  có code nào của việc đó trong BAOCAO19 này.
