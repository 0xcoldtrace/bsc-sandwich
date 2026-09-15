1. LÁT: `vps-lowthreshold-retest` — TRÊN VPS (BAOCAO11/12/26, không đụng
config.toml local): (A) hạ 3 ngưỡng test về 0, (B) tax-inject thủ công 3
token zero-tax (USDT/CAKE/BUSD) `roundtrip_tax_bps=0`, (C) ghi
`BSC_HTTP`/`BSC_WS` mới vào `.env` VPS, rồi chạy lại dry-run **mode 3
(universal ĐƠN ĐỘC)** đúng 5 phút, lấy dữ liệu thật kể cả nếu 0 Simulated.
Làm nốt việc dính cùng phiên: (tiền điều kiện) redact IP VPS rò rỉ trong
BAOCAO26; (Chủ cho phép giữa phiên) tạo SSH key lưu vào `key/` của dự án +
nạp public key lên VPS. Halt sạch cuối phiên.

2. LỆNH NHẬN (rút gọn): ĐỌC CLAUDE.md/STATE(`vps-2phase-dryrun`)/TASKS/
BAOCAO26 + XÁC NHẬN `grep 157.254.48.103` = 0 (sửa TRƯỚC nếu chưa). (A)
`min_profit_bnb=0, pairs_min_swap_bnb=0, min_reserve_wbnb=0` TRÊN VPS, GIỮ
`max_roundtrip_tax`. (B) inject tax 3 token lớn zero-tax qua cơ chế có sẵn
(`POST /api/tax` / `state/tax_inject.jsonl`, `allow_tax_inject=true`),
GHI RÕ đây là số DO CHỦ/GROK CUNG CẤP (hiểu biết công khai), KHÔNG đo tự
động. (C) KHÔNG copy file `.env`; Chủ dán URL trực tiếp, Code chỉ ghi/thay
dòng `BSC_HTTP=`/`BSC_WS=` qua SSH edit. Rerun mode 3 5 phút, lấy
`/api/status`, `/api/skips`, `/api/hits?limit=50`, 50 dòng cuối
`logs/bot.jsonl`, đếm `sim.*`/`Simulated`. CẤM: cờ live, sendRaw, ký thật,
gọi relay BlockRazor/48club, copy `.env`, sửa PRIVATE_KEY, config.toml
LOCAL, đổi pair khỏi WBNB, sửa executor/calldata/pool/pipeline/relay/main.

**Giữa phiên Chủ dán thêm (ngoài phạm vi lệnh gốc)**: (i) `PRIVATE_TX_URL=`
(3 relay 48club/blockrazor), (ii) yêu cầu "login vps tạo ssh key, bỏ vào
thư mục key của dự án máy dev". Xử lý: xem ô 8 + ô 10.

3. FILE ĐỔI:

**Repo local (git)**:
- `baocao/BAOCAO26.md` — TIỀN ĐIỀU KIỆN: dòng 42 rò IP `157.254.48.103` +
  user `root` → thay bằng placeholder `<VPS_HOST>`/`<VPS_USER>` (đúng CẤM
  CỨNG CLAUDE.md "in mật khẩu/đăng nhập ra chat/BAOCAO"). `grep` toàn repo
  sau sửa = **0 kết quả** (xác nhận ô 5).
- `baocao/BAOCAO27.md` (file này, mới).
- `docs/STATE.md` — thêm mục `vps-lowthreshold-retest` (phát hiện Alchemy WS
  KHÔNG stream pending trên BSC).
- `docs/TASKS.md` — thêm dòng nợ (pending-source + pairs.txt parse VPS).
- `key/bsc_vps_ed25519` + `.pub` — SSH keypair ed25519 MỚI (Chủ cho phép
  giữa phiên). `key/` ĐÃ nằm sẵn trong `.gitignore` (xác nhận
  `git check-ignore key/` + `git ls-files | grep key` = rỗng) → private key
  KHÔNG bao giờ bị commit. KHÔNG sửa `.gitignore` (đã có sẵn `key/`).

**Trên VPS** (`/root/bsc-sandwich`, ngoài git):
- `.env`: thay 2 dòng `BSC_HTTP=`/`BSC_WS=` bằng list Chủ đưa (primary =
  Alchemy). `PRIVATE_KEY` GIỮ NGUYÊN (xác nhận). KHÔNG đụng `PRIVATE_TX_URL`
  (đã có sẵn trong `.env` VPS từ trước phiên này — KHÔNG do Code thêm/xoá).
  KHÔNG copy/transfer file `.env` (chỉ đọc-sửa-ghi 2 dòng qua SFTP).
