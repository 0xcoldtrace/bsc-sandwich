# BSC backrun-arb — Hướng dẫn vận hành

Repo vẫn tên `bsc-sandwich` (crate `bsc_sandwich`). Cờ đang bật:
`strategy = "backrun"` trong `config.toml`. Code sandwich còn trong repo,
tắt bằng cờ đó — không xoá đường cũ.

Luật phiên hiện tại: `CLAUDE.md`. `AGENTS.md` chỉ import `CLAUDE.md` (đúng
một dòng `@CLAUDE.md`). `AGENTS.legacy.md` là bản cũ, **không** điều hành.

Bot theo dõi swap trên BSC (chain `56`). Nhánh arb 2 venue đang dùng
`pairs_arb.txt`: **28** token `both_ok` đã vet `2026-09-17` — đó là **list
hẹp**, không phải toàn thị trường. Pancake V2: **một cặp = một pool**. Cùng
token có thể có `TOKEN/WBNB` và `TOKEN/USDT` (hai cặp), không phải “một
token hai pool V2”.

Tài liệu này viết cho người **không cần biết Rust**. Mỗi bước là lệnh
copy-dán được, kèm "kỳ vọng thấy gì". Không sửa `.env` / `config.toml` /
`pairs_arb.txt` chỉ bằng cách đọc README — luôn tự tay sửa file rồi đối chiếu
lại với phần tương ứng ở đây.

Chi tiết kỹ thuật / luật phiên: `CLAUDE.md`. Vận hành paper + VPS dài:
`docs/RUN.md`. Trạng thái cụm: `docs/STATE.md`, `docs/TASKS.md`.

## Mục lục

