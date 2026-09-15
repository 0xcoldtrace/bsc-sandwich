# BSC Sandwich Bot

Bot quan sát mempool BSC (chain 56), tìm nạn nhân swap token/WBNB trên các family
PancakeSwap (V2/V3/V4-Infinity/mới hơn khi đã pin), mô phỏng sandwich, mặc định
**dry-run** (không gửi tx thật).

Stack: Rust + tokio. RPC crate: **alloy** (xem `docs/STATE.md`).

## Chạy

```
cp victims.example.txt victims.txt   # đã có sẵn, sửa theo ví thật
cp .env.example .env                  # điền RPC/khóa nếu cần (chưa dùng ở giai đoạn này)
cargo run
```

Web dashboard mặc định tại `http://127.0.0.1:8787` (chỉ đọc, không gửi tx).

## Chỉnh ngưỡng sau này (không đụng code)

Sửa `config.toml` (`min_profit_bnb`, `max_front_bnb`, `min_reserve_wbnb`,
`max_roundtrip_tax`, ...) hoặc `victims.txt` rồi lưu file — KHÔNG cần sửa
Rust, không cần build lại. Bot tự đọc lại `config.toml` mỗi tối đa
`config_reload_sec` giây và `victims.txt` mỗi `victims_reload_sec` giây (cả
hai ship mặc định 15s). Sửa sai định dạng (thiếu field, số âm) thì bot GIỮ
config/victims cũ + log lỗi, không crash — sửa lại đúng là tự áp dụng ở lần
đọc kế tiếp, không cần restart `cargo run`.

## Test

```
cargo test
```

## Vet `pairs.txt` (mode 2, pair-mode) — lọc thô + chạy paper

Chiến lược đã chốt (`CLAUDE.md` mục "Chiến lược đã chốt"): CHỈ mode 2
(`pairs.txt`, token do Chủ vet tay) đang bật. Bước lọc thô trước khi Chủ tự
soát tay + điền `vetted YYYY-MM-DD`:

```
scripts/vet_goplus.sh pairs.txt   # can curl + jq, in bang PASS/REVIEW/FAIL
```

Chạy paper 30 phút (thu funnel/skip/tax/validate) xem `docs/RUN.md`.

## Đo RPC (`rpc_probe`) — PHẢI chạy trên VPS, không phải máy dev

Máy dev (VN) đo RTT tới RPC US/Cluster sẽ RA SỐ SAI (cộng thêm ~150-250ms
xuyên lục địa) — số đó KHÔNG dùng được để so sánh/chọn thứ tự failover thật.
`vps.json` ghi `region_hint` cho VPS vận hành thật (New Jersey, US) vì phần
lớn RPC BSC công khai/trả phí đặt cụm ở US — **mọi phép đo RTT dùng để ra
quyết định (chọn thứ tự URL trong `.env`) phải chạy TRÊN VPS đó**, không phải
trên máy dev đang gõ lệnh.

```
# Trên VPS (SSH vào VPS bằng thông tin đăng nhập của bạn — KHÔNG lưu SSH
# user/mật khẩu trong bất kỳ file nào của repo này):
git clone <repo-url> && cd bsc-sandwich   # hoặc git pull nếu đã có sẵn
cp .env.example .env && nano .env         # tự điền BSC_HTTP/BSC_WS thật
bash scripts/run_rpc_probe.sh             # đọc .env, KHÔNG in ra man hinh
```

Kết quả in ra bảng `host redact / rtt min,p50,p95 (ms) / chain_ok / err` +
ghi `artifacts/rpc_probe.json` (đã redact, gitignored — không commit). Máy
Windows/dev dùng `scripts/run_rpc_probe.ps1` (cùng logic, chỉ để kiểm tra code
chạy được — số đo từ máy dev KHÔNG đại diện cho vị trí VPS thật).

## Deploy nhanh lên VPS (`scripts/deploy_vps.sh` / `.ps1`)

Máy Windows không có sẵn OpenSSH Client gốc (kiểm tra: `Get-Command ssh`) —
cài bằng PowerShell **quyền Administrator**: `Add-WindowsCapability -Online
-Name OpenSSH.Client~~~~0.0.1.0`, hoặc cài **Git for Windows** (đã kèm sẵn
`ssh.exe`/`scp.exe` trong `Git\usr\bin`, `deploy_vps.ps1` tự dò đường dẫn
này nếu PATH không có), hoặc dùng **WinSCP** (GUI) để copy tay rồi SSH chạy
lệnh thủ công như 2 mục trên. Script chỉ dùng SSH key/`ssh-agent` — **không
bao giờ** truyền/lưu mật khẩu qua tham số.

```
# Linux/macOS/git-bash (đã có SSH key trỏ VPS):
scripts/deploy_vps.sh --host <ip_vps> --user root --identity ~/.ssh/id_vps --probe --build --run

# PowerShell Windows:
.\scripts\deploy_vps.ps1 -VpsHost <ip_vps> -VpsUser root -Identity C:\path\id_vps -Probe -Build -Run
```

Script tự: kiểm tra SSH, cài `rustup` trên VPS nếu thiếu, đóng gói source
(loại `.git/target/state/logs/artifacts/.env`) rồi copy sang
`/root/bsc-sandwich`, sau đó tuỳ cờ chạy `run_rpc_probe.sh` / `cargo build
--release` / bot nền (`nohup`, vẫn `dry_run=true` theo `config.toml` đã
copy). Xem dashboard qua tunnel (không mở port public):
`ssh -N -L 8787:127.0.0.1:8787 -p <port> root@<ip_vps>`.

## Chạy bot trên VPS + xem dashboard từ xa

```
cargo build --release
./target/release/bsc_sandwich    # bind mặc định 127.0.0.1:8787, KHÔNG public 0.0.0.0
```

Dashboard chỉ bind loopback trên VPS theo đúng CLAUDE.md (không đổi
`web_bind` sang `0.0.0.0` trừ khi bạn tự chịu trách nhiệm mở firewall) — xem
từ máy khác qua SSH local port forward (tunnel), KHÔNG mở cổng ra Internet:

```
ssh -N -L 8787:127.0.0.1:8787 <user>@<vps-ip>
# rồi mở http://127.0.0.1:8787 trên máy của bạn
```

## Quy tắc vận hành

Xem `CLAUDE.md` — luật đầy đủ, không sửa file đó trừ khi được lệnh.
Trạng thái pin venue: `DEX_REGISTRY.md`. Quyết định kỹ thuật: `docs/STATE.md`.
Việc còn lại: `docs/TASKS.md`. Bản đồ tài liệu: `docs/DOC_MAP.md`.
Báo cáo từng phiên: `baocao/BAOCAO{NN}.md`.
