# BAOCAO28

## 1. LÁT

`vps-latencyprobe-retest` — 2 việc dính trong 1 phiên VPS (không đụng
`config.toml` LOCAL): (A) đọc đúng 2 dòng `BSC_HTTP=`/`BSC_WS=` từ `.env`
**LOCAL** của repo, ghi đè đúng 2 dòng đó vào `.env` VPS qua SSH edit
(KHÔNG copy/transfer nguyên file, KHÔNG đọc/in `PRIVATE_KEY`), restart
service để nạp `.env` mới, giữ nguyên ngưỡng=0 + tax-inject 0bps 3 token
như BAOCAO27, rerun mode 3 (universal đơn độc) đúng 5 phút. (B) đo round-trip
TỪNG CHẶNG (bot → RPC BSC toàn danh sách → BlockRazor/48club → block header)
bằng lệnh đọc/vô hại, viết 2 script mới trong `scripts/` (không sửa
relay.rs/pipeline.rs/executor.rs).

## 2. LỆNH NHẬN (rút gọn)

ĐỌC CLAUDE.md/STATE(`vps-lowthreshold-retest`)/TASKS/config.toml/`src/relay.rs`/
BAOCAO27. Đăng nhập VPS bằng key `key/bsc_vps_ed25519` trước, chỉ xin
password nếu key fail. (A) đọc 2 dòng `BSC_HTTP=`/`BSC_WS=` TRỰC TIẾP từ
`.env` LOCAL, ghi ĐÈ đúng 2 dòng đó vào `.env` VPS (các dòng khác giữ
nguyên, không đụng `PRIVATE_KEY`/`PRIVATE_TX_URL` VPS). Rerun mode 3 đúng 5
phút, GIỮ ngưỡng=0 + tax-inject 0bps 3 token như BAOCAO27 (re-inject vì
cache hết hạn). Mục tiêu: lần ĐẦU TIÊN có 1 dòng "Simulated" THẬT. (B) đo
độ trễ từng chặng: 3× `eth_blockNumber`/URL trong toàn bộ `BSC_HTTP`; 3×
method vô hại (`eth_chainId`) tới `BLOCKRAZOR_RPC_URL`/`CLUB48_RPC_URL` pin
trong `relay.rs`; block/validator latency = `|giờ hệ thống lúc nhận
rpc.block − block.header.timestamp|` cho toàn bộ block trong cửa sổ 5 phút
của (A). GHI RÕ đây là round-trip từng chặng riêng lẻ, KHÔNG phải end-to-end
1 tx thật (dry_run=true xuyên suốt). Halt sạch cuối phiên.

**Giữa phiên**: Chủ chưa cung cấp VPS host — đã hỏi qua `AskUserQuestion`
(đúng luật bảo mật CLAUDE.md: IP VPS KHÔNG được lưu trong bất kỳ file nào
của repo, kể cả gitignored, nên mỗi phiên phải xin lại trực tiếp). Chủ dán
`<VPS_HOST>:22`, `root` — khớp `~/.ssh/known_hosts` cục bộ, verify keyless
login OK trước khi làm bất kỳ thao tác nào khác. (IP thật KHÔNG ghi lại ở
đây — đúng CẤM CỨNG CLAUDE.md.)

## 3. FILE ĐỔI

