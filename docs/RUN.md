# docs/RUN.md — chạy PAPER 30 phút + vet `pairs.txt`

Mục đích: chạy bot ở chế độ **paper (dry_run)** trên máy thật (WSL hoặc VPS,
có `.env` với `BSC_HTTP`/`BSC_WS` thật) trong 30 phút, thu số liệu funnel/
skip/tax/validate — và lọc thô token ứng viên cho `pairs.txt` (mode 2) bằng
GoPlus Security trước khi Chủ tự vet tay.

Script dùng chung: `scripts/paper_run.sh` (phiên "wsl-env-rules-paperrun",
2026-09-15, cập nhật ở cụm `strategy-lock-mode2`) — chạy được cả trên WSL
(máy dev) lẫn VPS, không có bước SSH nào bên trong (SSH chỉ nằm ở
`scripts/deploy_vps.sh`, dùng để ĐƯA source lên VPS trước khi chạy, không
liên quan script paper run). `scripts/vps_paper_run.sh` (alias cũ forward
sang `paper_run.sh`) ĐÃ XOÁ ở cụm `docs-cleanup-mode2` — dùng thẳng
`scripts/paper_run.sh` cho cả WSL lẫn VPS. File này đổi tên từ
`docs/VPS_RUN.md` ở cụm `strategy-lock-mode2` (chạy được trên CẢ WSL lẫn
VPS, tên cũ gây hiểu nhầm chỉ dành cho VPS).

## Lọc thô `pairs.txt` bằng GoPlus (`scripts/vet_goplus.sh`)

Bước ĐẦU TIÊN trước khi Chủ tự vet tay + điền `vetted YYYY-MM-DD` vào
`pairs.txt` (xem AGENTS.md mục "Chiến lược đã chốt"):

```bash
scripts/vet_goplus.sh pairs.txt
```

- Gọi GoPlus Security `token_security` API công khai (chain 56, không cần
  key) cho MỖI token trong `pairs.txt`, in bảng `PASS`/`REVIEW`/`FAIL` +
  dòng tóm tắt số lượng.
- Yêu cầu `curl` + `jq` trên máy chạy. Gọi TUẦN TỰ (không batch — GoPlus âm
  thầm bỏ qua địa chỉ ngoài đầu tiên khi batch) + retry-backoff khi gặp
  rate-limit (`.code != 1` dù HTTP 200) — KHÔNG bao giờ đoán `PASS` khi
  thiếu dữ liệu thật.
- **CHỈ là lọc thô** — `PASS` nghĩa "không cờ đỏ/vàng nào từ GoPlus", KHÔNG
  phải "Chủ đã vet". Chủ vẫn phải tự đọc contract/BscScan/thử swap nhỏ trước
  khi điền `vetted` — script KHÔNG tự sửa `pairs.txt`.

## KHÔNG bao giờ

- Script `scripts/paper_run.sh` **KHÔNG** bật `allow_live`/`bot_armed`/
  `dry_run=false`, **KHÔNG** gửi tx, **KHÔNG** ký, **KHÔNG** in `PRIVATE_KEY`/
  URL RPC có token/IP VPS. Nó chỉ đọc file + gọi API local `127.0.0.1`.
- Redact mọi secret trước khi dán log cho Grok.

## Điều kiện

1. Máy chạy (WSL hoặc VPS) đã có repo + `.env` (chứa `BSC_HTTP`/`BSC_WS`
   thật của Chủ).
2. Đã `cargo build --release` được (script tự build lại).
3. `BSC_WS` nên là host `publicnode` (script tự verify host, **không in URL**).

## Chạy

```bash
# Trong thu muc repo, tren WSL hoac VPS deu chay duoc:
scripts/paper_run.sh --minutes 30 --port 8799
```