- `config.toml`: 6 field — `min_profit_bnb=0.0`, `min_reserve_wbnb=0.0`,
  `pairs_min_swap_bnb=0.0`, `wallet_scan_enabled=false`,
  `pair_scan_enabled=false`, `pair_scan_universal=true`. KHÔNG đổi field
  khác (dry_run/allow_live/bot_armed/live_*/max_roundtrip_tax giữ nguyên).
- `state/tax_inject` qua `POST /api/tax` (15 vòng × 3 token = 45 lần, đều
  `ok:true`). `state/halt.lock` tạo bởi `POST /api/control` cuối phiên.
- `~/.ssh/authorized_keys`: append 1 dòng public key ed25519 mới (idempotent).

4. LỆNH CHẠY (paramiko, password — không có SSH key nào cấp sẵn cho phiên
trắng; đúng tiền lệ BAOCAO12/26; credential CHỈ qua env, đã xoá file env
tạm scratchpad cuối phiên, KHÔNG ghi ra file repo nào):
```
# PHA setup: whoami/date -u; systemctl stop bot cũ; SFTP đọc-sửa .env
#   (BSC_HTTP/BSC_WS); SFTP đọc-sửa config.toml (6 field); rm halt.lock;
#   systemd-run transient (source .env; exec target/release/bsc_sandwich);
#   sleep 25; /api/status.
# PHA measure 300: baseline /api/skips; POST /api/tax x3 (buy_tax=0,
#   sell_tax=0) mỗi 20s trong 300s; cuối: /api/status /api/skips /api/tax
#   /api/hits?limit=50; tail -50 bot.jsonl; grep -c sim.
# PHA post: DIAG pending/tx.seen theo cửa sổ; append pubkey + verify keyless;
#   POST /api/control halt.
```

5. OUTPUT THẬT:

**TIỀN ĐIỀU KIỆN — `grep 157.254.48.103` toàn repo sau khi sửa BAOCAO26**:
```
(No matches found)   # 0 kết quả — đạt điều kiện trước khi làm A/B/C
```

**setup — /api/status sau ~25s (bot mới, dry-run, cổng live khoá)**:
```
{"allow_live":false,"bot_armed":false,"chain_id":56,"dry_run":true,
 "halt_lock":false,"last_block":121911303,
 "live_gate":{"allow_live":false,"bot_armed":false,"chain_id_56":true,
 "not_dry_run":false,"not_halted":true},"max_exposure_bnb":5.0,
 "max_front_bnb":5.0,"min_profit_bnb":0.0,"pending_source":"ws",
 "state":"WATCHING","uptime_sec":26}
CANH BAO: min_profit_bnb=0 BNB thap hon tong gas front+back cap (6e15 wei)
```
config sau ghi (rút gọn field liên quan): `min_profit_bnb=0.0`,
`min_reserve_wbnb=0.0`, `pairs_min_swap_bnb=0.0`, `wallet_scan_enabled=false`,
`pair_scan_enabled=false`, `pair_scan_universal=true`, `dry_run=true`,
`allow_live=false`, `bot_armed=false`, `live_v2/v3/v4=false`,
`max_roundtrip_tax=0.005` (giữ nguyên). `.env` keys sau ghi:
`[PRIVATE_KEY, BSC_HTTP, BSC_HTTP_2..5, BSC_WS, PRIVATE_TX_URL]`;
`BSC_HTTP` primary = `https://bnb-mainnet.g.alchemy.com/***` (35 url),
`BSC_WS` = `wss://bnb-mainnet.g.alchemy.com/***`; `PRIVATE_KEY` giữ nguyên.

**(B) tax-inject — /api/tax NGAY SAU vòng đầu (fresh:true, bps=0)**:
```
{"allow_tax_inject":true,"current_block":121911687,"tax_cache_blocks":30,
 "entries":[
  {"fresh":true,"measured_at_block":121911681,"roundtrip_tax_bps":0,"token":"0x55d3...7955"},  # USDT
  {"fresh":true,"measured_at_block":121911683,"roundtrip_tax_bps":0,"token":"0x0e09...ce82"},  # CAKE
  {"fresh":true,"measured_at_block":121911685,"roundtrip_tax_bps":0,"token":"0xe9e7...d56"}]}   # BUSD
```
(45 lần POST đều `ok:true`. Đây là số DO CHỦ/GROK CUNG CẤP theo hiểu biết
công khai 3 token lớn không có transfer-tax — KHÔNG phải đo tự động thật,
đúng bản chất cơ chế tax-inject từ BAOCAO07.)

