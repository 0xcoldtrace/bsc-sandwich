1. LÁT: pair mode (PairBook) + `7.1` live gate + victim sell direction + deploy script fix
(4 việc dính liền theo lệnh, làm cùng 1 phiên, không nợ vụn).

2. LỆNH NHẬN:
```
ĐỌC: CLAUDE.md, docs/STATE.md, docs/TASKS.md, DEX_REGISTRY.md,
      config.toml, baocao/BAOCAO12.md.

LÁT: pair mode (PairBook) + 7.1 live gate + victim sell direction + deploy script fix
     Làm nốt test/docs/logger dính trong cụm này. Stack = Rust. Không npm/viem.

ĐƯỢC ĐỤNG: src/pairbook.rs (tạo mới), src/executor.rs, src/pipeline.rs,
  src/sim_v2.rs, src/config.rs, src/web.rs, src/main.rs, src/lib.rs,
  config.toml, pairs.txt (tạo mới, không địa chỉ thật), scripts/deploy_vps.sh,
  scripts/deploy_vps.ps1, docs/STATE.md, docs/TASKS.md, baocao/BAOCAO13.md.

CẤM: CLAUDE.md, victims.txt thật, .env, DEX_REGISTRY.md, bật cờ
  live/dry_run=false/bot_armed, sendRaw bất kỳ, đổi pair khỏi WBNB.
```
(Nguyên văn đầy đủ chi tiết A-G trong lệnh chat — xem transcript phiên).

3. FILE ĐỔI:
- `Cargo.toml` — thử thêm feature `signer-local` cho `alloy` rồi PHẢI BỎ (xem
  ô 8) vì lỗi resolve dependency, giữ nguyên như cũ (không đổi so BAOCAO12).
- `src/pairbook.rs` — MỚI. `PairBook` (theo dõi POOL token/WBNB qua
  `pairs.txt`), trait `PairResolver` (`RpcPairResolver` production +
  `MockResolver` test), resolve 10 concurrent/timeout 3s, `reload`/
  `reload_if_due` cùng khuôn `VictimBook`.
- `src/config.rs` — thêm field `pairs_path`/`pairs_reload_sec`/
  `pairs_min_swap_bnb` (validate + `pairs_min_swap_wei()`); thêm struct
  `RiskGuard` (`record_result`/`consecutive_loss_exceeded`/
  `front_cap_after_gas_reserve`, wire `max_consecutive_loss`/
  `gas_reserve_bnb_wei`).
- `src/sim_v2.rs` — thêm `simulate_front_then_victim_sell`/
  `search_max_front_in_sell` (chiều victim BÁN token) + fixture test tay.
- `src/pipeline.rs` — thêm `SwapDirection`, `decode_and_classify`,
  `precheck_token_only`, `PaperDecisionV2`, `evaluate_candidate` (lõi dùng
  chung), `decide_paper_v2` (dual-branch wallet|pair), `log_outcome_v2`. Đổi
  chữ ký `resolve_v2_reserves` trả thêm `pair_addr`. `decide_paper` GỐC GIỮ
  NGUYÊN (không sửa, mọi test cũ vẫn pass).
- `src/executor.rs` — thêm `LiveGateStatus`/`gate_check` (7.1 live gate chi
  tiết) + `load_signer` (đọc/validate `PRIVATE_KEY`, trả `B256` — xem ô 8 lý
  do không dùng `PrivateKeySigner`). `can_send_live` cũ giữ nguyên.
- `src/web.rs` — thêm field `pairbook`/`risk_guard` vào `AppStateInner`,
  route + handler `GET /api/pairs`.
- `src/main.rs` — wire `PairBook` (task reload nền, cùng khuôn victims/
  config), `RiskGuard` vào `AppStateInner`; `handle_paper_tx` đổi sang gọi
  `pipeline::precheck_token_only` + `decide_paper_v2` + `log_outcome_v2`
  thay vì `precheck_without_reserves`/`decide_paper`/`log_outcome`.