Script tự in máy đang chạy (`WSL`/`VPS`, tự phát hiện qua `/proc/version`) và
`sha256sum` của binary vừa build ngay đầu phần kết quả — dán nguyên vào
BAOCAO theo luật #2 (`AGENTS.md` mục "3 luật bổ sung").

Script sẽ:

1. Verify `BSC_WS` host = publicnode (grep host, không in URL đầy đủ).
2. `cargo build --release`.
3. Xoá `state/halt.lock` / `*.req` cũ.
4. Tạo 1 **config TẠM** (không đụng `config.toml` thật) copy từ `config.toml`
   rồi CHỈ override ngưỡng kinh tế về `0` (`min_profit_bnb`,
   `min_reserve_wbnb`, `max_roundtrip_tax`, `pairs_min_swap_bnb`,
   `min_profit_usdt`, `min_reserve_usdt`) + `web_port=<port>`. Cụm
   `strategy-lock-mode2`: KHÔNG còn ép `pair_scan_universal`/
   `scan_quote_usdt`/`sim_engine` — 3 field đó GIỮ NGUYÊN giá trị ship trong
   `config.toml` thật (mode 2 only, `sim_engine="v2"`), để paper run phản
   ánh ĐÚNG hành vi sản phẩm đang chạy thay vì 1 chế độ đo riêng.
5. In nhanh số dòng `pairs.txt` (tổng/đã có `vetted`/chưa vet — grep thô,
   xem log `pair.reload`/`pair.unvetted` để có số THẬT từ chính bot).
6. Chạy bot ngầm (binary nhận config làm **tham số vị trí thứ 1**), đợi 30 phút.
7. In: 30 dòng `funnel.minute` cuối, `/api/skips`, `/api/funnel`, `/api/tax`,
   `/api/pairs`, `/api/econ`, `/api/validate`, 20 dòng `tx.skip` (token+venue),
   10 dòng `sim.evm`, số `Simulated` (đếm `sim.evm` có `decision=simulated`).
8. `halt` sạch: ghi `state/halt.lock` + kill PID.

## Đọc kết quả

- `/api/funnel`: phễu theo gate — `seen → not_pancake_router → decode_fail →
  not_wbnb_pair → venue_v3|venue_v2 → no_pool → below_min → thin_liq →
  honeypot_or_tax → unprofitable → victim_would_revert → simulated`, cộng
  `sim_error` (số tx EVM/RPC không chạy được — nếu cao là RPC quá tải) và
  `gas_cap` (cụm `real-economics-mode2` — `gas_cost_wei` đo thật vượt trần,
  hoặc `eth_gasPrice` vượt `gas_price_max_gwei`).
- `/api/tax`: cache tax đo tự động bằng `pairs_vet_task` NỀN (mỗi dòng có
  `buy_bps`/`sell_bps`/`honeypot`/`quote`), TTL theo `tax_cache_ttl_sec`.
- `/api/pairs`: cụm `strategy-lock-mode2` — thêm cột `vetted_at`/`candidate`/
  `buy_bps`/`sell_bps`/`honeypot`/`last_vet_sec_ago` (kết quả `pairs_vet_task`
  gần nhất cho từng pool).
- `/api/econ` (cụm `real-economics-mode2`, đọc trực tiếp `logs/bot.jsonl`):
  bucket `victim_in` theo BNB (`<0.01`/`0.01-0.05`/`0.05-0.2`/`0.2-1`/`>=1`,
  mỗi bucket có `count`/`gross_pos`/`net_pos`/`sum_net_pos_bnb`/
  `best_net_bnb`/`median_gas_cost_bnb`), tách theo `by_quote`
  (`wbnb`/`usdt`), `top_tokens` (10 token nhiều dòng nhất), `decode_fail_by_router`
  (nhóm theo tên router: V2 Router/SwapRouter/SmartRouter/UR v3 (cũ)/UR
  Infinity), `latency_ms.p50`/`p95` (`seen_to_decision_ms`),
  `nonce_stale_pct_of_candidate`, và `summary_line` (dòng tổng dạng
  `candidate=<n> net_pos=<n> best_net_bnb=<x> p50_ms=<n> p95_ms=<n>
  stale_pct=<x> decode_fail_smartrouter=<n>`). Bucket BNB CHỈ áp dụng cho
  `quote=wbnb` (USDT không quy đổi được sang BNB nếu không có price oracle —
  AGENTS.md cấm oracle giá).