**rerun mode 3 — /api/status + /api/skips cuối cửa sổ 5 phút (SỐ THẬT)**:
```
STATUS: {"chain_id":56,"dry_run":true,"allow_live":false,"bot_armed":false,
 "halt_lock":false,"last_block":121912430,"pending_source":"ws",
 "state":"WATCHING","uptime_sec":533,"min_profit_bnb":0.0}
SKIPS:  {"below_min":0,"deadline":0,"decode_fail":0,"honeypot_or_tax":0,
 "hooks_unread":0,"no_pool":0,"not_in_list":0,"not_wbnb_pair":0,
 "thin_liq":0,"unprofitable":0,"venue_unpinned":0,"victim_would_revert":0}
```
**/api/hits?limit=50 + tail -50 bot.jsonl**: 100% là `rpc.block` (+ vài
`config.reload`/`victim.reload`). KHÔNG có 1 dòng `tx.seen`/`tx.skip`/`sim`
nào trong 50 dòng cuối. Ví dụ đại diện:
```
{"block":121912411,"event":"rpc.block",...}
{"event":"config.reload","min_profit_bnb":0.0,"min_reserve_wbnb":0.0,
 "max_roundtrip_tax":0.005,"max_front_bnb":5.0,...}
{"count":103,"error_lines":0,"event":"victim.reload",...}
```
**Đếm `sim.*`/`Simulated`**: `grep -ac 'sim\.' = 0`. `Simulated` = **0**.

**DIAG cửa sổ đo (ô này giải thích DỨT KHOÁT vì sao 0, KHÔNG bịa lý do)**:
```
rpc.pending_subscribed (ws) @ 21:40:01  url=wss://bnb-mainnet.g.alchemy.com/***
tx.seen COUNT trong cửa sổ 21:43..21:49 = 0
tx.seen CUỐI CÙNG trong toàn log @ 21:39:50 (source pending_ws) — TRƯỚC lúc
  bot Alchemy mới subscribe; đây là đợt xả cuối của bot CŨ (publicnode) lúc
  bị systemctl stop, KHÔNG phải bot Alchemy mới.
Đếm event theo cửa sổ 21:43..21:49:
  933 rpc.block | 45 tax.inject | 900 pair.parse_error | 18 config.reload
   9 pair.reload | 17 victim.reload | 0 tx.seen | 0 tx.skip | 0 sim
run.log: 0 dòng pending/subscribe/error/panic/txpool (ngoài CANH BAO gas).
```
**KẾT LUẬN THẬT**: 3 ngưỡng ĐÃ = 0 (chứng minh qua config + `/api/status
min_profit_bnb:0.0`) và tax 3 token ĐÃ = 0 bps fresh (chứng minh qua
`/api/tax`) — tức 2 rào cản của BAOCAO26 (`below_min`/`honeypot_or_tax`)
ĐÃ được gỡ đúng. Nhưng 0 Simulated vì **nguồn pending-tx RỖNG**: endpoint
Alchemy nhận subscribe (`pending_source:ws`, `rpc.pending_subscribed`) nhưng
**KHÔNG đẩy 1 pending tx nào** suốt ~9 phút (933 block nhận đều, 0 tx.seen).
Pipeline bị đói input → không tx nào tới bước sim. Khác hẳn BAOCAO26 (BSC_WS
= publicnode) nơi pending flood 85707 decode_fail/5 phút. **Nguyên nhân là
NGUỒN MEMPOOL (Alchemy không stream pending trên BSC), KHÔNG phải ngưỡng,
KHÔNG phải tax-cache, KHÔNG phải lỗi logic pipeline.**

**SSH key (Chủ cho phép giữa phiên)**:
```
append pubkey -> /root/.ssh/authorized_keys (wc -l = 1)
verify: KEYLESS_LOGIN_OK VPS-511043-157   # đăng nhập bằng key MỚI, không password
pub: ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK+dhLtgDcySotqEP0QS28Tr5VZFmMhbew1LOFb14Jv4 bsc-sandwich-vps
```

**Halt cuối phiên**:
```
POST /api/control {"action":"halt"} -> {"action":"halt","ok":true}
/api/status -> halt_lock:true, live_gate.not_halted:false, uptime_sec:737
state/halt.lock  (4 bytes, Sep 14 21:52)
```

6. CHAIN: `0x38` xác nhận THẬT xuyên suốt (`chain_id:56`,
`live_gate.chain_id_56:true` mọi lần gọi). `last_block` tăng liên tục THẬT:
`121911303` (sau setup) → `121912430` (cuối 5 phút) → `121912882` (sau halt)
— ~1580 block trong phiên, RPC Alchemy stream block đều. KHÔNG pin
address/contract mới, KHÔNG gọi `eth_getCode` mới phiên này.

7. REGISTRY: KHÔNG đổi. Venue y hệt BAOCAO02/22/26 (V2+V3+V4/Infinity pinned,
"bản mới hơn" DISABLED). KHÔNG sửa `DEX_REGISTRY.md`/`src/venues.rs`.