- `src/lib.rs` — thêm `pub mod pairbook;`.
- `config.toml` — thêm 3 field `pairs_path`/`pairs_reload_sec`/
  `pairs_min_swap_bnb` (ship `"pairs.txt"`/`30`/`0.05`), giữ nguyên mọi giá
  trị cũ.
- `pairs.txt` — MỚI, chỉ có comment hướng dẫn định dạng, KHÔNG có địa chỉ
  thật (đúng lệnh CẤM bịa/điền pool thật).
- `web/index.html`/`web/app.js` — thêm khối "Pairs" (đọc `/api/pairs`) ngay
  sau khối "Victims".
- `scripts/deploy_vps.sh`/`scripts/deploy_vps.ps1` — nhánh `--run`/`-Run` đổi
  từ `nohup ... & disown` sang `systemd-run --unit=bsc-sandwich-paper
  --collect ...` (fallback in cảnh báo + dùng lại `nohup` nếu VPS không có
  `systemd-run`).
- `docs/STATE.md` — thêm mục "Pair-mode + 7.1 live gate + victim sell
  direction + deploy nohup->systemd-run" (đầy đủ quyết định kỹ thuật +
  PHÁT HIỆN TOÁN HỌC về chiều bán, xem ô 10).
- `docs/TASKS.md` — cập nhật dòng `7.1` (XONG, phạm vi rõ), thêm dòng
  "Pair-mode", thêm 5 mục nợ mới (sell-direction cần Grok review, RiskGuard
  record_result chưa gọi, nợ đơn vị amount_in chiều bán, tăng tải RPC
  pair-mode, 7.2/7.3 vẫn chưa làm).
- `baocao/BAOCAO13.md` — MỚI (file này).

KHÔNG ĐỤNG: `CLAUDE.md`, `victims.txt` thật, `.env`, `DEX_REGISTRY.md`, cờ
`allow_live`/`bot_armed`/`dry_run` (mọi nơi vẫn giữ `false`/`false`/`true`).

4. LỆNH CHẠY:
```
cargo test 2>&1
cargo build --release 2>&1 | tail -5
```
Verify runtime thật thêm (không bắt buộc theo lệnh nhưng làm để chắc chắn):
boot binary với config/victims/pairs SCRATCH (port `18793`, ngoài repo,
`scratch13/` xoá sau khi xong) — `curl /api/status`, `/api/pairs`,
`/api/victims`.

5. OUTPUT THẬT:
```
$ cargo test 2>&1 | tail -20
test result: ok. 158 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 4.08s
     Running unittests src\main.rs (target\debug\deps\bsc_sandwich-ccc2de570b43de27.exe)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src\bin\rpc_probe.rs (target\debug\deps\rpc_probe-2c9d69c9c0cabe6c.exe)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests bsc_sandwich
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
9 test bắt buộc theo lệnh (lọc từ output cargo test đầy đủ ở trên, đều `ok`):
```
test config::tests::risk_guard_consecutive_loss ... ok
test config::tests::risk_guard_gas_reserve ... ok
test executor::tests::gate_check_ok ... ok
test executor::tests::gate_check_fail ... ok
test pairbook::tests::pairbook_load_token_resolve_mock ... ok
test pairbook::tests::pairbook_load_direct_addr ... ok
test pipeline::tests::decide_paper_sell_direction ... ok
test pipeline::tests::decide_paper_pair_source ... ok
test pipeline::tests::decide_paper_wallet_source ... ok
```
```
$ cargo build --release 2>&1 | tail -5
    Finished `release` profile [optimized] target(s) in 21.44s
```
Không có `FAILED` nào trong toàn bộ `cargo test` (158 passed, 0 failed, 2
ignored [test RPC thật `#[ignore]` sẵn từ trước, không đổi]).