- `/api/validate`: chỉ số SỐNG của validator nhúng (B3.4) — `within_1pct_ratio`
  là tỉ lệ dự đoán victim khớp on-chain ≤1%. Cụm `real-economics-mode2`
  (F-27): tách riêng `isolated`/`non_isolated` (mỗi nhóm có `n`/
  `within_1pct`/`p50_lech_pct`/`p95_lech_pct`) — bộ đếm CŨ chỉ tính đúng
  nhóm `isolated` vào `within_1pct` tổng (bug audit F-27), giờ tổng cộng
  đúng cả 2 nhóm. Đây là thước đo độ chính xác sim thay cho việc replay
  sandwich lịch sử.

## Lưu ý hạ tầng (đo thật, xem `docs/STATE.md` mục cụm này)

- RPC công khai giữ state ~128 block (~96s). Fork EVM (`pairs_vet_task`,
  validator, đo lại trước ký live — xem AGENTS.md mục "Chiến lược đã chốt")
  luôn mở tại block hiện tại nên luôn trong cửa sổ — không phụ thuộc archive.
- Cụm `strategy-lock-mode2`: `sim_engine="v2"` (ship) — đường nóng KHÔNG mở
  fork EVM mỗi tx nữa, nên log `sim.evm`/`sim_error` từ đường nóng SẼ RỖNG
  trong 1 lần chạy bình thường (không phải lỗi — token trong `pairs.txt` đã
  vet, quyết định `Simulated` dùng công thức đóng V2 + gas thật). Muốn quan
  sát lại đường EVM per-tx (so sánh/đối chiếu), đổi tạm `sim_engine="evm"`
  trong `config.toml` (hot-reload, không cần build lại).

---

## CẢNH BÁO vận hành: rò rỉ bộ nhớ → OOM khi chạy dài (đo thật 2026-09-16)

Lần chạy paper 24 h trên VPS (8 GB RAM, binary commit `5284bd3`) **bị kernel
OOM-kill sau 656 phút** với `anon-rss 7,6 GB` (`dmesg`:
`Out of memory: Killed process 377294 (bsc_sandwich)`), tức ~11 MB/phút. Chưa
sửa được (xem `docs/TASKS.md` mục "Nợ CÒN THẬT").

Cho tới khi sửa xong, khi chạy dài trên VPS:

- Theo dõi RSS: `watch -n 60 'ps -o rss=,etime= -p $(cat state/paper_run.pid)'`.
  Mốc tham khảo: ~11 MB/phút ⇒ máy 8 GB đầy sau ~11 h.
- Chia thành nhiều phiên ngắn (4–6 h) thay vì 1 phiên 24 h, hoặc thêm
  `MemoryMax=` vào unit systemd + `Restart=always` để bot tự sống lại thay vì
  chết im lặng.
- Đọc `logs/paper24h.out`: dòng `BOT DA CHET sau N phut` là dấu hiệu bị kill,
  KHÔNG phải chạy xong (`scripts/paper_run.sh` in `DONE` khi hết giờ bình
  thường).

## Vận hành trên VPS

Checklist đầy đủ để đưa bot lên 1 VPS chạy paper (hoặc chờ live sau này).
Bot trên VPS **vẫn `dry_run=true`** như trên WSL — không có bước nào ở đây
tự bật live.