1. [Bot làm gì (hiện tại)](#1-bot-làm-gì-hiện-tại)
2. [Đã làm / chưa làm](#2-đã-làm--chưa-làm)
3. [Cài đặt trên WSL](#3-cài-đặt-trên-wsl)
4. [Cấu hình](#4-cấu-hình)
5. [Hai list token](#5-hai-list-token)
6. [Vet token (`pairs_arb.txt`)](#6-vet-token-pairs_arbtxt)
7. [List đa venue](#7-list-đa-venue)
8. [Chạy paper (dry-run)](#8-chạy-paper-dry-run)
9. [Dashboard](#9-dashboard)
10. [Đọc log `logs/bot.jsonl`](#10-đọc-log-logsbotjsonl)
11. [Dừng / khởi động / halt](#11-dừng--khởi-động--halt)
12. [Deploy VPS](#12-deploy-vps)
13. [Sự cố thường gặp](#13-sự-cố-thường-gặp)
14. [An toàn](#14-an-toàn)

---

## 1. Bot làm gì (hiện tại)

Trên BSC (chain `56`), bot theo dõi mempool, tìm swap trên list đang đọc.
Nhánh arb 2 venue (đang bật vì `strategy="backrun"`) **mô phỏng** một giao
dịch nguyên tử trên token có venue thứ hai trong `state/multi_venue.json`
(Pancake V2 + Pancake V3 và/hoặc Uniswap V3 — **không** phải hai pool V2
cùng một cặp):

1. Vay flash (ưu tiên Pancake Infinity Vault, phí 0).
2. Mua venue rẻ → bán venue đắt (có chân USDT↔WBNB nếu khác quote).
3. Trả nợ trong cùng tx, giữ phần chênh.

Không đứng trước victim, không cần `victim_ok`, không cần vốn xoay. Không lãi
→ tx sẽ revert (khi đã có contract). Hiện **chỉ mô phỏng** — chưa gửi bundle
thật.

Đường sandwich cũ giữ code, **tắt** bằng `strategy = "backrun"`. Kết luận
sandwich người-thật ≈ 0 lãi là mẫu đo cũ, không phải lệnh đổi dự án.

**Nguồn candidate đang bật:** `pairs_arb.txt` (List A, 28 token `both_ok` đã
vet `2026-09-17`). Đó là **list hẹp**, không phải bản đồ BSC. `pairs.txt`
**không dùng** khi `strategy="backrun"` (chỉ đọc lại nếu đổi
`strategy="sandwich"`). `victims.txt` tắt (`wallet_scan_enabled=false`).
Mode 3 tắt (`pair_scan_universal=false`).

**Mặc định an toàn:** `dry_run=true`, `allow_live=false`, `bot_armed=false`,
`live_mode="off"`. Bot không gửi tx. `live_mode="shadow"` **có thể ký thật**
bằng `PRIVATE_KEY` nhưng **không broadcast**.

Dashboard local (chỉ đọc) xem trạng thái, flash, funnel, skip. Không có nút
gửi giao dịch.

---

## 2. Đã làm / chưa làm

Sự thật trong repo (không phải kết quả paper mới của commit tài liệu này):

| Có | Chưa |
|---|---|
| Decoder router Pancake đã pin + Uniswap V3 SwapRouter02 (cổng backrun) | Contract `ArbExecutor` — **chưa viết** |
| `sim_arb` route V2↔V3 / V3↔V3 (PCS + Uniswap) | Gửi bundle thật / `sendRaw` |
| 4 nguồn flash + `/api/flash` | Live / `bot_armed` |
| Paper dry-run, dashboard, vet nền revm | Infinity CL làm venue arb (flash Vault thì đã có) |
| Shadow: ký thật, không gửi | |
| List A: 28 token `both_ok` trong `pairs_arb.txt` (list hẹp) | |

`BAOCAO51` No-Go B1 là **số mẫu cũ** (ngưỡng khi đó: ≥ 30 cơ hội/ngày và
p50 ≥ 5 USDT sau bribe, đo ≥ 6 h). Thiếu mẫu **không** có nghĩa hết cơ hội
hay đổi dự án. Chi tiết số đó: `baocao/BAOCAO51.md` ô 10. Cụm sau đó nằm
trong `docs/STATE.md` / `baocao/` — README không bịa số paper mới.

**Cảnh báo số liệu:** dòng `sim.arb` `simulated` có `borrow` lớn hơn
`arb_max_borrow_bnb` / `arb_max_borrow_usdt` **không dùng để kết luận lãi**.
Mẫu BAOCAO51 từng thấy borrow 40–46 BNB khi trần 20; code sau đó kẹp search
theo `arb_max_borrow_*` (xem `docs/STATE.md`). Không dùng hàng oversized
cũ làm lãi.

---

## 3. Cài đặt trên WSL

Chạy trong **WSL (Ubuntu)**, thư mục trên filesystem Linux (ví dụ
`~/bsc-sandwich`), **không** `/mnt/c/...`.

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev curl git jq
```

Kỳ vọng: không có dòng `E:` ở cuối.

Cài Rust (nếu chưa có):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Build:

```bash
cd ~/bsc-sandwich
cargo build --release
```

Kỳ vọng: `Finished release profile ...`. Lỗi `openssl` / `pkg-config` → mục 13.

Test (không cần mạng cho nhóm thường):

```bash
cargo test --lib --offline
cargo test --bin bsc_sandwich --offline
```

Kỳ vọng: `0 failed`. Số `ignored` là test gọi RPC thật (`real_rpc_*`) — không
chạy trong `cargo test` thường, không được coi là đã verify. Lần đo
BAOCAO51: 453 lib + 18 bin passed.

---

## 4. Cấu hình

### `.env` — bí mật + RPC

```bash
cp .env.example .env
nano .env
```

Tối thiểu:

```
BSC_HTTP=https://bsc-dataseed1.bnbchain.org
BSC_WS=wss://bsc-rpc.publicnode.com
```

- `BSC_WS` nên hỗ trợ `newPendingTransactions` (publicnode). Log cảnh báo nếu
  `pending_source != "ws"`.
- `PRIVATE_KEY` để **trống** ở paper `live_mode="off"`. Chỉ điền khi chạy
  **shadow** (ví trắng, không phải ví đang giữ tiền).
- `PRIVATE_TX_URL` / `BLOCKRAZOR_AUTH` để trống cho tới khi live bundle.
- `BSC_HTTP_SIM` (tuỳ chọn): RPC riêng cho revm (vet nền / crosscheck). Rỗng
  = dùng lại `BSC_HTTP`. Điền khi log `pair.vet_error` / `eth_getStorageAt`
  báo `-32000` / `not supported`.
- Không commit `.env` (đã gitignore).

Nhiều URL: `BSC_HTTP` phẩy, hoặc `BSC_HTTP_2`…`_16`, hoặc `BSC_HTTP_LIST`.
Cùng quy ước cho `BSC_WS` và `BSC_HTTP_BG` (task nền).

### `config.toml` — ngưỡng vận hành

Thiếu field bất kỳ = bot từ chối chạy. **Không tự thêm/xoá field** — chỉ sửa
giá trị. Sau khi lưu, bot đọc lại trong tối đa `config_reload_sec` giây
(mặc định 15), không cần rebuild.

Field đang quyết định hành vi sản phẩm (giá trị **ship** trong file repo):

| Field | Ship | Ý nghĩa |
|---|---|---|
| `strategy` | `"backrun"` | `"backrun"` = arb sau swap. `"sandwich"` = đường cũ, tắt. Chuỗi khác = fail load. |
| `pairs_arb_path` | `"pairs_arb.txt"` | List A. Khi `strategy="backrun"`, PairBook + vet nền đọc file này, **không đè** `pairs.txt`. |
| `pairs_path` | `"pairs.txt"` | List sandwich / mode 2 cũ. Không dùng khi đang backrun. |
| `arb_max_borrow_bnb` | `20` | Trần **vay flash** (BNB) mỗi route. Không phải vốn tự có. |
| `arb_max_borrow_usdt` | `12000` | Trần vay flash (USDT). |
| `pairs_min_swap_bnb` | `0.05` | Swap nhỏ hơn (quy BNB) → `below_min`. Chiến lược mô tả “swap lớn ≥ 0,5 BNB”; ngưỡng file đang `0.05` — tự chỉnh. |
| `min_profit_bnb` | `0.002` | Lãi tối thiểu (BNB) để `Simulated`. |
| `min_reserve_wbnb` | `20` | Pool mỏng hơn → `thin_liq`. List đa venue còn ngưỡng V2 ≥ 50 BNB khi **sinh list**. |
| `scan_quote_usdt` | `true` | Bật nhánh quote USDT. |
| `min_profit_usdt` / `max_front_usdt` / `min_reserve_usdt` | `3` / `3000` / `15000` | Ngưỡng USDT (không quy đổi từ BNB). |
| `sim_engine` | `"v2"` | Đường nóng: công thức đóng V2 + gas đo thật. `"evm"` chỉ để đối chiếu; vet nền/validator không đi qua field này. |
| `pairs_require_vetted` | `true` | Thiếu `vetted YYYY-MM-DD` → không sim. |
| `pairs_vet_interval_sec` | `600` | Chu kỳ vet nền revm. |
| `gas_price_max_gwei` | `10` | `eth_gasPrice` cao hơn → `gas_cap`. |
| `live_mode` | `"off"` | `"off"` / `"shadow"` / `"live"`. `"live"` không tự mở khoá gửi. |
| `allow_competitor_victims` | `false` | Khi `live_mode != "off"`, skip tx của cụm đối thủ đã nhận diện. Paper thuần (`off`) không chặn, vẫn gắn cờ để `/api/econ` đếm. |
| `wallet_scan_enabled` | `false` | Mode 1 tắt. |
| `pair_scan_enabled` | `true` | Mode 2 bật. |
| `pair_scan_universal` | `false` | Mode 3 tắt. |
| `dry_run` / `allow_live` / `bot_armed` | `true` / `false` / `false` | Cổng live. Không bật trừ khi có lệnh riêng. |
| `flash_source_interval_sec` | `300` | Task nền chụp chiều sâu 4 nguồn flash. |
| `multi_venue_path` | `"state/multi_venue.json"` | Bản đồ token → nhiều pool. Thiếu file → `arb_no_second_venue`, bot không crash. |
| `multivenue_min_v2_bnb` | `50` | Ngưỡng V2 khi chạy `discover_multivenue`. |
| `multivenue_min_v2_usdt` | `35000` | Cùng ý, quote USDT. |
| `multivenue_min_v3_impact_pct` | `2` | Impact V3 tối đa (%) khi bán `probe` 1 BNB. |
| `multivenue_probe_bnb` | `1` | Cỡ probe impact V3. |

Gas đường nóng `sim_engine="v2"`: chi phí = `eth_gasPrice` × gas unit (đo
revm lúc boot, fallback `gas_units_front`/`gas_units_back`).
`front_max_gas_bnb_wei` / `back_max_gas_bnb_wei` chỉ còn là **trần**. Route
arb dùng `gas_units_arb_infinity` / `gas_units_arb_v2flash` /
`gas_units_arb_v3` (ước lượng; V3 p50 hop đo 7 case ≈ 275k + overhead →
360000).

Kiểm tra config hợp lệ:

```bash
cargo test config::tests --offline
```

---

## 5. Hai list token

Đừng nhầm file:

| File | Khi nào bot đọc | Nội dung hiện tại |
|---|---|---|
| `pairs_arb.txt` | `strategy="backrun"` (ship) | List A: **28** token `both_ok`, vet `2026-09-17`. **List hẹp**, không phải toàn thị trường. |
| `pairs.txt` | `strategy="sandwich"` | List mode 2 cũ (sandwich). Backrun **không** sim file này. |
| `baocao/evidence/baocao50_listB_watch.txt` | Không | List B: có V3 nhưng mỏng — **theo dõi, không sim, không kết luận**. |

`both_ok` = V2 đủ ngưỡng **và** V3 cùng quote có impact ≤ 2 % khi bán 1 BNB.
Venue hợp lệ: Pancake V2, Pancake V3, Uniswap V3 BSC. **Không** THENA,
**không** Biswap.

Định dạng 1 dòng (cả hai file list):

```
0xToken # SYMBOL | vetted YYYY-MM-DD | tax b/s | owner renounced|active | note
```

- Trước `#`: `0xToken` (quote ngầm WBNB) hoặc `0xToken,0xQuote`. Cột 2 phải
  là WBNB hoặc USDT đã pin; địa chỉ khác → lỗi dòng, không sim.
- Sau `#`: bot chỉ đọc `vetted YYYY-MM-DD`. Thiếu ngày → **CHƯA VET**, không
  thành candidate (`pairs_require_vetted=true`).
- Dòng bắt đầu `#` = comment.

Ví dụ đúng (đang trong `pairs_arb.txt`):

```
0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82 # Cake | vetted 2026-09-17 | tax 0/0 | owner active | ...
```

Quote USDT tường minh:

```
0x80f1ff15b887cb19295d88c8c16f89d47f6d8888,0x55d398326f99059ff775485246999027b3197955 # COCO | vetted 2026-09-17 | ...
```

---

## 6. Vet token (`pairs_arb.txt`)

List arb chỉ nhận token **Chủ đã vet tay**. Tool chỉ lọc thô.

### a. Lọc thô GoPlus

```bash
scripts/vet_goplus.sh pairs_arb.txt
```

Cần `curl` + `jq`. Gọi tuần tự, retry khi rate-limit. In `PASS` / `REVIEW` /
`FAIL`. **Không tự sửa file.**

| Verdict | Việc cần làm |
|---|---|
| `PASS` | Vẫn đọc BscScan (bước b) rồi mới điền `vetted` |
| `REVIEW` | Cờ vàng (owner, mint, proxy, anti-whale…) — đọc kỹ, không tự loại |
| `FAIL` | Cờ đỏ (honeypot, blacklist, cooldown, pausable, hidden owner, tax > 5%) — không điền `vetted` |

Rate-limit (`loi_goi_API > 0`): đợi rồi chạy lại **chỉ các dòng đó**. Không
coi lỗi mạng là `PASS`.

### b. Checklist BscScan (bắt buộc)

Với mỗi token, mở `https://bscscan.com/address/0x...`:

- [ ] Contract verified
- [ ] Không phải proxy ẩn (nếu proxy phải đọc implementation; List A từng
      FAIL 6 token Binance-Peg EIP-1967 vì không `--allow-proxy`)
- [ ] Không tax/fee bất thường trong `_transfer`
- [ ] Không blacklist / cooldown / pause / maxTx quá thấp
- [ ] Không mint tuỳ ý / rebase
- [ ] Owner renounce, hoặc hiểu rõ quyền owner

Ô nào không tick → đừng điền `vetted`.

### c. Volume / thanh khoản

DexScreener `https://dexscreener.com/bsc/0x...`: pool Pancake V2 **và** V3
(PCS hoặc Uniswap) cùng quote. List A đòi V2 đủ sâu **và** V3 impact ≤ 2 %.
Chỉ “có V3 mỏng” → List B, không vào `pairs_arb.txt`.

### d. Điền ngày + reload

```
0xTokenAddress # SYMBOL | vetted 2026-09-17 | tax 0/0 | owner renounced | note
```

Bot đọc lại trong tối đa `pairs_reload_sec` (30 s), không restart.

### e. Vet nền (`pairs_vet_task`)

Bot đo lại tax/honeypot bằng revm mỗi `pairs_vet_interval_sec`, không tin mù
vet tay:

```bash
curl -s http://127.0.0.1:8787/api/pairs | jq
```

Kỳ vọng backrun: `count` = số dòng List A đã resolve (paper BAOCAO51:
`count=28`, `error_lines=0`). `candidate:true` = đã vet **và** vet nền chưa
loại. `pair.vet_fail` trong log → ẩn khỏi candidate tới lần PASS sau, không
tự xoá file.

`pending_count` cao + `last_error` lặp `-32000` / `not supported` → điền
`BSC_HTTP_SIM` (mục 4).

---

## 7. List đa venue

Tool sinh bản đồ token → nhiều pool (chỉ đọc chain, không ký):

```bash
cargo run --release --bin discover_multivenue -- \
  --hours 4 --pairs pairs.txt \
  --out state/multi_venue.json
```

- Nguồn volume: Swap log Pancake V2+V3 (cửa sổ `--hours`).
- Giữ **chỉ** `both_ok`. Output: `state/multi_venue.json`, TSV,
  `state/multi_venue_candidates.txt`.
- Ngưỡng lấy từ CLI hoặc `multivenue_*` trong config.
- Token PASS vet mới được copy vào `pairs_arb.txt`. List B không copy.

Thiếu `state/multi_venue.json` khi chạy bot → mọi tx arb `arb_no_second_venue`.

Đo lại / đối chiếu math (dev, cần RPC):

```bash
cargo run --release --bin arb_measure -- --jsonl logs/bot.jsonl --pairs-arb pairs_arb.txt
cargo run --release --bin arb_crosscheck
```

---

## 8. Chạy paper (dry-run)

```bash
scripts/paper_run.sh --minutes 30 --port 8799
```

Script: build lại, xoá `halt.lock` cũ, tạo **config tạm** (không đụng
`config.toml` thật — chỉ hạ ngưỡng kinh tế về 0 + đổi port), chạy N phút,
in kết quả, halt sạch. **Không** bật live/armed, **không** gửi tx.

Tuỳ chọn: `--live-mode shadow` (ký thật, không gửi — cần `PRIVATE_KEY` ví
trắng). `--allow-competitor-victims` chỉ để **đo** tx cụm đối thủ ở shadow.

Kỳ vọng đầu ra:

```
== may chay: WSL (repo: ...) ==
== binary sha256 = <64 hex> ==
== git HEAD = <40 hex> ==
== pairs_arb.txt (list A, strategy=backrun): total=28 vetted=28 ==
======== KET QUA SAU 30 PHUT ... ========
---- /api/skips ----
---- /api/funnel ----
---- /api/pairs ----
```

**Đọc đúng số backrun** (script vẫn in khối `sim.evm` của đường sandwich —
với `strategy="backrun"` khối đó thường rỗng, **không phải lỗi**):

```bash
# trong lúc bot chạy (đổi port cho khớp)
curl -s http://127.0.0.1:8799/api/pairs  | jq '{count,error_lines}'
curl -s http://127.0.0.1:8799/api/flash  | jq
curl -s http://127.0.0.1:8799/api/skips  | jq
curl -s http://127.0.0.1:8799/api/funnel | jq
grep '"event":"sim.arb"' logs/bot.jsonl | tail
```

- `/api/pairs` backrun: 28 dòng List A, `venue_unpinned=0` nếu V3 đã nối.
- `/api/flash`: chiều sâu Infinity / Aave / V2 flash / Balancer tại block đã
  chụp. Infinity Vault phí 0; Balancer trên BSC gần rỗng.
- `sim.arb`: kết quả route. `simulated` chỉ đáng tin khi `borrow` ≤ trần
  `arb_max_borrow_*`.
- `not_in_list` / `decode_fail` cao là bình thường (đa số mempool không phải
  swap Pancake/Uni trên List A).
- Mọi số paper phải ghi **máy (WSL/VPS)** + **sha256 binary** hoặc git HEAD
  (luật repo).

Chạy dài trên VPS từng bị OOM (~11 MB/phút, chết ~11 h trên máy 8 GB). Chia
phiên 4–6 h hoặc theo dõi RSS cho tới khi nợ mem được sửa. Xem `docs/RUN.md`.

Dừng sớm (an toàn):

```bash
touch state/halt.lock
# PID wrapper nằm ở state/paper_run.pid — kill đúng PID đó, đừng pkill -f paper_run.sh
```

---

## 9. Dashboard

Mở `http://127.0.0.1:8787` (hoặc port paper). Chỉ đọc. Khối chính: bot,
cổng live, **flash sources**, venue, pairs, hits, skip, funnel, tax, econ,
validator, shadow.

API:

```
GET  /api/health
GET  /api/status
GET  /api/venues
GET  /api/pairs
GET  /api/flash      # 4 nguồn flash tại block đã chụp
GET  /api/hits?limit=50
GET  /api/skips
GET  /api/funnel
GET  /api/econ       # bucket, top, net_pos_non_cluster, latency
GET  /api/validate
GET  /api/compete    # cụm đối thủ
GET  /api/mem        # RSS, kích thước container (chống OOM)
GET  /api/shadow
GET  /api/tax        POST /api/tax   (cần allow_tax_inject=true)
GET  /api/victims    # mode 1 đang tắt — file gần như trống
POST /api/control    body {"action":"halt"|"disarm"|"reset"}
```

Nút Halt/Disarm/Reset chỉ ghi `state/*.req` / `halt.lock`, không gọi signer.

---

## 10. Đọc log `logs/bot.jsonl`

| Event | Ý nghĩa | Xem nhanh |
|---|---|---|
| `bot.start` | Boot, config/venue ban đầu | `grep '"event":"bot.start"' logs/bot.jsonl \| tail -1` |
| `pair.reload` | Đọc lại list (backrun = `pairs_arb.txt`) | `grep '"event":"pair.reload"' logs/bot.jsonl \| tail -5` |
| `pair.unvetted` / `pair.vet_fail` | Chưa vet / vet nền loại | `grep '"event":"pair.vet' logs/bot.jsonl \| tail` |
| `tx.seen` | Pending mới (WS / txpool / inject) | `grep '"event":"tx.seen"' logs/bot.jsonl \| tail -5` |
| `tx.skip` | Bỏ qua — field `reason` | `grep '"event":"tx.skip"' logs/bot.jsonl \| tail -20` |
| `sim.arb` | Kết quả backrun-arb (đường đang bật) | `grep '"event":"sim.arb"' logs/bot.jsonl \| tail -10` |
| `sim.result` / `sim.evm` | Đường sandwich / EVM — thường rỗng khi backrun | — |
| `bundle.shadow` | Đã ký thật, **không gửi** (`live_mode=shadow`) | `grep '"event":"bundle.shadow"' logs/bot.jsonl \| tail` |
| `halt.triggered` / `halt.cleared` | `state/halt.lock` | `grep halt logs/bot.jsonl \| tail` |

Lý do `tx.skip`:

| Reason | Ý nghĩa |
|---|---|
| `not_in_list` | Token không nằm trong list đang đọc (`pairs_arb.txt` khi backrun) |
| `below_min` | Swap nhỏ hơn `pairs_min_swap_bnb` |
| `decode_fail` | Không giải mã được (NFT UR, hàm lạ, router/selector lệch). ~81% `execute()` fail cũ là Seaport/NFT — đúng, không phải swap sót |
| `not_wbnb_pair` / `not_quote_pair` | Không phải WBNB/USDT hợp lệ |
| `sell_direction` | Victim bán token — sandwich không làm chiều này; backrun vẫn có thể cân venue khác (V2 và V3, hoặc cặp quote khác) sau swap lớn (tuỳ path decode). Không phải hai pool V2 cùng cặp. |
| `not_pancake_router` | `tx.to` không phải router được cổng nhận. Sandwich: 5 router Pancake. Backrun: 5 Pancake **+** Uniswap V3 SwapRouter02 |
| `venue_unpinned` | Venue chưa pin đủ để sim |
| `no_pool` | Factory trả `address(0)` |
| `rpc_error` | `eth_call` lỗi mạng — khác `no_pool` |
| `thin_liq` | Reserve dưới ngưỡng |
| `deadline` / `nonce_stale` / `nonce_future` | Không kịp / nonce lệch |
| `victim_would_revert` | Đường sandwich: victim sẽ revert nếu bị kẹp |
| `unprofitable` | Lãi ≤ 0 hoặc dưới `min_profit_*` |
| `honeypot_or_tax` | Tax / honeypot (hoặc `vet_fail`) |
| `hooks_unread` | Pool Infinity không đọc được hook — skip **pool**, không tắt bot |
| `sim_error` | Lỗi mô phỏng (RPC…) |
| `gas_cap` | Gas thật vượt trần, hoặc `eth_gasPrice` > `gas_price_max_gwei` |
| `sanity_reject` | Sim vượt trần vô lý (front > 10% reserve, profit > 2% reserve, victim_in > 100% reserve) — **chưa bắt hết** borrow arb oversized |
| `competitor_victim` | Ví cụm đối thủ, chỉ khi `live_mode != "off"` và `allow_competitor_victims=false` |
| `arb_no_second_venue` | Token không có ≥ 2 venue đủ sâu trong `multi_venue.json` |
| `arb_no_flash_source` | Không nguồn flash nào đủ sâu tại block |

---

## 11. Dừng / khởi động / halt

Dừng khẩn cấp (cả paper loop):

```bash
touch state/halt.lock
```

hoặc:

```bash
curl -s -X POST http://127.0.0.1:8787/api/control \
  -H 'content-type: application/json' \
  -d '{"action":"halt"}'
```

Kỳ vọng: log `halt.triggered`, `/api/status` → `bot_state:"STOPPED"`.

Chạy lại:

```bash
rm -f state/halt.lock
```

hoặc `POST /api/control {"action":"reset"}`. Kỳ vọng: `halt.cleared`.

`scripts/paper_run.sh` tự halt cuối phiên. Nếu phải giết process: dùng PID
trong `state/paper_run.pid`, **không** `pkill -f paper_run.sh` (có thể giết
nhầm shell đang chạy lệnh).

---

## 12. Deploy VPS

Chi tiết: `docs/RUN.md` mục “Vận hành trên VPS”. Tóm tắt:

1. SSH key **riêng** cho VPS (không dùng khóa GitHub, không commit `key/`).
2. `ufw` chỉ mở 22. Không mở `8787` ra Internet.
3. `scripts/deploy_vps.sh --host <ip> --user root --identity <key> --build --run`
4. Điền `.env` **trên VPS** — không copy `.env` máy dev.
5. `config.toml` trên VPS là file **riêng**. Binary mới có field mới (ví dụ
   `pairs_arb_path`, `gas_units_arb_v3`, `multivenue_*`) phải cập nhật
   **cùng lúc**, thiếu = fail load.
6. Dashboard qua tunnel:
   ```bash
   ssh -N -L 8787:127.0.0.1:8787 -p <port> <user>@<ip>
   ```
7. WSL và VPS phải **cùng git commit**. Ghi `git log -1 --format=%H` +
   `sha256sum target/release/bsc_sandwich` trên VPS.

VPS chỉ nhận commit đã paper trên WSL. Không ghi IP VPS vào README / BAOCAO.

---

## 13. Sự cố thường gặp

| Triệu chứng | Nguyên nhân thường gặp | Cách xử lý |
|---|---|---|
| `/api/status` `pending_source != "ws"` | `BSC_WS` rỗng / không hỗ trợ pending | Đổi WSS (ưu tiên publicnode), xem `rpc.pending_unavailable` |
| `decode_fail` chiếm gần hết `seen` | Đa số mempool không phải swap List A — bình thường | Đáng lo nếu `sim.arb` = 0 suốt lâu **và** `/api/pairs` candidate > 0 |
| `arb_no_second_venue` mọi tx | Thiếu / cũ `state/multi_venue.json` | Chạy `discover_multivenue`, kiểm `multi_venue_path` |
| `venue_unpinned` trên V3 | Binary/config cũ chưa nối V3 arb | Cần bản có cụm B5; paper BAOCAO51 đã `venue_unpinned=0` |
| `sim.arb` simulated nhưng borrow > trần | Mẫu BAOCAO51: 40–46 BNB khi trần 20; code sau kẹp `arb_max_borrow_*` | **Không** dùng dòng oversized cho Go/No-Go |
| `sim.evm` rỗng khi paper | Đúng với `strategy=backrun` + `sim_engine=v2` | Đọc `sim.arb` + `/api/flash` |
| `/api/pairs` `count=0` dù đã vet | Đang đọc nhầm file, hoặc sai `vetted YYYY-MM-DD` | Backrun phải là `pairs_arb.txt`. Xem `pair.unvetted` |
| `sim_error` / vet `missing_trie_node` | RPC public không giữ state | Điền `BSC_HTTP_SIM` node đủ state |
| `cargo build` thiếu openssl | Thiếu gói hệ thống | `sudo apt-get install -y build-essential pkg-config libssl-dev` |
| Bot chết sau nhiều giờ, không có `DONE` | OOM (đã đo trên VPS 8 GB) | RSS qua `/api/mem`; chia phiên ngắn; xem `docs/RUN.md` |
| Halt không dừng | Sai thư mục `state/` hoặc giết nhầm PID | `ls state/halt.lock`; PID = `state/paper_run.pid` |

---

## 14. An toàn

- Không dán `.env`, private key, `BLOCKRAZOR_AUTH`, IP VPS vào chat / BAOCAO
  / file trong git.
- URL RPC có token phải redact. Bot tự redact khi log.
- Cổng live cần **đủ** `allow_live && !dry_run && bot_armed` && không
  `halt.lock` && `chain_id==56` && `live_*` && venue đã pin. Thiếu một cái
  = không gửi. Dashboard khối “Cổng live” tick từng điều kiện.
- Mọi tx MEV (khi có) chỉ gửi **bundle** qua builder đã pin (48 Club
  Puissant, BlockRazor). Cấm `sendRaw` lẻ qua RPC thường. Bribe = chuyển BNB
  tới EOA builder **trong tx**, chỉ khi thành công.
- `live_mode="shadow"`: ký thật, không gửi. Dùng ví trắng.
- `live_mode="live"` trong config **không** tự mở khoá — vẫn cần cụm
  executor + lệnh Chủ. Contract arb **chưa có** (No-Go B1).
- Flash chỉ từ nguồn đã pin trong `DEX_REGISTRY.md`, trả trong cùng tx.

Nguồn flash (đọc on-chain mỗi chu kỳ, không giả định số dư):

1. Pancake Infinity Vault — phí 0, trần = `balanceOf(vault)` (không phải
   `reservesOfApp`).
2. Aave V3 Pool — 5 bps.
3. Pancake V2 `pancakeCall` — ~25 bps.
4. Balancer V2 Vault — phí 0 nhưng trên BSC gần rỗng + wind-down; không chờ.

---

Xem thêm: `CLAUDE.md` (luật phiên), `AGENTS.md` (chỉ `@CLAUDE.md`),
`AGENTS.legacy.md` (archive, không điều hành), `DEX_REGISTRY.md` (venue +
flash đã pin), `docs/STATE.md`, `docs/TASKS.md`, `docs/DOC_MAP.md`,
`docs/RUN.md`, `docs/CONTRACT_DESIGN.md` (thiết kế, chưa code), `baocao/`
(báo cáo theo cụm; không lấy mẫu hẹp làm “hết cơ hội”).