Verify runtime thật (scratch, port `18793`, `.env` rỗng nên KHÔNG có
provider — đúng kỳ vọng vì máy phiên này không có `.env` thật):
```
GET /api/status -> {"state":"WATCHING","dry_run":true,"allow_live":false,"bot_armed":false,"chain_id":56,...}
GET /api/pairs  -> {"count":0,"error_lines":0,"last_reload_sec_ago":3,"pairs":[]}
GET /api/victims -> {"count":2,"error_lines":0,"last_reload_sec_ago":3,"victims":[{"address":"0x2222...2222","min_swap_bnb":0.5},{"address":"0x1111...1111","min_swap_bnb":0.01}]}
logs/bot.jsonl: {"event":"pair.reload_skip","reason":"chua co provider HTTP, thu lai tick sau",...}
```
`pair.reload_skip` xác nhận task nền pair-reload chạy đúng luồng (due-check
→ không có provider → log skip, KHÔNG crash, thử lại tick sau).

6. CHAIN: Không pin/đổi address/contract nào phiên này (không đụng
`DEX_REGISTRY.md`/`src/venues.rs`). `chain_id: 56` xác nhận qua `/api/status`
ở bản build scratch phía trên (số thật từ `Config` đã load, không bịa).
`eth_getCode`: MISSING (không cần cho cụm này — không có contract mới nào
cần pin).

7. REGISTRY: KHÔNG đổi. `DEX_REGISTRY.md` giữ nguyên y hệt BAOCAO02 (không
nằm trong phạm vi ĐƯỢC ĐỤNG của lệnh này).

8. KHÔNG LÀM:
- Không bật `allow_live`/`dry_run=false`/`bot_armed` (giữ nguyên `false`/
  `true`/`false` ở mọi nơi, kể cả config scratch dùng để verify).
- Không `sendRaw`/`sign_transaction`/`buildRaw` bất kỳ — `load_signer` CHỈ
  đọc biến môi trường + validate độ dài hex, không có hàm ký/gửi nào tồn tại
  trong repo.
- Không dùng `alloy::signers::local::PrivateKeySigner` như lệnh gợi ý ban
  đầu — thử bật feature `signer-local` cho `alloy` làm `cargo check` FAIL
  NGAY ở bước resolve dependency:
  ```
  error: failed to select a version for the requirement `alloy-signer-local = "^2.4.2"`
  candidate versions found which didn't match: 2.4.1, 2.4.0, 2.3.0, ...
  required by package `alloy v2.4.2`
  ```
  CÙNG LỚP LỖI đã ghi ở `docs/STATE.md` mục "5.2" (`alloy-rpc-types-txpool`
  chưa có bản `2.4.2` trên crates.io). Theo đúng quyết định đã chốt ở `5.2`
  ("KHÔNG hạ version `alloy` xuống `2.4.1`"), phiên này ĐÃ BỎ feature
  `signer-local` (Cargo.toml quay lại y hệt BAOCAO12), `load_signer` trả
  `alloy::primitives::B256` (32 byte thô, có sẵn không cần feature mới) thay
  vì `PrivateKeySigner` — đủ cho phạm vi `7.1` (đọc + validate, KHÔNG ký).
  Grok cần quyết định lệnh sau (chờ crate bắt kịp version, hay bơm `alloy`
  lên bản mới hơn) khi làm `7.3` thật.
- Không đổi pair khỏi WBNB (V2 factory/router vẫn y hệt pin cũ,
  `PairBook`/`decode_and_classify` vẫn bắt buộc 1 đầu path là WBNB đã pin).
- Không sửa `DEX_REGISTRY.md`.
- Không thay thế wallet mode bằng pair mode — `decide_paper` GỐC giữ nguyên
  100% (không sửa 1 dòng), mọi test cũ (BAOCAO01-12) vẫn pass y hệt;
  `decide_paper_v2` là entry point MỚI chạy SONG SONG, `main.rs` đổi sang gọi
  hàm mới nhưng lõi wallet-mode cũ vẫn còn nguyên trong `decide_paper` cho
  tham chiếu/test.
- Không chạy thật `scripts/deploy_vps.sh`/`.ps1` lên VPS (không có VPS mới
  được cấp phiên này) — chỉ verify cú pháp (`bash -n` xanh, PowerShell
  `Parser::ParseFile` không lỗi).