**Repo local (git)**:
- `scripts/latency_probe.sh` (mới) — bash thuần + curl, đo 3×
  `eth_blockNumber`/URL trong `BSC_HTTP` + 3× `eth_chainId` tới 2 relay pin
  (`CLUB48_RPC_URL`/`BLOCKRAZOR_RPC_URL` đọc trực tiếp từ `src/relay.rs`),
  in bảng avg/min/max/err, KHÔNG Python (đúng CLAUDE.md "Cấm ... Python
  runtime").
- `scripts/block_latency.sh` (mới) — bash + `eth_getBlockByNumber`, đọc
  `logs/bot.jsonl` lọc `rpc.block` trong cửa sổ `[since, until)`, đối chiếu
  `block.header.timestamp` on-chain, in avg/min/max delta ms. **Sửa 1 lần
  giữa phiên**: bản đầu dùng `while read` fork `grep`/`sed`/`date` cho MỖI
  dòng `rpc.block` trong TOÀN BỘ log (45451 dòng tích luỹ) → chạy quá
  120s/timeout — đã sửa sang awk 1-pass lọc trước rồi mới fork `date` cho số
  block đã lọc (xem ô 5, mục B).
- `docs/STATE.md` — thêm mục `vps-latencyprobe-retest` (phát hiện `BSC_WS`
  LOCAL chết hẳn 404, khác bản chất BAOCAO27; log `rpc.block` ngừng từ
  22:12:43Z; nợ mới `tx.skip.token` luôn `null`).
- `docs/TASKS.md` — thêm dòng nợ tương ứng.
- `baocao/BAOCAO28.md` (file này, mới).

**Trên VPS** (`/root/bsc-sandwich`, ngoài git):
- `.env`: thay đúng 2 dòng `BSC_HTTP=`/`BSC_WS=` bằng giá trị LOCAL (34 URL
  HTTP, 1 URL WSS `wss://bsc-dataseed1.bnbchain.org`). `PRIVATE_KEY`/
  `PRIVATE_TX_URL`/số dòng file (8 dòng) xác nhận KHÔNG đổi. Backup
  `.env.bak.<epoch>` giữ lại trên VPS.
- `config.toml`: KHÔNG đổi — verify đầu phiên 6 field từ BAOCAO27 vẫn giữ
  nguyên (`min_profit_bnb=0.0`, `min_reserve_wbnb=0.0`, `pairs_min_swap_bnb
  =0.0`, `wallet_scan_enabled=false`, `pair_scan_enabled=false`,
  `pair_scan_universal=true`).
- `scripts/latency_probe.sh`, `scripts/block_latency.sh`: scp lên
  `/root/bsc-sandwich/scripts/` để chạy tại chỗ (đo từ VPS — nơi bot thật
  gọi RPC, không phải từ máy dev).
- `state/tax_inject` qua `POST /api/tax` (15 vòng × 3 token = 45 lần, đều
  `ok:true`). `state/halt.lock` cập nhật bởi `POST /api/control` cuối phiên
  (đã tồn tại từ BAOCAO27, ghi lại đúng ý nghĩa).
- Đã dọn file tạm trên VPS cuối phiên (`window_start.txt`, `window_end.txt`,
  `tax_inject_log.txt`, `tax_loop_stdout.txt`, `measure_baseline_skips.json`)
  — không phải file sản phẩm, chỉ là artefact đo tạm của phiên này.

## 4. LỆNH CHẠY

```
# Đăng nhập
ssh -i key/bsc_vps_ed25519 root@<VPS> "whoami; date -u; hostname"
# -> root / Mon Sep 14 22:09:45 UTC 2026 / VPS-511043-157

# (A) đọc 2 dòng BSC_HTTP/BSC_WS LOCAL (không in PRIVATE_KEY), scp lên VPS,
# ghi đè qua script python3 CHỈ CHẠY TRÊN VPS (không phải stack sản phẩm,
# chỉ thao tác chỉnh sửa file 1 lần, xem ô 8), verify masked, restart:
scp -i key/bsc_vps_ed25519 <2-dong-local> root@<VPS>:/root/bsc_env_new_lines.tmp
ssh -i key/bsc_vps_ed25519 root@<VPS> "... thay 2 dong .env, rm file tmp, verify masked"
ssh -i key/bsc_vps_ed25519 root@<VPS> "systemctl restart bsc-sandwich-paper.service"
curl -s http://127.0.0.1:8787/api/status   # state=WATCHING, halt_lock=true (chua doi tu BAOCAO27)

# baseline /api/skips (22:16:54Z) -> tax inject loop 15x3 moi 20s (300s,
# 22:17:12Z -> 22:22:13Z) song song voi latency_probe.sh
curl -s http://127.0.0.1:8787/api/skips > baseline.json
# ... nohup loop POST /api/tax x3 token x15 vong ...
bash scripts/latency_probe.sh

# sau cua so: /api/status /api/skips /api/tax /api/hits?limit=50; tail -50 bot.jsonl; grep -c sim/Simulated
curl -s http://127.0.0.1:8787/api/status
curl -s http://127.0.0.1:8787/api/skips
curl -s http://127.0.0.1:8787/api/tax
curl -s 'http://127.0.0.1:8787/api/hits?limit=50'
tail -50 logs/bot.jsonl
grep -c '"event":"sim\.' logs/bot.jsonl; grep -c 'Simulated' logs/bot.jsonl

# (B) block latency (window)
bash scripts/block_latency.sh logs/bot.jsonl "2026-09-14T22:17:12Z" "2026-09-14T22:22:13Z"
# -> 0 dong rpc.block trong cua so -> live spot-check thay the (xem o 5)

# Halt cuoi phien
curl -s -X POST http://127.0.0.1:8787/api/control -H "Content-Type: application/json" -d '{"action":"halt"}'
```

## 5. OUTPUT THẬT

### (A) `/api/status` NGAY SAU restart (22:12:47Z, ~4s sau restart)

```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":true,"last_block":121915628,"live_gate":{"allow_live":false,
 "bot_armed":false,"chain_id_56":true,"not_dry_run":false,"not_halted":false},
 "max_exposure_bnb":5.0,"max_front_bnb":5.0,"min_profit_bnb":0.0,
 "pending_source":"txpool","state":"WATCHING","uptime_sec":10}
```
`pending_source:"txpool"` (KHÔNG phải `"ws"`) — dấu hiệu đầu tiên rằng WS
không hoạt động, xem phát hiện dưới.

### (A) baseline `/api/skips` (22:16:54Z, ~18s trước cửa sổ đo)

```
{"below_min":0,"deadline":0,"decode_fail":9105,"honeypot_or_tax":4,
 "hooks_unread":0,"no_pool":2,"not_in_list":0,"not_wbnb_pair":38,
 "thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
```

### (A) `/api/status` cuối cửa sổ 5 phút (uptime_sec=582, ~22:22:15Z)

```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":true,"last_block":121916895,"live_gate":{"allow_live":false,
 "bot_armed":false,"chain_id_56":true,"not_dry_run":false,"not_halted":false},
 "max_exposure_bnb":5.0,"max_front_bnb":5.0,"min_profit_bnb":0.0,
 "pending_source":"txpool","state":"WATCHING","uptime_sec":582}
```

### (A) `/api/skips` cuối cửa sổ (delta xấp xỉ 5 phút, baseline ở trên)

```
{"below_min":0,"deadline":0,"decode_fail":21670,"honeypot_or_tax":22,
 "hooks_unread":0,"no_pool":3,"not_in_list":0,"not_wbnb_pair":91,
 "thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
```
Delta: `decode_fail +12565, honeypot_or_tax +18, no_pool +1,
not_wbnb_pair +53`, mọi nhánh sâu hơn (`below_min/thin_liq/unprofitable/
victim_would_revert/deadline/hooks_unread/venue_unpinned/not_in_list`) đều
`+0`. **18 `honeypot_or_tax` mới = 18 swap WBNB thật đã qua decode + resolve
pool (`eth_call` thành công)** — pipeline chạm sâu vào mempool thật, khác
hẳn BAOCAO27 (0 mọi nhánh vì mempool rỗng).

### (A) `/api/tax` cuối cửa sổ (45/45 `POST` trong lúc đo đều `ok:true`)

```
{"allow_tax_inject":true,"current_block":121916895,"tax_cache_blocks":30,
 "entries":[
  {"fresh":false,"measured_at_block":121916817,"roundtrip_tax_bps":0,"token":"0x55d398326f99059ff775485246999027b3197955"},
  {"fresh":false,"measured_at_block":121916817,"roundtrip_tax_bps":0,"token":"0xe9e7cea3dedca5984780bafc599bd69add087d56"},
  {"fresh":false,"measured_at_block":121916817,"roundtrip_tax_bps":0,"token":"0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82"}]}
```
`fresh:false` ở THỜI ĐIỂM ĐỌC (78 block/~36s sau lần inject cuối, quá
`tax_cache_blocks=30`≈14s) — KHÔNG có nghĩa injection thất bại (45/45
`ok:true` xác nhận qua log `tax.inject`), chỉ là gap stale đã biết từ
BAOCAO27 (chưa sửa, xem ô 10).

### (A) `/api/hits?limit=50` — mẫu cuối cửa sổ (rút gọn 5/50 dòng, toàn bộ là `tx.seen`/`tx.skip` decode_fail thật)

```
{"event":"tx.skip","from":"0x3c5db57c0d1ab4e8f578c2c30365d18a5cb6bca2","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:22:25.855195579+00:00"}
{"event":"tx.seen","from":"0x8a23e3ca057e7b0874c2740bdf44152c505e40d6","source":"txpool","ts":"2026-09-14T22:22:25.855164460+00:00"}
{"event":"tx.seen","from":"0x943015fa73805864f65cd09f69474fcc2d019efc","source":"txpool","ts":"2026-09-14T22:22:25.855228602+00:00"}
{"event":"tx.skip","from":"0x943015fa73805864f65cd09f69474fcc2d019efc","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:22:25.855268699+00:00"}
{"event":"tx.skip","from":"0x8a23e3ca057e7b0874c2740bdf44152c505e40d6","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:22:25.855279359+00:00"}
```
(50/50 dòng đều `tx.seen`(source=`txpool`)/`tx.skip`(reason=`decode_fail`)
— **KHÔNG có dòng `Simulated`/`sim.*` nào**, đúng bản chất kết quả 0.)

### `tail -50 logs/bot.jsonl` (chạy sau halt, bot vẫn xử lý paper — 20 dòng cuối)

```
{"event":"tx.seen","from":"0x2e8179991608233707ec155d369d55a1af72718c","source":"txpool","ts":"2026-09-14T22:30:59.066682269+00:00"}
{"event":"tx.seen","from":"0x34b1b946e55c5aa1fe4c4b3dc313ff17f8af8149","source":"txpool","ts":"2026-09-14T22:30:59.066686587+00:00"}
{"event":"tx.seen","from":"0x6872b6630a3afcd3117191a8403c2002e13df7de","source":"txpool","ts":"2026-09-14T22:30:59.066690815+00:00"}
{"event":"tx.seen","from":"0x8403fc00abb756d7b2b6be8be85a1052b188a9aa","source":"txpool","ts":"2026-09-14T22:30:59.066698951+00:00"}
{"event":"tx.seen","from":"0x9ac2efcb4c75283ef2298a29bf725f10228f118d","source":"txpool","ts":"2026-09-14T22:30:59.066702978+00:00"}
{"event":"tx.seen","from":"0xe8d1c8d774c7af962a687fb97a69a413d81dd262","source":"txpool","ts":"2026-09-14T22:30:59.066707036+00:00"}
{"event":"tx.seen","from":"0xf50c05cb9fb15feda971b07e14a0219364329418","source":"txpool","ts":"2026-09-14T22:30:59.066711114+00:00"}
{"event":"tx.seen","from":"0xbfd5e3856c58fee3db5f5760538296c2ce7ae861","source":"txpool","ts":"2026-09-14T22:30:59.066715022+00:00"}
{"event":"tx.skip","from":"0x0c8f7a12f086df4cd78ac7b988a25f1b0dbb0260","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066708699+00:00"}
{"event":"tx.skip","from":"0x0249cec2aed450fbc883430be97469638666df19","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066749447+00:00"}
{"event":"tx.skip","from":"0x1403e9fca6f0d506bdeb689985f124fcd4e8d6c6","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066767001+00:00"}
{"event":"tx.skip","from":"0x8403fc00abb756d7b2b6be8be85a1052b188a9aa","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066767963+00:00"}
{"event":"tx.skip","from":"0x0cc43ebaa841ee6ecaf44e5ec4fd4517f9110f19","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066778173+00:00"}
{"event":"tx.skip","from":"0x11ca84f31c9638ea20c1e87471599cae365bdd52","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066788361+00:00"}
{"event":"tx.skip","from":"0x6872b6630a3afcd3117191a8403c2002e13df7de","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066791298+00:00"}
{"event":"tx.skip","from":"0x34b1b946e55c5aa1fe4c4b3dc313ff17f8af8149","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066789253+00:00"}
{"event":"tx.skip","from":"0xe8d1c8d774c7af962a687fb97a69a413d81dd262","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066840601+00:00"}
{"event":"tx.skip","from":"0xf50c05cb9fb15feda971b07e14a0219364329418","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066926567+00:00"}
{"event":"tx.skip","from":"0xbfd5e3856c58fee3db5f5760538296c2ce7ae861","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066933029+00:00"}
{"event":"tx.skip","from":"0x9ac2efcb4c75283ef2298a29bf725f10228f118d","reason":"decode_fail","source":"none","token":null,"ts":"2026-09-14T22:30:59.066847395+00:00"}
```

### `grep -c` `sim.`/`Simulated` — TOÀN BỘ log (không chỉ cửa sổ)

```
grep -c '"event":"sim\.' logs/bot.jsonl   -> 0
grep -c 'Simulated' logs/bot.jsonl        -> 0
```
**Vẫn 0 Simulated.** Không có bất kỳ dòng nào để dán làm bằng chứng —
đúng luật "không bịa": KHÔNG có Simulated thật để báo cáo lần này.

### PHÁT HIỆN CHÍNH — `BSC_WS` LOCAL chết hẳn (404), khác BAOCAO27

```
{"event":"rpc.failover","reason":"rpc khong ket noi duoc: HTTP error: 404 Not Found","transport":"ws","ts":"...22:12:43.986266668+00:00","url":"wss://bsc-dataseed1.bnbchain.org/***"}
{"event":"rpc.failover","reason":"rpc khong ket noi duoc: HTTP error: 404 Not Found","transport":"ws_heads","ts":"...22:12:43.986378111+00:00","url":"wss://bsc-dataseed1.bnbchain.org/***"}
{"event":"rpc.skip","reason":"khong URL WSS nao trong danh sach connect/subscribe_blocks duoc","transport":"ws_heads","ts":"...22:12:43.986383502+00:00"}
{"event":"rpc.pending_subscribed","transport":"txpool_content","ts":"...22:12:44.263000261+00:00"}
```
`.env` LOCAL đặt `BSC_WS=wss://bsc-dataseed1.bnbchain.org` — domain dataseed
công khai của BNB Chain chỉ phục vụ HTTP, KHÔNG có route WSS (404, không
phải lỗi quota/rate-limit như Alchemy ở BAOCAO27). Hệ quả: (1) nhánh pending
WS thất bại → rơi xuống fallback HTTP `txpool_content` (đã có sẵn, xoay
vòng 36 URL) → **fallback này chạy được thật** (`pending_source:"txpool"`,
hàng nghìn `tx.seen`/giây) → mempool KHÔNG rỗng, khác hẳn BAOCAO27; (2)
nhánh head-subscribe (`rpc.block`) thất bại và KHÔNG có fallback → dòng
`rpc.block` CUỐI CÙNG trong toàn bộ log = block `121915606` lúc
`22:12:43.668Z` — sau đó KHÔNG còn dòng `rpc.block` nào (`grep -c` không
tăng), dù `last_block` trong `/api/status` vẫn tăng đều (nguồn khác:
`http_pool_health_check`, không log JSONL khi thành công).

**Suy luận 0 Simulated lần này**: 18 `honeypot_or_tax` mới trong cửa sổ =
18 swap WBNB thật đã qua decode+pool nhưng không khớp 3 token đã inject
(USDT/CAKE/BUSD) — KHÔNG thể xác nhận 100% vì `tx.skip.token` luôn `null`
(nợ mới, xem ô 10), chỉ suy luận: (a) 3 token lớn là mẫu hiếm trong 5 phút
giữa hàng nghìn pool WBNB; (b) `tax_cache_blocks=30`(~14s tươi) < chu kỳ
re-inject 20s → gap stale ~6s/chu kỳ (nợ cũ BAOCAO27, chưa sửa) có thể đã
"ăn" đúng lúc 1 trong 18 tx chạm 1 trong 3 token. Không bịa kết luận dứt
khoát.

### (B) Hop-latency — `scripts/latency_probe.sh`, chạy TRÊN VPS trong cửa sổ đo, 3 mẫu/endpoint

```
=== (B) latency probe — 2026-09-14T22:17:21Z ===
--- BSC_HTTP list (eth_blockNumber x3) ---
http     https://bsc-dataseed1.bnbchain.org            ok=3/3 avg=71ms  min=58ms  max=90ms
http     https://jp-bscscutum.blockrazor.xyz           ok=3/3 avg=432ms min=410ms max=448ms
http     https://bsc.blockrazor.xyz                    ok=3/3 avg=74ms  min=65ms  max=84ms
http     https://bsc-dataseed2.bnbchain.org            ok=3/3 avg=63ms  min=59ms  max=68ms
http     https://bsc-dataseed4.defibit.io              ok=3/3 avg=68ms  min=66ms  max=71ms
http     https://bsc-dataseed3.bnbchain.org            ok=3/3 avg=69ms  min=60ms  max=86ms
http     https://bsc-dataseed2.defibit.io              ok=3/3 avg=74ms  min=59ms  max=105ms
http     https://bsc-dataseed2.ninicoin.io             ok=3/3 avg=64ms  min=61ms  max=69ms
http     https://bsc-dataseed4.bnbchain.org            ok=3/3 avg=68ms  min=60ms  max=82ms
http     https://bsc-dataseed3.defibit.io              ok=3/3 avg=79ms  min=61ms  max=89ms
http     https://bsc-dataseed4.ninicoin.io             ok=3/3 avg=67ms  min=65ms  max=70ms
http     https://bsc-dataseed3.ninicoin.io             ok=3/3 avg=72ms  min=63ms  max=85ms
http     https://ger-bscscutum.blockrazor.xyz          ok=3/3 avg=225ms min=156ms max=267ms
http     https://bsc-dataseed1.defibit.io              ok=3/3 avg=69ms  min=68ms  max=71ms
http     https://bsc-dataseed1.ninicoin.io             ok=3/3 avg=63ms  min=60ms  max=68ms
http     https://public-bsc.nownodes.io                ok=3/3 avg=206ms min=194ms max=214ms
http     https://rpc.solidrpc.io                       ok=3/3 avg=310ms min=198ms max=514ms
http     https://56.rpc.thirdweb.com                   ok=3/3 avg=184ms min=147ms max=203ms
http     https://us-bscscutum.blockrazor.xyz           ok=3/3 avg=84ms  min=70ms  max=103ms
http     https://ire-bscscutum.blockrazor.xyz          ok=3/3 avg=155ms min=141ms max=177ms
http     https://bsc-mainnet.gateway.tatum.io          ok=3/3 avg=85ms  min=71ms  max=114ms
http     https://rpc.sentio.xyz                        ok=3/3 avg=142ms min=135ms max=151ms
http     https://binance.rpc.thirdweb.com              ok=3/3 avg=239ms min=170ms max=329ms
http     https://api.uniblock.dev                      ok=1/3 avg=246ms err=http_429 "Throughput limit 1000 CUs/sec..."
http     https://rpc.owlracle.info                     ok=3/3 avg=212ms min=175ms max=247ms
http     https://bsc.api.pocket.network                ok=3/3 avg=369ms min=355ms max=396ms
http     https://bsc-rpc.blockreq.com                  ok=3/3 avg=214ms min=123ms max=267ms
http     https://xrpc.cl                               ok=3/3 avg=696ms min=334ms max=1062ms
http     https://rpc.nodeflare.app                     ok=1/3 avg=391ms err=http_429 "1 per 10s"
http     https://shared.us-east-1.getblock.io          ok=3/3 avg=77ms  min=74ms  max=82ms
http     https://bsc.leorpc.com                        ok=3/3 avg=338ms min=326ms max=349ms
http     https://bsc.merkle.io                         ok=3/3 avg=82ms  min=64ms  max=95ms
(tổng 34 URL trong BSC_HTTP, 2 URL bị rate-limit 429 ngay ở mẫu 2-3)

--- Relay pinned (src/relay.rs) — eth_chainId x3, vô hại ---
relay    https://puissant-builder.48.club              ok=3/3 avg=88ms min=74ms max=111ms
relay    https://bsc.blockrazor.xyz                    ok=3/3 avg=73ms min=67ms max=83ms
```
GHI RÕ: round-trip TỪNG CHẶNG riêng lẻ bằng lệnh đọc/vô hại
(`eth_blockNumber`/`eth_chainId`), KHÔNG phải end-to-end 1 tx thật
(`dry_run=true` xuyên suốt, không ký/gửi gì).

### (B) Block/validator latency — `scripts/block_latency.sh` trên cửa sổ `[22:17:12Z, 22:22:13Z)`

```
=== (B) block/validator latency — cua so [2026-09-14T22:17:12Z, 2026-09-14T22:22:13Z) ===
block rpc.block distinct trong cua so: 0
MISSING: khong co block nao trong cua so nay trong logs/bot.jsonl
```
**MISSING đúng nghĩa đen** — 0 dòng `rpc.block` tồn tại trong cửa sổ vì
nhánh head-subscribe đã chết từ `22:12:43Z` (trước cả khi cửa sổ đo bắt
đầu), xem phát hiện ở trên. Thay bằng **live spot-check** (KHÔNG thuộc cửa
sổ 5 phút của A, chạy ngay lúc phân tích để không bịa số):

```
block=121917864 block_ts_s=1789424979 recv_now_ms=1789424979928 delta_ms=928
block=121917868 block_ts_s=1789424981 recv_now_ms=1789424982038 delta_ms=1038
block=121917873 block_ts_s=1789424983 recv_now_ms=1789424984105 delta_ms=1105
```
`delta = |giờ hệ thống lúc gọi − block.header.timestamp|` = 928/1038/1105ms
(3 mẫu, cách nhau 2s) — hợp lý với chu kỳ khối quan sát ~0.46–1s và
timestamp header chỉ có độ chính xác giây (không phải latency mạng thuần).
GHI RÕ: đây KHÔNG phải số trong cửa sổ (A), chỉ là ước lượng thay thế gần
đúng nhất có sẵn khi log rỗng.

## 6. CHAIN

```
eth_chainId (primary bsc-dataseed1.bnbchain.org): {"jsonrpc":"2.0","id":1,"result":"0x38"}   # = 56 dung
eth_getCode WBNB 0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c:
  {"jsonrpc":"2.0","id":1,"result":"0x6060604052600436106100af576000357c010000...  (getCode > 0, dung venue pin)
```

## 7. REGISTRY

Không đổi — `DEX_REGISTRY.md` không nằm trong phạm vi ĐƯỢC ĐỤNG của cụm
này, không liên quan pin venue mới.

## 8. KHÔNG LÀM

- Không bật `allow_live`/`dry_run=false`/`bot_armed` — xác nhận
  `/api/status` `dry_run:true, allow_live:false, bot_armed:false` xuyên
  suốt.
- Không `sendRaw`, không ký tx thật, không gọi `eth_sendBundle`/
  `eth_sendMevBundle` tới BlockRazor/48club (chỉ `eth_chainId` đọc, vô hại).
- Không ghi `PRIVATE_TX_URL` (vẫn ngoài phạm vi, giữ nguyên như BAOCAO27).
- Không đọc/in/log `PRIVATE_KEY` dưới bất kỳ hình thức nào — chỉ đọc đúng 2
  dòng `BSC_HTTP=`/`BSC_WS=` từ `.env` LOCAL (verify `grep -ci PRIVATE_KEY`
  trên file trích xuất = 0 trước khi scp).
- Không copy/transfer nguyên file `.env` giữa 2 máy — chỉ scp 1 file tạm
  chứa đúng 2 dòng cần thiết, xoá ngay sau khi dùng trên VPS.
- Không sửa `config.toml` LOCAL, không sửa `executor.rs`/`calldata.rs`/
  `pool.rs`/`pipeline.rs`/`relay.rs`/`main.rs` — 2 script đo mới nằm hoàn
  toàn trong `scripts/`, không đụng code sản phẩm.
- Không đổi pair khỏi WBNB.
- Không in IP/host VPS ra bất kỳ file nào trong repo (kể cả file này) —
  chỉ dùng trong lệnh SSH/scp trực tiếp qua Bash tool.
- Không tự ghi chữ ĐẠT.

## 9. CHỮ

CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

- **Vẫn 0 Simulated** — nguyên nhân lần này KHÔNG phải mempool rỗng (đã
  chứng minh HTTP `txpool_content` fallback chạy thật), mà là (a) mẫu 5
  phút không chắc trúng đúng 1 trong 3 token đã inject, (b) gap stale cache
  ~6s/chu kỳ (nợ cũ BAOCAO27, CHƯA sửa: `tax_cache_blocks=30` ~14s tươi <
  20s chu kỳ re-inject — có thể giảm chu kỳ re-inject xuống ~10s hoặc tăng
  `tax_cache_blocks` ở phiên sau nếu muốn thu hẹp gap). Đề xuất phiên sau:
  hoặc (i) kéo dài cửa sổ đo (30–60 phút thay vì 5 phút) để tăng xác suất
  trúng 1 trong 3 token lớn, hoặc (ii) inject nhiều token hơn (mở rộng danh
  sách zero-tax ngoài USDT/CAKE/BUSD), hoặc (iii) dùng
  `state/inject_tx.jsonl` giả (đã đề xuất ở BAOCAO27, chưa làm) để chứng
  minh đường sim end-to-end thông mà không phụ thuộc mempool thật.
- **Nợ mới quan trọng**: `tx.skip` log KHÔNG BAO GIỜ ghi `token` thật (luôn
  `null`, kể cả nhánh đã biết token như `honeypot_or_tax`/`no_pool`) — chặn
  khả năng đối chiếu "tx nào chạm đúng token đã inject tax" từ log. Cần 1
  khối lệnh riêng sửa logger trong `handle_paper_tx`/`decide_paper_v2` (có
  đụng code sản phẩm, ngoài phạm vi cụm đo thuần này).
- **`BSC_WS` LOCAL chết (404) cho head-subscribe** — `rpc.block` JSONL
  event đã ngừng từ `22:12:43Z`, không ảnh hưởng pipeline paper (đã có
  fallback HTTP cho pending), nhưng làm mất khả năng đo block-latency từ
  log thật (phải dùng live spot-check thay thế lần này). Nếu muốn khôi phục
  `rpc.block`/head-subscribe: cần chọn 1 URL WSS THẬT SỰ hỗ trợ (không phải
  domain HTTP dataseed) — quyết định thuộc Chủ/Grok, cần khối lệnh riêng.
- `pairs.txt` VPS vẫn lỗi parse toàn bộ (`pair.parse_error` tiếp tục tăng
  cumulative) — chưa đụng, đúng phạm vi lệnh (nợ từ BAOCAO27, chưa soi).
- `PRIVATE_TX_URL` Chủ dán ở BAOCAO27 vẫn CHƯA ghi — ngoài phạm vi, cần
  khối lệnh Grok riêng.