8. KHÔNG LÀM:
- KHÔNG bật `allow_live`/`dry_run=false`/`bot_armed` (VPS lẫn local) — xác
  nhận `false` mọi lần `/api/status`. KHÔNG sendRaw/executor/ký tx thật.
- KHÔNG gọi relay BlockRazor/48club (dry_run → không có bước submit
  bundle/tx; `relay.rs` chưa nối pipeline).
- **KHÔNG ghi `PRIVATE_TX_URL`** dù Chủ dán giữa phiên — ngoài phạm vi (C)
  (chỉ `BSC_HTTP`/`BSC_WS`), + CẤM lệnh liệt kê relay "không liên quan lệnh
  này", + tiền lệ BAOCAO26 (cần 1 khối lệnh Grok riêng). `.env` VPS vốn đã
  có sẵn dòng này từ trước — Code KHÔNG thêm/xoá/sửa nó.
- KHÔNG sửa `config.toml` LOCAL (chỉ VPS); KHÔNG copy/transfer file `.env`;
  KHÔNG đụng `PRIVATE_KEY`; KHÔNG sửa CLAUDE.md/victims.txt/DEX_REGISTRY.md/
  executor.rs/calldata.rs/pool.rs/pipeline.rs/relay.rs/main.rs.
- KHÔNG rút ngắn cửa sổ 5 phút; KHÔNG bịa số (0 Simulated là kết quả thật,
  có DIAG chứng minh nguyên nhân).
- **SSH key**: CHỈ làm sau khi Chủ cho phép TƯỜNG MINH giữa phiên. Lưu vào
  `key/` (đã gitignore sẵn) + nạp pubkey; KHÔNG in private key ra chat/BAOCAO,
  KHÔNG commit key, KHÔNG sửa `.gitignore`.

9. CHỮ: CHỜ GROK

10. CÒN NỢ / LÁT SAU:
- **PHÁT HIỆN CHÍNH — `BSC_WS`=Alchemy KHÔNG stream pending mempool trên
  BSC**: subscribe OK nhưng 0 pending tx/9 phút → bot (paper LẪN live tương
  lai) sẽ KHÔNG bao giờ thấy nạn nhân. Muốn thấy Simulated/vận hành thật cần
  1 trong 2: (a) đổi `BSC_WS` về node CÓ stream full-pending trên BSC
  (publicnode như BAOCAO26 đã chứng minh flood pending; hoặc dịch vụ mempool
  trả phí: blockrazor stream/bloXroute...) — cần 1 khối lệnh Grok ghi URL;
  (b) inject 1 `state/inject_tx.jsonl` giả (WBNB→USDT) để CHỨNG MINH đường
  sim end-to-end nay đã thông với ngưỡng=0 + tax=0 (không cần mempool thật) —
  KHÔNG làm phiên này vì lệnh chỉ yêu cầu "mode 3 universal đơn độc" + đã
  halt sạch; đề xuất làm ở khối lệnh sau nếu Grok muốn bằng chứng dứt điểm
  "không phải lỗi logic".
- **`PRIVATE_TX_URL`** (Chủ dán): CHƯA ghi — cần 1 khối lệnh Grok riêng
  (ĐỌC/LÀM/CẤM). Lưu ý `relay.rs` chưa nối `pipeline.rs`/`executor.rs` nên
  thêm biến này KHÔNG đổi hành vi paper ngay; và 1/3 relay từng không hỗ trợ
  `eth_sendBundle` (BAOCAO24) — cần verify route nếu ghép thật.
- **`pair.parse_error=900`** trong cửa sổ (9 reload × 100 dòng) → `pairs.txt`
  trên VPS đang lỗi parse TOÀN BỘ 100 dòng mỗi lần reload. KHÔNG ảnh hưởng
  test này (mode 3 universal không dùng `pairs.txt` để khớp) nhưng là lỗi
  thật cần soi ở phiên sau (ngoài phạm vi + CẤM sửa các file liên quan).
- **Tax freshness gap**: block rate quan sát ~0.46s/block → `tax_cache_blocks
  =30` chỉ tươi ~14s, trong khi re-inject mỗi 20s → có ~6s/chu kỳ entries bị
  stale. Không ảnh hưởng kết quả (0 pending tx dù sao) nhưng nếu chạy lại với
  nguồn pending thật, nên re-inject ≤10s hoặc tăng `tax_cache_blocks`.
- Kế thừa nợ cũ chưa đổi (BAOCAO08→26): V4/Infinity sim chưa có, đo tax thật
  (probe) chưa làm, `RiskGuard::record_result` chưa gọi, relay chưa nối dây.