- **Không tự ý đảo thứ tự front/back cho chiều victim bán** dù đã CHỨNG MINH
  (không phải giả định — số tay + suy luận AMM) rằng thứ tự lệnh yêu cầu
  (front mua trước, back bán sau) luôn cho lợi nhuận ÂM khi victim bán khối
  lượng thật > 0. Triển khai ĐÚNG NGUYÊN VĂN lệnh, ghi rõ phát hiện, để Grok
  quyết định lệnh sau (xem ô 10 + `docs/STATE.md`).
- Không sim V4/Infinity (vẫn `hooks_unread`, không phải phạm vi lệnh này).
- Không gọi `RiskGuard::record_result` ở bất kỳ đâu trong live loop — paper
  mode không có kết quả thật để đếm lỗ (xem ô 10).

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- **Cần Grok quyết định — chiều victim BÁN token đang triển khai ĐÚNG lệnh
  nhưng CHỨNG MINH ĐƯỢC LUÔN LỖ**: `sim_v2::search_max_front_in_sell` theo
  đúng thứ tự "front mua trước, back bán sau victim bán" — số tay verify
  (pool 1000/1000, front_in=100): victim bán 100 token → profit=-12 (gas=0);
  victim bán 1000 token → profit=-77 (gas=0), càng victim bán nhiều càng lỗ
  nặng (test `sim_v2::tests::sell_direction_*`). Lý do: front mua đẩy giá
  token LÊN, victim bán sau đẩy giá XUỐNG, back bán ở giá đã thấp hơn giá
  attacker vừa mua — mua cao bán thấp, lỗ chắc chắn theo đúng cơ chế AMM
  constant-product (không phải bug code, là hệ quả toán học của đúng thứ tự
  lệnh yêu cầu). Sandwich THẬT có lãi ở chiều bán cần đảo NGƯỢC (front bán
  trước, back mua lại sau — "back-run" chuẩn). Test
  `decide_paper_sell_direction` verify WIRING đúng (định tuyến đúng hàm sim
  chiều bán, không rơi vào `not_wbnb_pair`/`decode_fail`) nhưng kết quả là
  `Skip(Unprofitable)`, KHÔNG PHẢI `Simulated` — trung thực theo phát hiện
  toán học, không bịa số dương giả. **Cần lệnh Grok**: giữ nguyên thứ tự
  (chiều bán sẽ luôn bị `unprofitable`, an toàn nhưng vô dụng cho chiều này)
  hay đảo front/back để có lãi thật.
- `RiskGuard::record_result` CHƯA được gọi tự động ở đâu — chỉ có cổng ĐỌC
  (`consecutive_loss_exceeded`/`front_cap_after_gas_reserve`) wire vào
  `decide_paper_v2`. Cần tầng executor thật (`7.x`) gọi `record_result` sau
  khi biết kết quả on-chain thật.
- Nợ đơn vị: `evaluate_candidate` so `amount_in` chiều BÁN (đơn vị token,
  decimals tuỳ token) trực tiếp với ngưỡng BNB-wei (`victims.txt`/
  `pairs_min_swap_bnb`) — lệch đơn vị kinh tế, chưa quy đổi qua reserve. Xem
  chi tiết `docs/TASKS.md`/`docs/STATE.md`.
- `PairBook`/`decide_paper_v2` làm MỌI tx decode được (không chỉ tx trong
  `victims.txt`) tốn thêm 1 `eth_call resolve_v2_reserves` — tăng tải RPC so
  `5.1`, chưa đo tải thật dài hạn với `pairs.txt` có nhiều entry thật (repo
  hiện `pairs.txt` rỗng/chỉ comment).
- `PairEntry` không lưu `min_swap_wei` riêng từng dòng (khác gợi ý struct ban
  đầu trong lệnh) — dùng THẲNG `cfg.pairs_min_swap_wei()` (GLOBAL, hot-reload
  sẵn) mỗi lần quyết định, đơn giản hoá có chủ đích, không mất chức năng.
- `7.2` (pin calldata router) và `7.3` (executor gửi tx thật) vẫn CHƯA LÀM —
  đúng roadmap, không nhảy cóc.
- `scripts/deploy_vps.sh`/`.ps1` sau khi sửa CHƯA được chạy thật lên VPS nào
  (không có VPS mới cấp phiên này) — chỉ verify cú pháp tĩnh.