### 1. Chuẩn bị SSH key riêng cho VPS

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_vps -C "bsc-sandwich-vps"
ssh-copy-id -i ~/.ssh/id_vps.pub root@<ip_vps>   # hoặc dán tay vao ~/.ssh/authorized_keys tren VPS
```

Sau khi xác nhận đăng nhập bằng key được, **tắt đăng nhập bằng password**
trên VPS (sửa `PasswordAuthentication no` trong `/etc/ssh/sshd_config` rồi
`systemctl restart sshd`). Không bao giờ lưu password VPS vào bất kỳ file
nào trong repo (kể cả file gitignored).

### 2. Bật tường lửa, chỉ mở port 22

```bash
# Tren VPS:
ufw allow 22/tcp
ufw enable
apt-get install -y fail2ban   # tuy chon, chong brute-force SSH
```

Không mở port `8787` (dashboard) ra Internet — xem tunnel ở bước 6.

### 3. Cài `rustup`/`build-essential` trên VPS

`scripts/deploy_vps.sh` tự cài nếu thiếu khi copy source lần đầu — không
cần làm tay trừ khi script báo lỗi.

### 4. Copy source + build + chạy nền

```bash
scripts/deploy_vps.sh --host <ip_vps> --user root --identity ~/.ssh/id_vps \
  --build --run
