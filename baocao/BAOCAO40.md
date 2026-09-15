# BAOCAO40 — cụm `econ-truth-latency-vps`

## 1. LÁT
`econ-truth-latency-vps` — (0) PairBook/RPC resolve cache+backoff, (1) fix
2 lỗi đo econ (**+ tìm ra và sửa BUG GỐC nghiêm trọng** làm lệch
`funnel.simulated` vs `sim.result`), (2) kiểm cạnh tranh trên 14 case thật +
task nền `compete.check`, (3) cache reserve theo Sync-event + độ trễ
`decision_vs_mined_block`, (4) nợ nhỏ (nonce gate v2 từ cache, rotate log),
(5) deploy VPS + chạy 30 phút cả 2 máy.

## 2. LỆNH NHẬN
Khối lệnh Grok `econ-truth-latency-vps`. Máy: WSL + VPS. HEAD bắt đầu:
`583d09e` (pairs.txt 126 token, sau BAOCAO39). Không subagent ghi file
(luật #4) — toàn bộ code/docs phiên này do phiên chính viết trực tiếp.

## 3. FILE ĐỔI

Commit `1f0884a` (Phần A — mục 0, 1 một phần, 4 dedup+nonce):
- `src/pairbook.rs` — `LineState`/`PendingRetry`/`retry_backoff` (0.a): dòng
  đã resolve KHÔNG resolve lại; lỗi RPC/"no pool" tạm thời → `Pending`
  (backoff 5s/15s/60s), không còn tính `error_lines`. `pair.resolve_fail`
  đầy đủ token/quote/error/url_label (0.b). `PairEntry.symbol` (mục 1).
  `pending_count()`/`pending_entries()`. +7 test.
- `src/transport.rs` — `is_unsupported_method_error`, `RpcPool::
  mark_current_unsupported`/`current_url_label` (0.c). `SeenHashSet` dedup
  xuyên nguồn tx (mục 1, đóng khoảng hở WS→txpool fallback). +6 test.
- `src/main.rs` — `sim_http_pool`/`BSC_HTTP_SIM`/`sim_provider` (0.d),
  `pairs_vet_task` sleep 300ms (0.e), wire dedup vào `subscribe_pending_txs`/
  `poll_txpool_pending`, gate `nonce_stale`/`nonce_future` thuần từ
  `NonceCache` ở nhánh V2 (mục 4), `RpcPairResolver` thêm `url_label`.
- `src/web.rs` — `AppStateInner.sim_provider`/`seen_hashes`. `/api/pairs`
  thêm `symbol`/`pending`/`pending_count`.
- `src/pipeline.rs` — `TxLogMeta.amount_in_bnb_equiv`, `convert_usdt_to_bnb_wei`.
- `.env.example`, `README.md` — tài liệu `BSC_HTTP_SIM`, `/api/pairs pending`.

Commit `9dd725a` (Phần B — mục 1 FIX BUG GỐC, mục 2, mục 3):
- `src/pipeline.rs` — **FIX BUG NGHIÊM TRỌNG**: `profit_wei`/
  `profit_gross_wei`/`profit_net_wei` (i128) đưa thẳng vào `serde_json::json!`
  làm PANIC nội bộ khi giá trị vượt `i64::MAX` (rất phổ biến với profit
  USDT) — task `handle_paper_tx` chết âm thầm SAU KHI đã tăng
  `funnel.simulated` nhưng TRƯỚC KHI ghi `sim.result` — đây là NGUYÊN NHÂN
  GỐC của lệch số liệu BAOCAO39 (27 vs 14). Sửa: 3 field chuyển `String`. +2
  test (`log_outcome_v2_simulated_with_profit_over_i64_max_does_not_panic`,
  `compute_econ_profit_over_i64_max_as_string_still_buckets_correctly`).
- `src/web.rs` — `compute_econ_from_rows`: `top_pools` (thay `top_tokens`),
  bucket cả USDT qua `amount_in_bnb_equiv`, `parse_profit_wei` (đọc String).
  `CompeteStats` + `GET /api/compete`. +3 test.
- `src/pool.rs` — `sync_topic0`/`decode_sync_log_reserves`/
  `order_reserves_by_quote` (Sync event). +4 test.
- `src/main.rs` — `std::panic::set_hook` (log `debug.panic`, giữ vĩnh
  viễn), `subscribe_sync_events` (WS, cập nhật `ReserveCache` trực tiếp từ
  event), `spawn_post_simulated_tracker` (gộp `decision_vs_mined_block` +
  `compete.check`, thay `spawn_decision_latency_tracker` cũ trong phiên).

Commit `5284bd3` (deploy_vps.sh fix):
- `scripts/deploy_vps.sh` — GIỮ LẠI `.git` khi copy (trước loại trừ, khiến
  `git rev-parse HEAD` trên VPS không chạy được).

Commit `5b17a83` (docs, KHÔNG đổi binary):
- `docs/STATE.md`, `docs/TASKS.md`, `docs/RUN.md`, `CLAUDE.md` (dòng
  `GET /api/compete`).

**KHÔNG đụng**: `.env` (2 máy), dòng token `pairs.txt`, cờ live, `sendRaw`,
decoder logic (`decoder.rs` không sửa dòng nào).

## 4. LỆNH CHẠY

```bash
cargo build --release
cargo test --release
sha256sum target/release/bsc_sandwich
git log -1 --format="%H %ci"
git status --short

scripts/paper_run.sh --minutes 6 --port 189xx   # nhiều lần, xác nhận fix
scripts/paper_run.sh --minutes 30 --port 18910  # WSL, DoD cuối
# Tren VPS (qua SSH, cung khung gio voi WSL):
scripts/paper_run.sh --minutes 30 --port 18910
```

## 5. OUTPUT THẬT

**Máy: WSL** (`/home/dmin/bsc-sandwich`), **Máy: VPS** (IP KHÔNG ghi vào
BAOCAO theo luật CLAUDE.md — Chủ đã dán qua chat, dùng để SSH/deploy).

### `cargo test --release` (HEAD cuối `5284bd3`, docs commit `5b17a83` sau đó không đổi binary)

```
test result: ok. 347 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```
347 lib + 15 main = **362 passed** (tăng từ 344 đầu phiên = 583d09e, +18 test
mới — không đếm trùng vài test viết-rồi-sửa trong phiên).

Binary sha256 WSL (HEAD `5284bd3`, `.env` thật):
`150879ae76ab8ca6ff6fb7dadd2c399c2493582285edb9cd9d7b29b052d6d154`
Binary sha256 VPS (cùng HEAD `5284bd3`, build trên VPS):
`4af44b29ef1adf729f416d624d18147030bf8d52f0c7f149bfaa0a0143b0feab`
(khác WSL — BÌNH THƯỜNG, build trên 2 hệ khác nhau dù cùng source; CLAUDE.md
chỉ yêu cầu CÙNG COMMIT, không yêu cầu cùng byte binary).

### Mục 0 — PairBook/RPC, DoD (WSL, paper 6 phút, HEAD `1f0884a`)

```
== may chay: WSL == binary sha256 = 5cc827bae7b3ff037cacbc0f3de634204ccc120cb8ed40bab4289df861a059f2 ==
== git HEAD = 1f0884a356bb98644c2733db6b65588443d76d32 ==
```
`/api/pairs`: `{"count":126,"error_lines":0,"pending_count":0,...}` — ổn
định qua nhiều lần `pair.reload` trong 6 phút (`pairs_reload_sec` mặc định
30s → ~12 lần reload). `pair.resolve_fail` = 0 lần trong log (không có lỗi
RPC thật để kích hoạt trong cửa sổ 6 phút này — cơ chế backoff verify riêng
bằng 2 test unit dùng `CountingResolver` đếm số lần gọi RPC thật:
`reload_does_not_reresolve_already_resolved_lines` (1 lần gọi ban đầu, 0
lần gọi thêm ở 2 reload sau dù resolver đổi sang lỗi 100%),
`pending_line_respects_backoff_before_retrying` (đúng 1 lần retry sau 6s,
không retry ở mốc 2s < backoff 5s)). **DoD 30 phút WSL sau (mục 5.f dưới)
xác nhận `pair.resolve_fail` THẬT xảy ra 9 lần, `/api/pairs` vẫn giữ
`126/0` xuyên suốt — không rớt pool nào.**

### Mục 1 — FIX BUG GỐC funnel.simulated vs sim.result (bằng chứng thực nghiệm)

**Tái hiện TRƯỚC fix** (WSL, `debug.panic`/`debug.simulated_marker` tạm
thời — đã xoá sau khi xác định xong, không còn trong code cuối):

Lần 1 (HEAD `1f0884a`, port 18902, 5 phút):
```
debug.simulated_marker: 4 dong (4 hash rieng biet)
sim.result: 2 dong
funnel.minute sum simulated: 4
debug.panic: 4 dong, TAT CA cung 1 vi tri:
  "panicked at src/pipeline.rs:1308:17:
   called `Result::unwrap()` on an `Err` value: Error(\"number out of range\", line: 0, column: 0)"
```
(2/4 debug.panic khớp đúng 2 hash KHÔNG có sim.result tương ứng — 2 hash
còn lại ghi thành công.)

**Nguyên nhân**: `pipeline::log_outcome_v2` đưa `q.profit_wei`/
`profit_gross_wei`/`q.profit_wei` (cả 3 đều `i128`) thẳng vào
`serde_json::json!{...}` — macro này gọi `serde_json::to_value(...).unwrap()`
nội bộ; `serde_json` (không bật `arbitrary_precision`, xem `Cargo.toml`)
trả lỗi "number out of range" khi giá trị VƯỢT `i64::MAX`
(`9_223_372_036_854_775_807`, chỉ > 9.22 đơn vị token wei — rất phổ biến
với profit quote USDT). `.unwrap()` panic → task `tokio::spawn(handle_paper_tx)`
chết ÂM THẦM (JoinHandle bị bỏ qua) — nhưng `record_funnel_terminal` (tăng
bộ đếm `simulated`) đã chạy TRƯỚC dòng gây panic 1 lệnh, nên bộ đếm ĐÃ TĂNG
mà không có `sim.result` tương ứng bao giờ.

**Sửa**: 3 field profit chuyển `String` (`.to_string()`, cùng khuôn
`front_in_wei`/`back_out_wei` cũ). `web::compute_econ_from_rows` đọc lại
qua `parse_profit_wei` (String, fallback `as_i64` cho log cũ). Test hồi quy
dựng `profit_wei=17_579_175_023_944_993_805` (giá trị THẬT từng thấy trong
`logs/bot.jsonl`, xem BAOCAO39) — PASS sau fix.

**Verify SAU fix, 3 lần độc lập**:

| Lần | Máy | HEAD | Phút | funnel.simulated | sim.result | debug.panic |
|---|---|---|---|---|---|---|
| 1 | WSL | `9dd725a` | 6 | 7 | 7 | 0 |
| 2 | WSL | `9dd725a` | 6 | 5 | 5 | 0 |
| 3 (DoD 30p) | WSL | `5284bd3` | 30 | **29** | **29** | 0 |
| 4 (DoD 30p) | VPS | `5284bd3` | 30 | **27** | **27** | 0 |

**2 số bằng nhau tuyệt đối ở cả 4 lần chạy, kể cả 2 lần DoD 30 phút chính
thức** — đạt đúng yêu cầu lệnh.

### Mục 1 — `/api/econ` top_pools + bucket USDT (WSL, paper 5 phút, HEAD `1f0884a`)

```
candidate: 2567
tong buckets_bnb[].count: 2567   (KHOP CHINH XAC — gom ca 241 candidate usdt)
by_quote: {"usdt": 241, "wbnb": 2326}
```
Trước cụm này, USDT hoàn toàn vắng mặt khỏi `buckets_bnb` (0/241). `top_pools`
mẫu: `{"pair":"0x3dc2878f...","count":60,"symbol":"priceless",...}` — có
`symbol` cho pool trong `pairs.txt`, `null` cho pool ngoài danh sách (đúng
thiết kế, không bịa).

### Mục 2 — Kiểm cạnh tranh 14 case thật

Tìm lại 14 hash từ BAOCAO39 trong `logs/bot.jsonl` (grep `sim.result` +
lọc `ts` khung `2026-09-15T13:34`–`14:24`). Tra `eth_getTransactionReceipt`/
`eth_getBlockByNumber` qua RPC công khai `bsc-dataseed1.bnbchain.org`
(không cần archive — dữ liệu block/receipt full node giữ vĩnh viễn).

**Bảng 14 dòng** (vị trí = `tx_index` trong block; "sandwiched?" = có tìm
thấy CẶP before+after CÙNG 1 địa chỉ chạm cùng pool không — tiêu chuẩn
sandwich thật):

| # | hash (rút gọn) | block | vị trí | sandwiched? | competitor liền kề (before/after) | gas competitor (gwei) | gas victim (gwei) |
|---|---|---|---|---|---|---|---|
| 1 | 0x082b5ae5 | 122038525 | 5 | KHÔNG | before+after khác nhau, cả 2 chạm cùng pool | 1.05 / 0.0 | 0.12 |
| 2 | 0x78768177 | 122038719 | 19 | KHÔNG | chỉ after chạm pool | - / 0.0 | 0.12 |
| 3 | 0xe0935f2d | 122039160 | 14 | KHÔNG | chỉ before chạm pool | 0.05 / - | 0.05 |
| 4 | 0x9af92a95 | 122039396 | 76 | KHÔNG | chỉ before chạm pool | 0.05 / - | 0.05 |
| 5 | 0x4e6f150f | 122039411 | 14 | KHÔNG | chỉ before chạm pool | 0.05 / - | 0.05 |
| 6 | 0x0d47b2b1 | 122041378 | 23 | KHÔNG | chỉ after chạm pool | - / 0.0 | 0.12 |
| 7 | 0x1edd4e24 | 122041568 | 20 | KHÔNG | chỉ before chạm pool | 0.05 / - | 0.05 |
| 8 | 0xecc4e37a | 122042711 | 1 | KHÔNG | chỉ after chạm pool | - / 0.0 | 0.12 |
| 9 | 0x8cfce705 | 122042767 | 16 | KHÔNG | chỉ before chạm pool | 0.05 / - | 0.05 |
| 10 | 0x69dfb604 | 122043246 | 11 | KHÔNG | cả 2 chạm pool, KHÁC địa chỉ | 0.05 / 0.05 | 0.05 |
| 11 | 0xe4ba765c | 122043481 | 5 | KHÔNG | cả 2 chạm pool, KHÁC địa chỉ | 0.05 / 0.05 | 0.05 |
| 12 | 0xe6b07957 | 122043962 | 16 | KHÔNG | cả 2 chạm pool, KHÁC địa chỉ | 0.05 / 0.0521 | 0.05 |
| 13 | 0x92f32957 | 122044852 | 2 | KHÔNG | before = CHÍNH victim (tx khác cùng ví) | 1.0 / - | 1.0 |
| 14 | 0x75ea650a | 122045164 | 39 | KHÔNG | cả 2 chạm pool, KHÁC địa chỉ | 0.05 / 0.105 | 0.05 |

**Kết luận (quan trọng nhất)**: **0/14 case có bằng chứng bị sandwich** ở
mức kiểm tra 1 vị trí liền kề mỗi bên. Không case nào có cặp
before-từ == after-từ (dấu hiệu 1 bot làm cả 2 chân). Địa chỉ
`0xb406021e07b31e1f7850fcccd7076094f18d07ef` xuất hiện LẶP LẠI 6/14 lần ở
vị trí "before", LUÔN ở gas price BẰNG (không cao hơn) victim — đặc điểm
của 1 trader/bot hoạt động thường xuyên trên CÙNG pool BORT (WBNB), KHÔNG
PHẢI front-run (front-run thật cần gas CAO HƠN để đảm bảo thứ tự). **Giới
hạn**: chỉ kiểm 1 vị trí liền kề mỗi bên, KHÔNG loại trừ bot dùng bundle
riêng/relay private không lộ trong mempool công khai/block thường.

**`compete.check` (task nền, đã nối dây, verify 30 phút WSL)**: 29/29
candidate `Simulated` có `compete.result`; 16/29 tìm thấy tx liền kề chạm
cùng pool, nhưng **0/16 có gas CAO HƠN victim** (tất cả gas BẰNG victim) —
khớp hoàn toàn kết luận phân tích tay 14 case ở trên, mở rộng ra 29 case
mới quan sát được. `GET /api/compete` trả `checked=29 possible_competitor=16
possible_competitor_pct=55.17`.

### Mục 3 — Sync-event ReserveCache + decision_vs_mined_block

Verify 30 phút WSL: `sync.subscribed` = 3 lần (kết nối + reconnect qua
danh sách URL). `decision_vs_mined_block`: 29/29 candidate `Simulated` đo
được, `min=-1 max=0 avg=-0.97` — ĐA SỐ `-1` (bot quyết định 1 block TRƯỚC
khi victim thật sự lên block — kịp thời gian về mặt lý thuyết cho front-run,
dù CLAUDE.md F1 audit đã kết luận riêng chưa tính cạnh tranh/latency thực
thi). VPS: 27/27 đo được, `min=-2 max=0 avg=-0.74`.

`p95 seen_to_decision_ms` (mục tiêu CLAUDE.md <500ms, BAOCAO39 trước đó
1779ms/A-DoD 3692ms — CHƯA đạt): **WSL 30 phút = 321.25ms (ĐẠT, <500ms)**,
**VPS 30 phút = 43.84ms (ĐẠT, tốt hơn WSL ~7.3 lần)**. Không tách được
phần đóng góp CỤ THỂ của riêng Sync-event pre-warming (không có A/B cùng
điều kiện thị trường) — ghi rõ CÒN NỢ, chỉ có thể báo cáo kết quả TỔNG (đã
đạt mục tiêu tuyệt đối, không khẳng định nguyên nhân duy nhất).

### Mục 4 — Nợ nhỏ

Nonce gate đấu dây đúng (dùng `transport::compare_nonce` qua `NonceCache`,
không thêm `eth_call`) nhưng **NO-OP về số liệu thật** trên cả 2 lần DoD 30
phút (`nonce_stale=0 nonce_future=0` cả 2 máy, xem `/api/skips` mục 5 dưới)
— vì `NonceCache` chỉ được điền bởi `run_evm_decision` (`sim_engine="evm"`,
không chạy trên đường nóng `sim_engine="v2"` ship). Ghi thật, không bịa
hiệu quả — xem `docs/TASKS.md` mục nợ.

`scripts/paper_run.sh` rotate `logs/bot.jsonl` khi ≥200MB — chưa kiểm thật
(file hiện `112MB` sau nhiều lần chạy phiên này, chưa chạm ngưỡng 200MB
trong phiên) — cơ chế đã code + review logic, KHÔNG có bằng chứng runtime
kích hoạt thật (ghi rõ CÒN NỢ verify runtime, không bịa "đã chạy được").

### Mục 5 — Deploy VPS

VPS: Ubuntu 22.04.5 LTS, region NJ US (theo `vps.json`). Cài đặt:
`apt-get install -y build-essential pkg-config libssl-dev git curl jq
fail2ban ufw` (thành công), `ufw allow 22/tcp` TRƯỚC `ufw --force enable`
(xác nhận SSH vẫn sống sau khi enable), `fail2ban` `systemctl enable --now`
(active), `rustup` cài qua `sh.rustup.rs -y --default-toolchain stable`
→ `cargo 1.98.1`/`rustc 1.98.1`.

Deploy bằng `scripts/deploy_vps.sh --host <VPS> --user root --identity
key/bsc_vps_ed25519 --build` (đã sửa script giữ lại `.git`, xem mục 3).
Phát hiện thật lúc verify: VPS báo `fatal: detected dubious ownership` khi
chạy `git` (owner file khác owner chạy lệnh do tar+ssh bằng root) — sửa 1
lần bằng `git config --global --add safe.directory /root/bsc-sandwich`
(đã ghi vào `docs/RUN.md`).

**Xác nhận cùng commit**: WSL `git rev-parse HEAD` = VPS `git rev-parse
HEAD` = `5284bd376fb78ea0b450249fb2d09d4adfd7f812` (KHỚP). VPS
`git status --short` sau deploy: chỉ `?? .claude/` (thư mục settings local,
không phải mã nguồn — vô hại).

`.env` KHÔNG copy bởi Claude Code (đúng luật). In lệnh scp cho Chủ:
```
scp -i key/bsc_vps_ed25519 .env root@<VPS>:/root/bsc-sandwich/.env
```
Chủ xác nhận đã chạy — verify bằng `grep` tên biến (không in giá trị):
`.env` trên VPS có `BSC_HTTP=`/`BSC_WS=` (2 dòng khớp).

Chạy `paper_run.sh --minutes 30 --port 18910` TRÊN CẢ 2 MÁY, CÙNG KHUNG
GIỜ (WSL 23:47–00:17 giờ VN = 16:47–17:17 UTC; VPS 16:49–17:19 UTC — trùng
lấp ~28/30 phút).

### Bảng so sánh 30 phút WSL vs VPS (DoD, HEAD `5284bd3` cả 2 máy)

| Chỉ số | WSL | VPS |
|---|---|---|
| binary sha256 | `150879ae...` | `4af44b29...` |
| candidate (`/api/econ`) | 19213 | 18181 |
| `funnel.simulated` = `sim.result` | 29 = 29 | 27 = 27 |
| `debug.panic` | 0 | 0 |
| `net_pos_total` | 29 | 27 |
| `best_net_bnb` | 0.241646 | 0.241648 |
| p50 `seen_to_decision_ms` | 0.0059ms | 0.0086ms |
| **p95 `seen_to_decision_ms`** | **321.25ms** | **43.84ms** |
| `decision_vs_mined_block` avg | -0.97 (n=29) | -0.74 (n=27) |
| `pair.resolve_fail` (lỗi RPC thật) | 9 | 28 |
| `/api/pairs` count/error/pending | 126/0/0 | 126/0/0 |
| `rpc_error` (`/api/skips`) | 12 | 5 |
| `deadline` (`/api/skips`, F-14 lần đầu có số thật) | 1 | 1 |
| `victim_would_revert` | 6 | 3 |
| `compete.result` ghi được | 29/29 | (chưa trích riêng, xem log VPS) |

**Kết luận latency VPS vs WSL**: VPS nhanh hơn WSL **~7.3 lần** ở p95
(43.84ms so 321.25ms) — hợp lý vì VPS đặt tại datacenter US gần cụm RPC
BSC hơn kết nối nhà của Chủ qua WSL. `pair.resolve_fail` cao hơn hẳn trên
VPS (28 vs 9) dù `/api/pairs` vẫn giữ `126/0` xuyên suốt (đúng thiết kế
0.a — lỗi tạm thời không làm rớt pool) — có thể do RPC pool khác nhau
lúc đó/tải mạng VPS lúc chạy, KHÔNG kết luận thêm.

## 6. CHAIN — `0x38`
`connect_and_verify` xác nhận `chain_id=56` mọi lần kết nối (cả `http_pool`,
`sim_http_pool`, `subscribe_sync_events`, cả 2 máy). `eth_getTransactionReceipt`/
`eth_getBlockByNumber` (mục 2, 14 case thật) qua `bsc-dataseed1.bnbchain.org`
— đã verify `eth_chainId`/dữ liệu block thật trước khi dùng (block
`122038525`–`122045164`, khớp dải block thật BSC ngày 2026-09-15).

## 7. REGISTRY
Không đổi pin. `DEX_REGISTRY.md` giữ nguyên — cụm này không thêm/sửa router
nào, chỉ thêm khả năng đọc log `Sync` của pool ĐÃ pin (V2).

## 8. KHÔNG LÀM
- Không bật live/`bot_armed`/`dry_run=false`, không ký/gửi tx.
- Không sửa `.env` (2 máy — Chủ tự chạy scp), dòng token `pairs.txt`,
  `config.toml` giá trị, cờ live.
- Không sửa `decoder.rs` (0 dòng thay đổi trong file này cả phiên).
- Không giao việc ghi/sửa file cho subagent/fork (luật #4) — mọi phân tích
  14 case (RPC read-only) do phiên chính tự chạy bằng Bash+Python.
- Không cập nhật dashboard tĩnh (`web/app.js`/`index.html`) để hiển thị
  `top_pools`/`pending`/`/api/compete` mới — API đã sẵn sàng (`curl`
  dùng được ngay) nhưng UI web CHƯA vẽ thêm khối, ghi CÒN NỢ (lệnh không
  yêu cầu tường minh, ưu tiên thời gian cho phần backend/dữ liệu thật).
- Không tính `competitor_profit_bnb` từ Swap log trong `compete.check`
  (chỉ so gas_price) — ngoài phạm vi thời gian cụm này.
- Không wire nonce gate cho nhánh USDT (chỉ WBNB).

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **Dashboard tĩnh** (`web/app.js`/`web/index.html`) chưa vẽ `top_pools`/
  `pending` (`/api/pairs`)/`compete` (`/api/compete`) — dữ liệu có sẵn qua
  API, chỉ thiếu UI. Cần 1 lát riêng nếu Chủ muốn xem trực quan thay vì
  `curl`/`jq`.
- **Nonce gate v2** đấu dây đúng nhưng KHÔNG có số liệu thật (NO-OP) vì
  `NonceCache` chưa có nguồn điền nào chạy trên đường nóng `sim_engine="v2"`
  — cần lệnh riêng nếu Chủ muốn có `nonce_stale`/`nonce_future` thật (thêm
  1 nguồn điền cache, ví dụ task nền `eth_getTransactionCount` định kỳ cho
  ví đang có candidate).
- **`compete.check`** chưa tính `competitor_profit_bnb` (chỉ so gas) — cần
  decode thêm Swap log (token0/token1 + amountOut) của tx nghi ngờ.
- **p95 latency cải thiện nhờ Sync-event cụ thể bao nhiêu** — KHÔNG đo
  được tách biệt (không có A/B cùng điều kiện thị trường); chỉ biết KẾT QUẢ
  TỔNG đã đạt mục tiêu <500ms cả 2 máy (321ms WSL, 44ms VPS).
  `pairs_reload_sec`/`gas_units_boot_task` (`known_pair`/`ReserveCache` từ
  cụm trước) vẫn đóng góp phần lớn — không tách được % của riêng Sync-event.
- **Log rotation (`paper_run.sh`, ≥200MB)** — code + logic đã viết, CHƯA có
  bằng chứng runtime kích hoạt thật (file `logs/bot.jsonl` chưa chạm 200MB
  trong phiên này, hiện ~112MB sau nhiều lần chạy).
- **`decision_vs_mined_block`** đo được nhưng CHƯA đối chiếu với kết luận
  audit F1 cũ ("chưa tính cạnh tranh/latency thực thi") — số liệu mới
  (đa số -1, tức "kịp về lý thuyết") KHÔNG thay đổi kết luận đó (chưa tính
  chi phí broadcast/relay/latency mạng thật của chính bot).
- **`.claude/` xuất hiện trên VPS** sau deploy (thư mục settings local của
  Claude Code, không phải mã nguồn) — vô hại, không dọn trong phiên này
  (ngoài phạm vi lệnh).
- Danh sách nợ đầy đủ hơn: xem `docs/TASKS.md` mục "Nợ CÒN THẬT theo
  chiến lược mode 2" (đã cập nhật phiên này).

Commit chính: `1f0884a` (Phần A), `9dd725a` (Phần B — fix bug gốc +
compete/sync), `5284bd3` (deploy_vps.sh fix + deploy VPS thật), `5b17a83`
(docs, không đổi binary).
