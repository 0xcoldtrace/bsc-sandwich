# BSC Sandwich Bot — Hướng dẫn vận hành

Tài liệu này viết cho người **không cần biết Rust**. Mỗi bước là 1 lệnh
copy-dán được, kèm "kỳ vọng thấy gì" và "nếu sai thì xem đâu". Không sửa
`.env`/`config.toml`/`pairs.txt` chỉ bằng cách đọc README — luôn tự tay sửa
file rồi đối chiếu lại với phần tương ứng ở đây.

## Mục lục

1. [Bot làm gì](#1-bot-làm-gì)
2. [Cài đặt trên WSL](#2-cài-đặt-trên-wsl)
3. [Cấu hình](#3-cấu-hình)
4. [Quy trình vet `pairs.txt`](#4-quy-trình-vet-pairstxt)
5. [Chạy paper (dry-run)](#5-chạy-paper-dry-run)
6. [Dashboard](#6-dashboard)
7. [Đọc log `logs/bot.jsonl`](#7-đọc-log-logsbotjsonl)
8. [Dừng / khởi động / halt](#8-dừng--khởi-động--halt)
9. [Deploy VPS](#9-deploy-vps)
10. [Sự cố thường gặp](#10-sự-cố-thường-gặp)
11. [An toàn](#11-an-toàn)

---

## 1. Bot làm gì

- Bot theo dõi mempool BSC (chain `56`), tìm giao dịch của **nạn nhân đang
  mua** một token trong `pairs.txt` bằng WBNB hoặc USDT trên PancakeSwap
  (V2/V3/V4-Infinity), rồi mô phỏng sandwich (mua trước — victim mua — bán
  sau) để ước tính lợi nhuận.
- Chỉ giao dịch **pool đã được Chủ tự vet tay** và điền `vetted YYYY-MM-DD`
  vào `pairs.txt` (mode 2 — pair-mode). Tính năng theo dõi theo ví
  (`victims.txt`, mode 1) và quét mọi pool (mode 3, universal) đang **TẮT**
  theo mặc định, chỉ dùng cho thử nghiệm nếu tự bật trong `config.toml`.
- **Mặc định `dry_run=true`**: bot không bao giờ ký hay gửi giao dịch thật.
  Mọi kết quả "lãi/lỗ" chỉ là số mô phỏng ghi vào log + dashboard.
- Có một dashboard web local (chỉ đọc) để xem trạng thái bot, không dùng để
  gửi lệnh.
- Muốn bật gửi giao dịch thật (`live`) cần rất nhiều điều kiện (xem mục 11)
  và **hiện tại chưa có tính năng ký/gửi giao dịch nào trong bot** — phần đó
  chưa được xây dựng.

## 2. Cài đặt trên WSL

Chạy trong **WSL (Ubuntu)**, thư mục nên nằm trong hệ thống file Linux (ví
dụ `~/bsc-sandwich`), **không** để trong `/mnt/c/...` (chậm hơn nhiều).

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev curl git jq
```

Kỳ vọng: không có dòng `E:` (lỗi apt) ở cuối.

Cài Rust qua `rustup` (nếu máy chưa có):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Kỳ vọng: in ra `rustc 1.7x.x ...` và `cargo 1.7x.x ...`. Nếu báo "command
not found" — mở lại terminal mới hoặc chạy lại `source "$HOME/.cargo/env"`.

Lấy code (đã có sẵn thư mục thì bỏ qua bước `git clone`, chỉ cần `cd`):

```bash
cd ~/bsc-sandwich
```

Build:

```bash
cargo build --release
```

Kỳ vọng: dòng cuối `Finished \`release\` profile [optimized] target(s) in
...s`. Nếu lỗi liên quan `openssl`/`pkg-config` — xem mục 10.

Chạy test:

```bash
cargo test
```

Kỳ vọng: nhiều dòng `test result: ok. N passed; 0 failed; M ignored; ...`
(hiện tại N ~ 280+, M ~ 11 — số `ignored` là các test cần RPC mạng thật,
không chạy trong `cargo test` thường). Nếu có `FAILED` — đừng chạy bot, báo
lại nguyên văn dòng lỗi.

## 3. Cấu hình

### `.env` — bí mật + kết nối RPC

```bash
cp .env.example .env
nano .env   # hoặc trình soạn thảo bất kỳ
```

Điền tối thiểu 2 biến (xem chi tiết + comment trong chính file `.env.example`):

```
BSC_HTTP=https://bsc-dataseed1.bnbchain.org
BSC_WS=wss://bsc-rpc.publicnode.com
```

- `BSC_WS` **nên** trỏ vào host `publicnode` (hoặc nhà cung cấp WSS ổn định
  khác hỗ trợ `newPendingTransactions`) — bot cảnh báo trong log nếu nguồn
  pending không phải WS (`pending_source != "ws"`).
- `PRIVATE_KEY` và `PRIVATE_TX_URL` để **trống** ở giai đoạn paper — chưa
  cần, và bot cũng chưa có tính năng ký/gửi tx thật để dùng tới chúng.
- Không commit `.env` (đã có trong `.gitignore`).
- `BSC_HTTP_SIM` (cụm `econ-truth-latency-vps`, **tuỳ chọn**) — pool RPC
  RIÊNG cho revm fork (`pairs_vet_task`/`gas_units_boot_task`/validator),
  KHÁC `BSC_HTTP` (đường nóng). Rỗng = dùng lại danh sách `BSC_HTTP`. Điền
  riêng khi thấy log `pair.vet_error`/`gas.units_measure_error` báo lỗi dạng
  `-32000`/`not supported`/`method not found` (quan sát thật với một số node
  bloXroute/RPC riêng không hỗ trợ đủ method cho revm) — bot tự đánh dấu URL
  đó (`rpc.method_unsupported`) và chuyển URL kế trong `BSC_HTTP_SIM`, không
  cần restart.

### `config.toml` — ngưỡng vận hành

File này có ~40 field, thiếu field bất kỳ = bot từ chối chạy ("fail load").
**Không tự thêm/xoá field** — chỉ sửa GIÁ TRỊ. Sau khi lưu, bot tự đọc lại
trong tối đa `config_reload_sec` giây (mặc định 15s) — không cần build lại,
không cần restart.

12 field Chủ thường chỉnh (còn lại giữ nguyên trừ khi có lý do cụ thể):

| Field | Ship | Ý nghĩa |
|---|---|---|
| `min_profit_bnb` | `0.002` | Lợi nhuận tối thiểu (BNB) để coi 1 kèo là `Simulated`. Thấp hơn tổng gas 2 chiều sẽ bị cảnh báo lúc boot. |
| `max_front_bnb` | `5` | Trần BNB tối đa bot "bỏ ra" để front-run 1 kèo. |
| `min_reserve_wbnb` | `20` | Pool có ít hơn số WBNB này trong reserve → bỏ qua (`thin_liq`), tránh pool quá mỏng. |
| `max_exposure_bnb` | `5` | Trần tổng vốn rủi ro thứ 2 (đặt `0` = tắt, chỉ còn `max_front_bnb` chặn). |
| `pairs_min_swap_bnb` | `0.05` | Giao dịch của victim nhỏ hơn số này (quy đổi BNB) thì bỏ qua, không đáng front-run. |
| `pairs_require_vetted` | `true` | `true` = pool trong `pairs.txt` CHƯA có `vetted YYYY-MM-DD` hợp lệ thì KHÔNG được sim. Đừng tắt trừ khi biết rõ hậu quả. |
| `pairs_vet_interval_sec` | `600` | Chu kỳ (giây) bot tự đo lại tax/honeypot NỀN cho các pool đã vet (không chặn đường nóng). |
| `sim_engine` | `"v2"` | `"v2"` = dùng công thức đóng (nhanh, đúng cho token đã vet sạch). `"evm"` = mở fork EVM mô phỏng từng tx (chậm hơn nhiều, dùng để đối chiếu/kiểm tra). Chỉ nhận đúng 2 chuỗi này. |
| `scan_quote_usdt` | `false` | `true` = bật thêm nhánh quote USDT song song WBNB (victim mua bằng USDT). |
| `front_slippage_bps` | `10` | Trượt giá cho phép ở chân mua trước (0.10%). |
| `back_slippage_bps` | `50` | Trượt giá cho phép ở chân bán sau (0.50%, nới hơn vì giá đã dịch sau khi victim khớp). |
| `max_roundtrip_tax` | `0.005` | Tổng tax mua+bán tối đa cho phép (0.5%) — cao hơn thì coi `honeypot_or_tax`, bỏ qua. |
| `max_consecutive_loss` | `3` | Số lần "thua" liên tiếp (tín hiệu từ validator nội bộ) trước khi risk-guard chặn thêm kèo mới. |
| `gas_price_max_gwei` | `10` | Trần `eth_gasPrice` (gwei nguyên) — đo được cao hơn số này (mạng tắc nghẽn bất thường) thì coi `gas_cap`, bỏ qua kèo. |

Gas giờ được chặn theo **2 lớp** (cụm `real-economics-mode2`, F-03):
`gas_reserve_bnb_wei`/`front_max_gas_bnb_wei`/`back_max_gas_bnb_wei` (wei,
đã có từ trước) giờ CHỈ còn là TRẦN so với `gas_cost_wei` ĐO THẬT
(`eth_gasPrice` × gas unit đo bằng revm lúc boot — không còn dùng thẳng làm
chi phí gas như trước); `gas_units_front`/`gas_units_back` (ship
`160000`/`140000`) là số gas UNIT FALLBACK khi chưa đo được thật. Vượt trần
BẤT KỲ lớp nào → `gas_cap` (skip reason mới, xem mục 7). Dòng tổng kinh tế
"candidate=... net_pos=..." đọc qua `GET /api/econ` (mục 6).

Kiểm tra nhanh config đang hợp lệ (không cần chạy cả bot):

```bash
cargo test config::tests
```

Kỳ vọng: `test result: ok. N passed; 0 failed`.

## 4. Quy trình vet `pairs.txt`

Đây là bước **quan trọng nhất** để bot có việc để làm — mode 1 (`victims.txt`)
và mode 3 (universal) đang tắt, `pairs.txt` là nguồn candidate DUY NHẤT.

### a. Định dạng dòng

```
0xTokenAddress # SYMBOL | vetted YYYY-MM-DD | tax b/s | owner renounced|active | note
```

- Phần **trước** dấu `#`: địa chỉ token (hoặc `0xToken,0xWBNB`/`0xToken,0xUSDT`
  nếu cần chỉ rõ cặp — cột 2 phải đúng WBNB hoặc USDT đã pin, sai địa chỉ
  khác → lỗi dòng) — đây là phần DUY NHẤT bot dùng để xác định pool.
- Phần **sau** dấu `#`: chỉ có `vetted YYYY-MM-DD` được bot đọc và dùng để
  quyết định (thiếu hẳn, hoặc có chữ `vetted` nhưng không kèm ngày đúng định
  dạng → bot coi là **CHƯA VET**, không sim). Các trường còn lại
  (`SYMBOL`/`tax`/`owner`/`note`) chỉ là ghi chú cho người đọc, bot không
  parse.

Ví dụ **ĐÚNG** (đã vet, bot sẽ sim):
```
0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82 # CAKE | vetted 2026-09-16 | tax 0/0 | owner Pancake (mintable, MasterChef) | reserve_wbnb~=14002 BNB
```

Ví dụ **SAI** (chưa vet, bot bỏ qua — thiếu ngày):
```
0xAbc... # TOKEN | vetted | tax ?/? | owner ? | chưa soát
```

Dòng bắt đầu bằng `#` là comment nguyên dòng, bị bỏ qua hoàn toàn.

### b. Lọc thô bằng `scripts/vet_goplus.sh`

```bash
scripts/vet_goplus.sh pairs.txt
```

- Yêu cầu `curl` + `jq` (đã cài ở mục 2).
- Gọi API công khai GoPlus Security cho **từng** token (tuần tự, có nghỉ
  giữa các lần gọi) rồi in bảng `PASS` / `REVIEW` / `FAIL` + dòng tổng kết.
- Kỳ vọng: mỗi dòng có địa chỉ, verdict, symbol, lý do (nếu có cờ). Dòng cuối
  dạng `== TOM TAT: tong=100 PASS=30 REVIEW=41 FAIL=20 (loi_goi_API=9) ==`.

**Đọc verdict:**

| Verdict | Ý nghĩa | Việc cần làm |
|---|---|---|
| `PASS` | GoPlus không thấy cờ đỏ/vàng nào | Vẫn phải tự đọc BscScan (bước c) trước khi điền `vetted` — đây chỉ là lọc thô |
| `REVIEW` | Có ít nhất 1 cờ vàng (`owner_not_renounced`, `mintable`, `proxy_contract`, `anti_whale`, tax nhỏ, ít LP holder...) | Đọc kỹ contract trước khi quyết định, không tự động loại |
| `FAIL` | Có cờ đỏ (`honeypot`, `blacklist`, `trading_cooldown`, `transfer_pausable`, `can_take_back_ownership`, `hidden_owner`, tax > 5%) | Không điền `vetted`, cân nhắc xoá khỏi `pairs.txt` |

**Nếu bị rate-limit** (dòng `REVIEW ... khong goi duoc GoPlus
(rate_limit_or_network)`, hoặc tổng kết có `loi_goi_API > 0`): GoPlus chặn
burst rất chặt (~7-8 request liên tục). Script đã tự retry-backoff 3 lần
(5s/10s/20s) cho mỗi token — nếu vẫn lỗi, đợi vài phút rồi chạy lại **riêng
cho các token đó** (copy các dòng đó sang 1 file tạm rồi
`scripts/vet_goplus.sh file_tam.txt`). Không bao giờ coi lỗi rate-limit là
`PASS`.

### c. Checklist tự đọc BscScan (bắt buộc, script không thay được)

Với mỗi token dự định điền `vetted`, mở `https://bscscan.com/address/0x...`
(hoặc `token/0x...#code`) và kiểm tra bằng mắt:

- [ ] Contract đã **verified** (có tab "Contract" hiện source code, không
      phải bytecode thô).
- [ ] Không phải **proxy** ẩn logic thật ở nơi khác (nếu là proxy, phải đọc
      luôn implementation contract).
- [ ] Không có hàm chuyển tiền kèm **fee/tax** bất thường (đọc hàm
      `_transfer`/`transferFrom`, tìm `fee`/`tax`/`burn`).
- [ ] Không có **blacklist**/`isBlacklisted`/`_isExcluded` chặn địa chỉ tuỳ ý.
- [ ] Không có **cooldown**/anti-MEV (`lastTradeBlock`, `cooldownTime`,...)
      chặn giao dịch trong cùng block.
- [ ] Không **pausable** (`whenNotPaused`, `pause()`/`unpause()` cho owner).
- [ ] Không có `maxTxAmount`/`maxWalletAmount` quá thấp gây revert bất ngờ.
- [ ] Không **mint** tuỳ ý / không phải token **rebase** (supply tự đổi).
- [ ] Owner đã **renounce** (`owner() == 0x0`) hoặc, nếu chưa, hiểu rõ owner
      là ai và owner có quyền gì (mint/pause/blacklist).

Bất kỳ ô nào KHÔNG tick được → đừng điền `vetted`.

### d. Kiểm volume trên DexScreener

Mở `https://dexscreener.com/bsc/0x...` (địa chỉ token), chọn đúng **pool
"PancakeSwap V2"** quote WBNB hoặc USDT (không phải V3 — pool V2 là pool bot
đang sim). Kiểm:

- Volume 24h đủ lớn (có giao dịch thật xảy ra, không phải pool chết).
- Reserve/liquidity ≥ ngưỡng `min_reserve_wbnb`/`min_reserve_usdt` trong
  `config.toml` — pool mỏng hơn ngưỡng sẽ bị bot tự bỏ qua (`thin_liq`) dù
  đã điền `vetted`.

### e. Điền `vetted YYYY-MM-DD` + commit

Sau khi qua đủ b-c-d, tự tay sửa dòng trong `pairs.txt`:

```
0xTokenAddress # SYMBOL | vetted 2026-09-16 | tax 0/0 | owner renounced | note
```

Lưu file — bot tự đọc lại trong tối đa `pairs_reload_sec` giây (mặc định
30s), không cần restart. Commit thay đổi:

```bash
git add pairs.txt
git commit -m "vet: them token X vao pairs.txt"
```

### f. Bot tự đo lại (vet nền) — `pairs_vet_task`

Sau khi 1 pool có `vetted`, bot chạy **nền** (không chặn đường nóng) đo lại
tax/honeypot mỗi `pairs_vet_interval_sec` giây bằng cách mô phỏng thật qua
EVM (revm), KHÔNG tin tưởng mù quáng vào bước vet tay:

```bash
curl -s http://127.0.0.1:8787/api/pairs | jq
```

Kỳ vọng: mỗi pool có thêm cột `vetted_at`, `symbol`, `candidate`, `buy_bps`,
`sell_bps`, `honeypot`, `last_vet_sec_ago`. `candidate:true` nghĩa là pool
đang thực sự được dùng để sim (đã vet VÀ vet nền chưa phát hiện vấn đề).

`/api/pairs` còn có `pending_count`/`pending` (cụm `econ-truth-latency-vps`,
0.a) — dòng CHƯA resolve xong (RPC lỗi/timeout, hoặc "no pool" tạm thời),
mỗi dòng có `token`/`quote`/`attempts`/`last_error`/`last_attempt_sec_ago`.
Dòng pending tự retry backoff 5s/15s/60s ở các lần `pairs_reload_sec` kế
tiếp — KHÔNG rớt khỏi danh sách nếu trước đó ĐÃ resolve thành công (chỉ dòng
mới/chưa từng resolve mới rơi vào đây). `pending` cao kéo dài + `last_error`
lặp lại "not supported"/"-32000" → điền `BSC_HTTP_SIM` (xem mục 3) hoặc kiểm
tra `getPair` cho đúng cặp token/quote đã khai trong `pairs.txt`.

File `state/pairs_vetted.json` là bản chụp nhanh cùng dữ liệu (đọc nhanh
không cần `jq`/API):

```bash
cat state/pairs_vetted.json | jq
```

Nếu 1 pool đã điền `vetted` nhưng vet nền phát hiện tax/honeypot, log sẽ có
dòng `pair.vet_fail` — pool đó **tự động bị loại khỏi candidate** (không
tự xoá khỏi `pairs.txt`, chỉ ẩn khỏi runtime cho tới lần vet PASS kế tiếp).
Xem chi tiết field trong dòng log đó (`token`, `buy_bps`, `sell_bps`,
`honeypot`) để quyết định có nên xoá hẳn dòng đó khỏi `pairs.txt` hay không.

## 5. Chạy paper (dry-run)

```bash
scripts/paper_run.sh --minutes 30 --port 8799
```

Script tự: build lại, xoá `state/halt.lock` cũ, tạo 1 **bản config tạm**
(không đụng `config.toml` thật, chỉ hạ ngưỡng kinh tế về 0 để dễ quan sát),
chạy bot 30 phút, in kết quả, rồi tự halt sạch.

Kỳ vọng đầu ra (rút gọn, số thật sẽ khác):

```
== may chay: WSL (repo: ...) ==
== binary sha256 = <64 ký tự hex> ==
== git HEAD = <40 ký tự hex> ==
== pairs.txt (grep tho...): tong=121 vetted=45 chua_vet=76 ==
======== KET QUA SAU 30 PHUT ... ========
---- /api/skips ----
{"below_min":0,"decode_fail":...,"not_in_list":...,...}
---- /api/funnel ----
{...,"seen":..., "simulated":0, ...}
```

Đọc từng khối:

- **`/api/funnel`** — phễu theo đúng thứ tự gate thật:
  `seen → not_pancake_router → decode_fail → not_wbnb_pair →
  venue_v2|venue_v3 → no_pool → below_min → thin_liq → honeypot_or_tax →
  unprofitable → victim_would_revert → simulated`, cộng `sim_error` (số tx
  không mô phỏng được — cao bất thường thường là RPC quá tải, không phải
  bug). Số ở bước sau luôn ≤ số ở bước trước (phễu hẹp dần).
- **`/api/skips`** — tổng số lần mỗi lý do bỏ qua xuất hiện trong phiên chạy
  (không reset mỗi phút như `/api/funnel`).
- **`/api/tax`** — cache tax đo tự động (mục 4f) + có thể inject tay qua
  `POST /api/tax`.
- **`/api/validate`** — độ chính xác sim: `within_1pct_ratio` càng gần `1.0`
  càng tốt (dự đoán sim khớp với kết quả thật trên chain trong biên độ 1%).
  Cụm `real-economics-mode2` (F-27): tách riêng `isolated`/`non_isolated`
  (mỗi nhóm có `n`/`within_1pct`/`p50_lech_pct`/`p95_lech_pct`) — block **cô
  lập** (không tx nào khác chen vào cùng pool) thường khớp gần tuyệt đối,
  block **có tx khác chen vào** (điều kiện MEV thật) mới phản ánh đúng độ
  khó thật của việc dự đoán.
- Dòng `sim_engine="v2"` (ship mặc định) → `sim.evm`/`sim_error` trên đường
  nóng sẽ **RỖNG** trong lần chạy bình thường. Đây **không phải lỗi** — đường
  nóng dùng công thức đóng V2, không mở fork EVM mỗi tx (xem mục 1). Muốn
  quan sát lại đường EVM per-tx để đối chiếu, đổi tạm `sim_engine="evm"`
  trong `config.toml` (hot-reload, không cần build lại) rồi chạy lại.
- Số nào là "tốt": `simulated > 0` với `unprofitable`/`victim_would_revert`
  thấp là dấu hiệu tích cực. `not_in_list`/`decode_fail` cao là bình thường
  (đa số tx mempool không liên quan). `sim_error` cao, hoặc `venue_v2 = 0`
  suốt nhiều phút dù `seen` cao, là dấu hiệu XẤU (xem mục 10).
- Dòng tổng kết kinh tế dạng "candidate=… net_pos=…" đọc qua `GET /api/econ`
  (field `summary_line`, cụm `real-economics-mode2`) — xem mục 6.

## 6. Dashboard

Mở `http://127.0.0.1:8787` (hoặc port bạn truyền qua tham số/`web_port`) —
chỉ đọc, không có nút gửi giao dịch. Các khối chính: trạng thái bot, cổng
live (tick xanh/đỏ từng điều kiện), venue đã pin, victims, pairs, hit sống,
đếm skip, funnel, tax cache, validator.

API JSON dùng trực tiếp qua `curl`/`jq` nếu cần:

```
GET /api/health
GET /api/status
GET /api/victims
GET /api/venues
GET /api/pairs
GET /api/hits?limit=50
GET /api/skips
GET /api/funnel
GET /api/econ       (cụm real-economics-mode2 — bucket victim_in theo BNB, quote wbnb/usdt, top token, decode_fail theo router, latency, dòng tổng)
GET /api/validate
GET /api/tax        POST /api/tax   (inject tax thủ công, cần allow_tax_inject=true)
POST /api/control   (body {"action":"halt"|"disarm"|"reset"})
```

## 7. Đọc log `logs/bot.jsonl`

10 event quan trọng nhất + lệnh grep mẫu:

| Event | Ý nghĩa | Lệnh xem nhanh |
|---|---|---|
| `bot.start` | Bot vừa boot xong, đọc config/venue ban đầu | `grep '"event":"bot.start"' logs/bot.jsonl \| tail -1` |
| `pair.reload` | `pairs.txt` vừa được đọc lại (hot-reload) | `grep '"event":"pair.reload"' logs/bot.jsonl \| tail -5` |
| `pair.unvetted` | Có dòng trong `pairs.txt` chưa có `vetted` hợp lệ, bị loại khi reload | `grep '"event":"pair.unvetted"' logs/bot.jsonl \| tail -5` |
| `pair.vet_fail` | Vet nền (mục 4f) phát hiện pool có tax/honeypot, loại khỏi candidate | `grep '"event":"pair.vet_fail"' logs/bot.jsonl \| tail -5` |
| `tx.seen` | Bot vừa nhận 1 tx pending mới (từ WS/txpool/inject) | `grep '"event":"tx.seen"' logs/bot.jsonl \| tail -5` |
| `tx.skip` | 1 tx bị loại — có field `reason` (xem bảng skip reason dưới) | `grep '"event":"tx.skip"' logs/bot.jsonl \| tail -20` |
| `sim.result` | Kết quả mô phỏng cuối cho 1 candidate (kể cả `Simulated`) | `grep '"event":"sim.result"' logs/bot.jsonl \| tail -10` |
| `tx.build` | Bot build xong calldata front-buy/back-sell (paper — không gửi) | `grep '"event":"tx.build"' logs/bot.jsonl \| tail -5` |
| `build.refused` | Có candidate `Simulated` nhưng bị từ chối build (vd chưa có signer thật) | `grep '"event":"build.refused"' logs/bot.jsonl \| tail -5` |
| `halt.triggered` / `halt.cleared` | Bot dừng/chạy lại do `state/halt.lock` | `grep -E '"event":"halt\.(triggered\|cleared)"' logs/bot.jsonl` |

Bảng lý do `tx.skip` (field `reason`):

| Reason | Ý nghĩa |
|---|---|
| `not_in_list` | Tx không khớp `pairs.txt`/`victims.txt` (mode đang tắt) |
| `below_min` | Số tiền victim giao dịch nhỏ hơn ngưỡng min |
| `decode_fail` | Không giải mã được calldata (router lạ, hàm chưa hỗ trợ, hoặc router/selector lệch nhau) |
| `not_wbnb_pair` / `not_quote_pair` | Cặp token không phải WBNB/USDT hợp lệ |
| `sell_direction` | Victim đang BÁN token (chưa có model sandwich cho chiều này) |
| `not_pancake_router` | `tx.to` không phải 1 trong 5 router Pancake đã pin |
| `venue_unpinned` | Venue chưa pin đủ để sim |
| `no_pool` | Factory trả `address(0)` — chắc chắn không có pool V2 |
| `rpc_error` | `eth_call` `getPair`/`getReserves` lỗi mạng/timeout — KHÁC `no_pool` (cụm `hotpath-fix-then-decoder-ur` A3, tách từ `no_pool` cũ) |
| `thin_liq` | Pool có reserve thấp hơn `min_reserve_wbnb`/`min_reserve_usdt` |
| `deadline` | Deadline của tx quá gần, không kịp front-run |
| `nonce_stale` / `nonce_future` | Nonce victim không khớp nonce kỳ vọng on-chain |
| `victim_would_revert` | Nếu front-run, tx victim gốc sẽ revert |
| `unprofitable` | Lợi nhuận ≤ 0 hoặc thấp hơn `min_profit_bnb`/`min_profit_usdt` |
| `honeypot_or_tax` | Tax đo được > `max_roundtrip_tax`, hoặc chưa đo (an toàn mặc định) |
| `hooks_unread` | Pool V4/Infinity không đọc được hook |
| `sim_error` | Lỗi kỹ thuật khi mô phỏng (RPC timeout, ...) |
| `gas_cap` | `gas_cost_wei` đo thật vượt trần cấu hình, hoặc `eth_gasPrice` vượt `gas_price_max_gwei` (cụm `real-economics-mode2`) |

## 8. Dừng / khởi động / halt

- **Dừng khẩn cấp** (dừng cả paper loop, không chỉ khoá live):
  ```bash
  touch state/halt.lock
  ```
  hoặc qua API:
  ```bash
  curl -s -X POST http://127.0.0.1:8787/api/control -H 'content-type: application/json' -d '{"action":"halt"}'
  ```
  Kỳ vọng: log có `halt.triggered`, `GET /api/status` trả `bot_state:"STOPPED"`.

- **Chạy lại sau halt**:
  ```bash
  rm -f state/halt.lock
  ```
  hoặc `POST /api/control {"action":"reset"}`. Kỳ vọng: log có `halt.cleared`.

- **`scripts/paper_run.sh`** tự halt sạch ở cuối phiên chạy — không cần làm
  gì thêm sau khi script in `DONE.`.

## 9. Deploy VPS

Xem chi tiết đầy đủ (checklist bảo mật + các bước) tại `docs/RUN.md` mục
"Vận hành trên VPS". Tóm tắt:

1. Tạo SSH key riêng cho VPS, tắt đăng nhập bằng password.
2. Bật `ufw` (chỉ mở port 22), cân nhắc `fail2ban`.
3. Cài `rustup`/`build-essential` trên VPS (script tự làm nếu thiếu).
4. `scripts/deploy_vps.sh --host <ip> --user root --identity <key> --build --run`
   để copy source + build + chạy bot nền (`systemd-run`, vẫn `dry_run=true`
   theo `config.toml` đã copy).
5. Tự điền `.env` **trên VPS** (không copy `.env` máy dev qua mạng).
6. Xem dashboard qua SSH tunnel, **không** mở port `8787` ra Internet:
   ```bash
   ssh -N -L 8787:127.0.0.1:8787 -p <port> <user>@<ip>
   ```
7. Ghi lại `git log -1 --format=%H` + `sha256sum target/release/bsc_sandwich`
   **trên VPS** — VPS và WSL phải cùng commit trước khi coi là "đã deploy
   đúng bản".

## 10. Sự cố thường gặp

| Triệu chứng | Nguyên nhân thường gặp | Cách xử lý |
|---|---|---|
| `/api/status` có `pending_source != "ws"` | `BSC_WS` rỗng/lỗi/không hỗ trợ `newPendingTransactions`, bot đã fallback sang `txpool_content` hoặc chỉ nhận inject | Đổi `BSC_WS` sang host khác (ưu tiên publicnode), kiểm log `rpc.pending_unavailable` để biết lý do cụ thể |
| `decode_fail` chiếm gần hết `seen` | Đa số tx mempool BSC không phải Pancake swap — **bình thường** | Chỉ đáng lo nếu `venue_v2`/`venue_v3` trong `/api/funnel` luôn bằng 0 dù chạy lâu |
| `nonce_stale` cao | Bình thường ở mức thấp-vừa (vài lần/phút) — victim tx bị tx khác chen nonce trước khi bot kịp xử lý | Chỉ đáng lo nếu gần bằng 100% số candidate |
| `sim_error` cao | RPC quá tải/timeout khi mô phỏng | Kiểm `.env` có nhiều URL failover chưa (mục 3), thử RPC khác |
| GoPlus trả `4029`/`REVIEW ... rate_limit_or_network` | Rate-limit burst của GoPlus (không phải lỗi mạng) | Đợi vài phút, chạy lại riêng các token đó (mục 4b) |
| `cargo build` lỗi thiếu `openssl`/`pkg-config` | Thiếu gói hệ thống | `sudo apt-get install -y build-essential pkg-config libssl-dev` rồi build lại |
| Ghi `state/halt.lock` nhưng bot không dừng | Kiểm tra đã ghi đúng thư mục `state/` trong repo đang chạy (không phải thư mục khác) | `ls -la state/halt.lock`, xem log có `halt.triggered` chưa — nếu bot không đọc được file do quyền, kiểm `chmod`/owner thư mục |
| `pairs.txt`: `candidate:0` dù đã điền `vetted` | Sai định dạng ngày (`vetted YYYY-MM-DD` phải đúng dạng số), hoặc vet nền vừa phát hiện `pair.vet_fail` | Kiểm `GET /api/pairs`, đối chiếu log `pair.unvetted`/`pair.vet_fail` |

## 11. An toàn

- **Không** dán nội dung `.env`, private key, hay IP/thông tin đăng nhập VPS
  vào chat, BAOCAO, hay bất kỳ file nào trong repo (kể cả file đã gitignore).
- URL RPC dán vào log/BAOCAO phải được redact (bot tự redact khi log —
  không tự ý dán URL đầy đủ có token/API key ra ngoài).
- Cờ live (`allow_live`/`bot_armed`/`dry_run=false`) hiện **CHƯA bật** và
  **CHƯA có tính năng ký/gửi tx thật** trong bot — mọi số liệu hiện tại đều
  là mô phỏng dry-run. Việc bật live đòi hỏi đủ 5 điều kiện (mục "Cổng live"
  trên dashboard) và một cụm công việc riêng (executor + relay bundle) chưa
  được xây dựng, xem `docs/TASKS.md`.

---

Xem thêm: `CLAUDE.md` (luật vận hành đầy đủ, không tự sửa trừ khi được lệnh),
`DEX_REGISTRY.md` (venue đã pin), `docs/STATE.md` (quyết định kỹ thuật),
`docs/TASKS.md` (việc còn lại), `docs/DOC_MAP.md` (bản đồ toàn bộ file),
`docs/RUN.md` (vận hành WSL + VPS chi tiết), `baocao/BAOCAO{NN}.md` (báo cáo
từng phiên).