```

- Đóng gói source (loại `.git/target/state/logs/artifacts/.env`) qua
  tar+SSH pipe, copy sang `/root/bsc-sandwich` (đổi bằng `--path`).
- `--build`: `cargo build --release` trên VPS.
- `--run`: chạy bot nền qua `systemd-run --unit=bsc-sandwich-paper --collect`
  (fallback `nohup` + cảnh báo nếu VPS không có `systemd-run`) — vẫn
  `dry_run=true` theo `config.toml` vừa copy, KHÔNG tự bật live/armed.
- `--probe`: chạy `scripts/run_rpc_probe.sh` trên VPS (cần `.env` đã điền
  RPC thật trên VPS trước).

### 5. Tự điền `.env` TRÊN VPS

**Không copy `.env` máy dev qua mạng** — `scripts/deploy_vps.sh` loại trừ
`.env` khỏi gói copy có chủ đích. SSH vào VPS rồi tự tạo:

```bash
cd /root/bsc-sandwich
cp .env.example .env
nano .env   # dien BSC_HTTP/BSC_WS that truc tiep tren VPS
```

### 6. Xem dashboard qua SSH tunnel (không mở port ra Internet)

```bash
ssh -N -L 8787:127.0.0.1:8787 -p <port> root@<ip_vps>
# roi mo http://127.0.0.1:8787 tren may cua ban
```

### 7. Xác nhận VPS cùng commit với WSL

Cụm `econ-truth-latency-vps` sửa `scripts/deploy_vps.sh` để GIỮ LẠI `.git`
khi copy (trước đó loại trừ, khiến bước này không chạy được). Nếu VPS báo
`fatal: detected dubious ownership` khi chạy `git`, chạy 1 lần (owner file
khác owner đang chạy lệnh — bình thường khi copy qua `tar+ssh` bằng `root`):

```bash
git config --global --add safe.directory /root/bsc-sandwich   # (hoac duong dan REMOTE_PATH da dung)
```

```bash
# Tren VPS:
git log -1 --format=%H
sha256sum target/release/bsc_sandwich
```

So 2 giá trị này với `git log -1 --format=%H` và `sha256sum` chạy trên WSL
(dev) — **khác nhau = MISSING**, chưa được coi là "đã deploy đúng bản" theo
`AGENTS.md` (Dev = WSL, Production = VPS, phải cùng commit). Ghi cả 2 cặp
giá trị vào BAOCAO khi báo cáo đã deploy.

### 7b. Config VPS — field bắt buộc khi nhận binary mới (CHƯA deploy cụm này)

Binary từ `planB-B4-multivenue-tool` (BAOCAO48) trở đi **fail load** nếu
`config.toml` trên VPS thiếu 4 field ngưỡng list đa venue. Thêm vào file
RIÊNG trên VPS **trước** khi copy binary (bài học BAOCAO46):

```toml
multivenue_min_v2_bnb = 50
multivenue_min_v2_usdt = 35000
multivenue_min_v3_impact_pct = 2
multivenue_probe_bnb = 1
```

Cụm `planB-B5-simarb-v3-measure` (BAOCAO51) thêm 2 field nữa — VPS phải có
**trước** khi nhận binary này (thiếu = fail load). **Chưa deploy** phiên này:

```toml
pairs_arb_path = "pairs_arb.txt"
gas_units_arb_v3 = 360000
```

`pairs_arb.txt` (list A both_ok đã vet) phải có trên VPS nếu `strategy="backrun"`
(PairBook đọc file này). Không đè `pairs.txt`.

### 8. Logrotate cho `logs/bot.jsonl`

`logs/bot.jsonl` không tự xoay vòng — nếu chạy VPS dài ngày, thêm 1 file
`/etc/logrotate.d/bsc-sandwich` trỏ vào `/root/bsc-sandwich/logs/bot.jsonl`
(hoặc dùng `systemd` journal nếu chạy qua `systemd-run`/unit thật) để tránh
đầy đĩa.

### 9. Dừng bot trên VPS

```bash
# Tren VPS, neu chay qua systemd-run --unit=bsc-sandwich-paper:
systemctl stop bsc-sandwich-paper
# Hoac dung state/halt.lock (dung ca 2 cach deu duoc, xem README.md muc 8):
touch /root/bsc-sandwich/state/halt.lock
```

---

## Lên live (chỉ VIẾT quy trình — cụm `competitor-recon-and-strategy` CHƯA chạy live, chỉ ký shadow)

Đây là checklist Chủ tự làm THEO THỨ TỰ khi quyết định thử live thật (sau khi
cụm `strategy-exec` — contract executor + gas-price/bribe model thật — đã
xong VÀ Chủ tự đọc + đồng ý rủi ro). Claude Code KHÔNG được tự thực hiện bất
kỳ bước nào dưới đây (bật `allow_live`/`bot_armed`/`dry_run=false`, gửi tx
thật) — AGENTS.md cấm tuyệt đối "tự live".

### 9.a Ví mới, nạp nhỏ

1. Tạo 1 ví MỚI HOÀN TOÀN (không dùng lại ví đã từng ký shadow mode nếu ví đó
   từng dùng cho việc khác) — dùng `cast wallet new`/bất kỳ tool tạo ví
   offline nào Chủ tin tưởng, KHÔNG generate qua web ngẫu nhiên.
2. Nạp SỐ TIỀN NHỎ (khuyến nghị: đúng bằng `max_front_bnb` dự kiến dùng ở
   bước 1h thử nghiệm × 2-3 lần, KHÔNG nạp cả `max_exposure_bnb` ngay) — đủ
   cho vài chục tx front/back + gas, không hơn.
3. Dán `PRIVATE_KEY` ví MỚI này vào `.env` trên máy chạy live (khuyến nghị
   VPS, không phải WSL cá nhân) — KHÔNG dùng lại `.env` đã dùng cho shadow
   mode nếu ví shadow đã lộ ra ngoài (log `bundle.shadow` có chứa raw tx đã
   ký — raw tx public-safe không lộ private key, nhưng vẫn nên tách ví cho
   sạch, tránh nhầm lẫn nonce giữa 2 mục đích).

### 9.b Approve router

Router V2 (`venues::V2_ROUTER_ADDRESS`) cần `allowance` để gọi
`swapExactTokensForETHSupportingFeeOnTransferTokens` (back-sell) cho MỖI
token định trade — approve THỦ CÔNG (không phải bot tự approve, executor
hiện tại không có logic approve) từng token trong `pairs.txt` Chủ định bật
live, số lượng approve = `type(uint256).max` hoặc số hữu hạn Chủ tự chọn
(trade-off: max tiện nhưng rủi ro nếu router bị exploit — router Pancake V2
đã hoạt động nhiều năm, rủi ro thấp nhưng không phải 0).

### 9.c Ngưỡng an toàn (chỉnh trong `config.toml` THẬT trước khi bật live)

- `max_front_bnb`: BẰNG hoặc THẤP HƠN số dư ví trừ gas reserve — không đặt
  cao hơn ví thực có.
- `max_exposure_bnb`: trần tổng vốn đang "kẹt" cùng lúc (nhiều candidate
  cùng lúc) — khuyến nghị bắt đầu = `max_front_bnb` (chỉ 1 vị thế 1 lúc).
- `gas_reserve_bnb_wei`: đủ cho ÍT NHẤT 20-30 tx gas (front+back đều tốn gas
  dù revert) — không để cạn gas giữa chừng.
- `max_consecutive_loss`: 2-3 (RiskGuard tự dừng sớm nếu lỗ liên tiếp — xem
  `config.rs::RiskGuard`, ĐÃ có sẵn, chỉ cần `7.x` gọi `record_result` thật
  khi có giao dịch live, xem `docs/TASKS.md` mục nợ).
- `bribe_pct_of_profit`/`bribe_min_bnb`/`bribe_max_bnb`: bắt đầu THẤP hơn số
  mặc định 40%/0.0005/0.01 (ship — mốc thô, xem cụm `competitor-recon-and-strategy`)
  nếu muốn ưu tiên an toàn vốn hơn tỉ lệ thắng vị trí trong block.

### 9.d Shadow 1h → Live 1h vốn 0.05 BNB

1. **Shadow 1 giờ** (`live_mode="shadow"`, `scripts/paper_run.sh --minutes 60
   --live-mode shadow`): xác nhận số `bundle.shadow` hợp lý so với
   `funnel.simulated`, không có `tx.abort{reason:pre_sign_revet_failed}` bất
   thường (nhiều lần liên tiếp = dấu hiệu re-vet quá chặt/RPC không ổn định
   — điều tra TRƯỚC khi qua bước live, không bỏ qua).
2. **Live thử 1 giờ, vốn 0.05 BNB** (`max_front_bnb=0.05`, `max_exposure_bnb=
   0.05`): CHỈ khi cụm `strategy-exec` (contract executor + bribe gửi thật)
   đã xong — bật `dry_run=false`, `allow_live=true`, `bot_armed=true` (Chủ tự
   bật, KHÔNG nhờ Claude Code bật). Theo dõi dashboard SÁT SAO (`/api/status`
   badge `LIVE_ARMED`) trong SUỐT 1 giờ, KHÔNG rời mắt.

### 9.e Tiêu chí dừng (STOP NGAY nếu bất kỳ điều nào xảy ra)

- 2 tx live liên tiếp lỗ (front revert hoặc back revert hoặc profit âm sau
  gas thật) — `touch state/halt.lock` NGAY, không đợi `max_consecutive_loss`
  tự dừng (an toàn kép).
- Gas thật vượt trần cấu hình bất thường (dấu hiệu network tắc nghẽn/relay
  gặp vấn đề).
- Dashboard mất kết nối RPC/WS > 2 phút (không còn nhìn thấy trạng thái thật
  của bot).
- Số dư ví giảm > 30% so với lúc bắt đầu phiên live (dù chưa chạm
  `max_consecutive_loss`).
- BẤT KỲ hành vi nào KHÔNG khớp với những gì đã quan sát ở shadow mode
  (bundle bị relay từ chối hàng loạt, nonce lệch, gas ước tính sai xa thực
  tế).

Sau khi dừng: rút toàn bộ số dư còn lại về ví lạnh, review lại
`logs/bot.jsonl` (`bundle.shadow` so với kết quả live thật) trước khi thử
lại.
