# BAOCAO43 — cụm `decision-data-24h`

## 1. LÁT

`decision-data-24h` — số liệu để Chủ chốt hướng + 4 nợ nhỏ. Gồm:

- **Mục 1** công cụ phân tích chạy trên VPS không cần Rust
  (`scripts/analyze_econ.sh` + `scripts/cluster_funded_scan.sh`) + bảng
  **1a–1d** trên dữ liệu THẬT.
- **Mục 2** `net_pos_non_cluster` ở mọi nơi có `net_pos`; `/api/compete` theo
  pool + "% victim có lãi là ví cụm".
- **Mục 3** nạp `state/pairs_vetted.json` lúc boot (+ sửa BUG file teo dần).
- **Mục 4** `shadow.sim` hỗ trợ quote USDT.
- **Mục 5** `BSC_HTTP_SIM` đọc từ `.env`; phân loại `vet_error`
  `missing trie node` riêng.
- **Mục 6** CLAUDE.md mục Skip: thêm `sanity_reject`, `competitor_victim`.
- **Mục 7** `paper24h.out` — **KHÔNG có `DONE`**: bot VPS đã bị OOM-kill.
- **Việc dính liền cùng phiên** (CLAUDE.md "Cùng phiên — khỏi nợ"): 2 BUG
  THẬT lộ ra từ chính mục 5 — (i) `pairs_vet_task` fork tại block đã cũ khiến
  các pool cuối danh sách KHÔNG BAO GIỜ vet được, (ii) `state/pairs_vetted.json`
  bị ghi đè teo dần. Cả 2 đã sửa + đo lại (0 lỗi vet / 30 phút). Thêm 1 lỗi
  nhỏ trong `scripts/paper_run.sh` do CHỦ chỉ ra giữa phiên (backtick trong
  chuỗi nháy kép) — đã sửa + verify.

## 2. LỆNH NHẬN

Khối lệnh Grok `decision-data-24h`. Máy: WSL (code) + VPS (CHỈ ĐỌC log qua
ssh, key `key/bsc_vps_ed25519`). HEAD bắt đầu lệnh ghi `63e11b5`, HEAD THẬT
lúc mở phiên là `09860fe` (= `63e11b5` + commit BAOCAO42). Không subagent
ghi file (luật #4 — toàn bộ phiên này do phiên chính làm, KHÔNG gọi subagent
nào). Không contract, không live. IP VPS KHÔNG ghi vào file nào trong repo
(kể cả BAOCAO này).

## 3. FILE ĐỔI

Commit: xem dòng `Commit:` cuối file.

- `scripts/analyze_econ.sh` (**MỚI**) — phân tích kinh tế ngoại tuyến, bash +
  jq + awk, chạy được trên VPS không cần toolchain Rust. Output 6 file TSV/TXT.
- `scripts/cluster_funded_scan.sh` (**MỚI**) — dựng danh sách ví cụm đối thủ
  cho 1 khoảng block bằng `eth_getLogs` THẬT (chỉ ĐỌC).
- `src/web.rs` — **mục 2**: `PoolAcc`/`BucketAcc` thêm `net_pos_non_cluster` +
  `sum_*_non_cluster`; `/api/econ` thêm `net_pos_total_non_cluster`,
  `sum_net_bnb_total_non_cluster`, `pct_net_pos_la_vi_cum` (trong `top_pools`
  và `top_pools_by_net`), `summary_line` in cả 2 số; `top_pools_by_net` SẮP
  theo lãi ĐÃ LOẠI CỤM. `/api/compete` thêm `by_pool` + 3 field tổng.
- `src/pairbook.rs` — **mục 3**: `set_vet_result_with_age` (giữ TUỔI THẬT khi
  nạp lại), `vet_snapshot()` (ảnh chụp ĐẦY ĐỦ).
- `src/main.rs` — **mục 3**: `restore_vet_snapshot()` gọi đầu `pairs_vet_task`;
  `pairs_vet_task` ghi `state/pairs_vetted.json` bằng `vet_snapshot()` (sửa
  BUG teo dần) + `pair.vet_cycle_done`. **mục 4**: `spawn_shadow_bundle_sim`
  chạy CẢ quote USDT (bỏ nhánh `skipped`). **mục 5**:
  `transport::load_dotenv_defaults` gọi ở dòng đầu `main()` + log
  `env.dotenv_loaded` (chỉ TÊN biến); phân loại `pair.vet_error{class}`.
- `src/sim_evm.rs` — **mục 4**: `simulate_sandwich_quote`/`run_sandwich_quote`
  (tổng quát theo quote asset; `quote=WBNB` đi ĐÚNG đường code cũ). Test
  `real_rpc_simulate_sandwich_quote_usdt_leg_mechanics` (**chạy THẬT**, output
  ở ô 5).
- `src/transport.rs` — **mục 5**: `classify_vet_error` (4 lớp),
  `load_dotenv_defaults` (không bao giờ log giá trị).
- `scripts/paper_run.sh` — in `pair.vet_restore`, `pair.vet_cycle_done`,
  4 bộ đếm `vet_error` theo lớp, bảng đối chiếu `profit_sim` vs `profit_net`
  (dùng `jq`, KHÔNG dùng Python).
- `web/index.html` + `web/app.js` — 2 cột mới `net_pos KHÔNG cụm` +
  `% net_pos là ví cụm`.
- `CLAUDE.md` — **mục 6** (theo lệnh): bảng Skip thêm `sanity_reject`,
  `competitor_victim` + giải thích + số đo thật.
- `docs/STATE.md`, `docs/TASKS.md`, `docs/RUN.md`, `.env.example` — cập nhật.
- `logs/vps_analysis/*` (7 file, `scp` từ VPS) + `baocao/evidence/
  decision24h_vet_classes.txt`.

**KHÔNG đụng**: `.env` (chỉ đọc), `PRIVATE_KEY`, `pairs.txt`, `config.toml`
(ship giữ nguyên), file nào của bot trên VPS (chỉ ĐỌC log + ghi
`/root/analysis/`), contract, cờ live.

## 4. LỆNH CHẠY

```bash
# --- phan tich (VPS, chi DOC) ---
ssh -i key/bsc_vps_ed25519 root@<VPS>  'pgrep -a bsc_sandwich; dmesg -T | grep -i oom'
./scripts/cluster_funded_scan.sh --from-block 122071909 --to-block 122159273 --out cluster_funded.tsv
scp analyze_econ.sh cluster_funded.tsv root@<VPS>:/root/analysis/
ssh ... 'nice -n 19 ./analyze_econ.sh --log /root/bsc-sandwich/logs/bot.jsonl \
          --from-line 43138 --out /root/analysis/out --cluster-file /root/analysis/cluster_funded.tsv'
scp root@<VPS>:/root/analysis/out/1*.tsv logs/vps_analysis/

# --- code (WSL) ---
cargo build --release && cargo test --release
cargo test --release --lib -- --ignored real_rpc_simulate_sandwich_quote_usdt --nocapture
scripts/paper_run.sh --minutes 30 --port 18940 --live-mode shadow                      # RUN 1
BSC_HTTP_SIM=... scripts/paper_run.sh --minutes 30 --port 18941 --live-mode shadow      # RUN 2
```

## 5. OUTPUT THẬT

**Máy sinh dữ liệu phân tích: VPS** (`/root/bsc-sandwich`, binary sha256
`4af44b29ef1adf729f416d624d18147030bf8d52f0c7f149bfaa0a0143b0feab`, git HEAD
`5284bd376fb78ea0b450249fb2d09d4adfd7f812`).
**Máy chạy code/test/shadow: WSL** (`/home/dmin/bsc-sandwich`), binary sha256
lúc chạy RUN 1+RUN 2 = `e2a070b4e33e98237e2c5cc24615e4e90a71e647df798160a101774f929795ea`.

### 0. PHÁT HIỆN TRƯỚC HẾT — bot paper 24h trên VPS ĐÃ CHẾT (OOM), chạy được 656/1440 phút

```
scripts/paper_run.sh: line 191: 377294 Killed  ./target/release/bsc_sandwich "$CFG"
BOT DA CHET sau 656 phut
```

`dmesg -T` trên VPS:

```
[Wed Sep 16 04:40:50 2026] tokio-rt-worker invoked oom-killer: gfp_mask=0x1100cca ...
oom-kill:constraint=CONSTRAINT_NONE,...,task=bsc_sandwich,pid=377294
Out of memory: Killed process 377294 (bsc_sandwich) total-vm:9532056kB,
  anon-rss:7627256kB, file-rss:0kB, shmem-rss:0kB, UID:0 pgtables:15180kB
```

**7,6 GB RSS / 8 GB RAM sau 656 phút ⇒ ~11 MB/phút.** Binary lúc đó là commit
`5284bd3`, TRƯỚC cả `competitor-recon-and-strategy` lẫn
`bugfix-presign-and-contract-plan` ⇒ rò rỉ CÓ SẴN, không do 2 cụm đó. Chưa
truy nguyên (ghi CÒN NỢ). **Cửa sổ dữ liệu vì vậy là 10,92 h, không phải 24 h**
— mọi số "mỗi ngày" dưới đây là QUY ĐỔI, ghi rõ.

**Xác nhận trạng thái bot VPS sau khi phân tích (ĐẠT CẦN DÁN yêu cầu `pgrep`)**:

```
pgrep -a bsc_sandwich        ->  (rong)   "bot VPS: 0 tien trinh"
```

**Bot đã CHẾT TRƯỚC khi phiên này ssh vào, KHÔNG phải do việc phân tích** —
3 mốc thời gian độc lập chứng minh:

```
dmesg:  [Wed Sep 16 04:40:50 2026] Out of memory: Killed process 377294 (bsc_sandwich)
stat :  /root/bsc-sandwich/logs/bot.jsonl  sua lan cuoi 2026-09-16 04:40:55 UTC
ssh   :  lan ket noi DAU TIEN cua phien nay ~04:57 UTC (sau khi bot chet 16 phut)
```

Việc phân tích chạy `nice -n 19`, chỉ ĐỌC `logs/bot.jsonl`, ghi vào
`/root/analysis/` (đã dọn file trung gian sau khi xong: còn **1,1 MB**,
đĩa `28G available`), KHÔNG đụng file nào của bot.

### 1a/1b/1c/1d — BẢNG SỐ (quan trọng nhất)

`1d_summary.txt` nguyên văn (VPS, `nice -n 19`, dữ liệu 10,92 h):

```
cua so du lieu: 2026-09-15T17:45:20Z  ->  2026-09-16T04:40:47Z
so gio THAT quan sat duoc: 10.92 h
tong: candidate=387409 sim.result=497 net_pos=497 net_pos_non_cluster=16
      (cum doi thu chiem 96.8% so net_pos) rate_rejected=1873

-- (a) pool dat >= 5 net_pos_non_cluster/NGAY --
   0x76c42dda… quote=wbnb net_pos_nc=9 -> 19.8/ngay  lai_nc=0.4546 wbnb  vi_cum 0.0%
   0xd69aeb83… quote=usdt net_pos_nc=5 -> 11.0/ngay  lai_nc=0.0069 usdt  vi_cum 0.0%

-- (b) quote WBNB co pool nao co lai khong --
   0x76c42dda… net_pos_nc=9 lai=0.454624 BNB
   0x6c6636ea… net_pos_nc=1 lai=0.025364 BNB

-- (c) gio tap trung (top 5 theo net_pos_non_cluster) --
   UTC 01 (VN 08): net_pos_nc=7  candidate=35991  usdt=1.63  wbnb=0.025364
   UTC 04 (VN 11): net_pos_nc=3  candidate=26824  usdt=0.00  wbnb=0.350958
   UTC 00 (VN 07): net_pos_nc=2  candidate=39581  usdt=0.00  wbnb=0.039122
   UTC 19 (VN 02): net_pos_nc=2  candidate=36669  usdt=0.00  wbnb=0.026322
   UTC 02 (VN 09): net_pos_nc=1  candidate=31462  usdt=0.00  wbnb=0.022952

-- (d) von can de lay 80% lai (chi dong KHONG thuoc cum) --
   quote=usdt: 6 co hoi, tong lai 1.6262 -> can von 2999.9950 usdt (100% lai)
   quote=wbnb: 10 co hoi, tong lai 0.4800 -> can von 4.9950 wbnb (100% lai)
```

**1b — theo quote** (`logs/vps_analysis/1b_by_quote.tsv`):

```
quote  candidate  cand_cum  sim.result  net_pos  net_pos_NC  sum_net  sum_NC  best     rate_rejected
usdt   33425      1837      487         487      6           22896.73 1.6262  219.55   1873
wbnb   353984     484       10          10       10          0.4800   0.4800  0.1187   0
```

**1b — top pool theo `net_pos_non_cluster`** (`1b_top_pools.tsv`):

```
pool        token        quote  candidate  net_pos  NC   lai_NC    %net_pos la vi cum  compete_touched
0x76c42dda… BORT         wbnb   59         9        9    0.454624  0.0                 1
0xd69aeb83… POP          usdt   557        5        5    0.006925  0.0                 0
0x3f803ec2… BTCB         usdt   45         1        1    1.619323  0.0                 0
0x6c6636ea… CATE         wbnb   1          1        1    0.025364  0.0                 0
0xcec13213… BinanceTown  usdt   551        236      0    0.000000  100.0               1
0xdfe23efb… BNC          usdt   540        245      0    0.000000  100.0               1
```

**1a — vốn cần theo pool** (`1a_front_p80.tsv`, p80 `front_in` trên dòng CÓ LÃI):

```
pool        quote  n_profit  front_in_p80  front_in_max
0x76c42dda… wbnb   9         4.995000      4.995000     (= tran max_front_bnb=5)
0xd69aeb83… usdt   5         4.095382      4.096207
0x3f803ec2… usdt   1         2999.995000   2999.995000  (= tran max_front_usdt=3000)
0xdfe23efb… usdt   245       2999.995000   2999.995000
0xcec13213… usdt   236       2999.995000   2999.995000
```

**1c — theo giờ** (`1c_by_hour.tsv`, cột `hour_vn = UTC+7`):

```
UTC VN  candidate  net_pos  net_pos_NC  sum_NC_usdt  sum_NC_wbnb
00  07  39581      53       2           0.0000       0.039122
01  08  35991      52       7           1.6262       0.025364
02  09  31462      42       1           0.0000       0.022952
03  10  44514      40       0           0.0000       0.000000
04  11  26824      24       3           0.0000       0.350958
17  00   9197      15       0           0.0000       0.000000
18  01  26038      34       0           0.0000       0.000000
19  02  36669      60       2           0.0000       0.026322
20  03  34101      33       0           0.0000       0.000000
21  04  34003      30       0           0.0000       0.000000
22  05  33728      55       0           0.0000       0.000000
23  06  35301      59       1           0.0000       0.015270
```

**Kiểm chéo định nghĩa "thuộc cụm"** (chống cáo buộc lọc quá tay): tính bằng
2 cách cho ra ĐÚNG CÙNG 1 số **481/497**:

```
net_pos=497  ever_cluster=481  strict_window(±2 block)=481  co_field_flag=0 (binary VPS chua co field)
```

`cluster_funded_scan.sh` (RPC THẬT, `eth_chainId=0x38` kiểm trước):
`44 call, 0 loi, 13.119 lan cap von, 824 vi rieng biet` trên
`122071909..122159273`.

### 2. `cargo test --release`

```
test result: ok. 384 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

399 pass (384 lib + 15 main), tăng từ 393 (BAOCAO42) = **+6 test**, 0 failed;
`ignored` 14 → **16** (+2 test `real_rpc_*`, cả 2 ĐÃ CHẠY THẬT, output dưới).
Test mới: 3 `transport` (dotenv × 2, `classify_vet_error`), 2 `pairbook`
(`set_vet_result_with_age`, `vet_snapshot`), 1 `web`
(`compute_econ_tach_net_pos_non_cluster_moi_cho_co_net_pos`), + 2 `real_rpc_*`
(`…usdt_leg_mechanics`, `…which_node_can_vet_the_two_hot_usdt_pools`).

### 3. Mục 4 — `real_rpc_simulate_sandwich_quote_usdt_leg_mechanics` (CHẠY THẬT, WSL)

```
real_rpc_simulate_sandwich_quote_usdt THAT: fork_block=122165721
  token=0x01fbed06a70eb1b4a0b43dbc6a94f99944cd7777 quote=USDT
  front_in=1000000000000000000000 USDT  token_received=2128336108169723607196448
  back_out=997694480649108767504 USDT  profit=-2305519350891232496 (don vi USDT wei)
  victim_success=true  buy_tax_bps=Some(0) sell_tax_bps=Some(0)
  round-trip loss = 23 bps (phi pool ~50 bps TRU phan victim bu lai; victim_success=true)
test sim_evm::tests::real_rpc_simulate_sandwich_quote_usdt_leg_mechanics ... ok
```

Cả 3 chân (front USDT → token, victim replay, back token → USDT) chạy THẬT
trên state chain thật. Trước cụm này nhánh USDT không tồn tại.

### 3b. BUG GỐC tìm được KHI làm mục 5 — `pairs_vet_task` fork tại block ĐÃ CŨ

Test `real_rpc_which_node_can_vet_the_two_hot_usdt_pools` (CHẠY THẬT, WSL):

```
real_rpc_which_node_can_vet: 2 URL, 2 pool USDT nong nhat
  bsc.blockrazor.xyz        BNC          block=122168084 OK  buy_bps=0 sell_bps=0 honeypot=false
  bsc.blockrazor.xyz        BinanceTown  block=122168084 OK  buy_bps=0 sell_bps=0 honeypot=false
  bsc-dataseed1.defibit.io  BNC          block=122168111 OK  buy_bps=0 sell_bps=0 honeypot=false
  bsc-dataseed1.defibit.io  BinanceTown  block=122168111 OK  buy_bps=0 sell_bps=0 honeypot=false
=> co it nhat 1 node vet duoc 2 pool nay: true
  do sau: bsc.blockrazor.xyz block=head-0   (122168127) OK buy=0 sell=0
  do sau: bsc.blockrazor.xyz block=head-2   (122168125) OK buy=0 sell=0
  do sau: bsc.blockrazor.xyz block=head-10  (122168117) OK buy=0 sell=0
  do sau: bsc.blockrazor.xyz block=head-50  (122168077) OK buy=0 sell=0
  do sau: bsc.blockrazor.xyz block=head-200 (122167927) LOI[unsupported_method] "doc storage slot 0 that bai: -32000 not supported"
```

⇒ Token/pool KHÔNG có vấn đề, node cũng vet được — chỉ hỏng khi fork ở block
QUÁ CŨ. Nguyên nhân: `pairs_vet_task` đọc block MỘT LẦN đầu vòng rồi dùng cho
cả 126 pool; vòng chạy tuần tự mất 3–4 phút mà BSC ~2,2 block/s ⇒ cuối vòng
block đã cũ 400–500 block (vượt cửa sổ state ~128 block của node full). Vì thứ
tự lặp ổn định, LUÔN là những pool cuối danh sách hỏng ⇒ không bao giờ vet
được ⇒ không bao giờ ký được. Sửa 2 bước (bước 2 lộ ra ngay khi chạy lại bước
1, 37 dòng `khong tim thay block N`): đọc lại block mỗi vòng lặp, và hỏi CHÍNH
NODE SIM (`eth_blockNumber`) chứ không dùng đỉnh của pool RPC đường nóng.
Chi tiết `docs/STATE.md` mục 6b.

### 4. Mục 3 + 5 — `pair.vet_restore` / `pair.vet_cycle_done` (2 lần chạy WSL)

```
=== RUN 1 (port 18940, KHONG dat BSC_HTTP_SIM — 5 URL public mac dinh) ===
{"event":"pair.vet_restore","restored":24,"skipped":0,"max_age_sec":4127,
 "vet_max_age_sec":1200,"still_fresh":false}
{"event":"pair.vet_cycle_done","due":105,"measured":16,"error_pools":89,
 "errors_by_class":{"missing_trie_node":89},"never_vetted_pools":88}
{"event":"pair.vet_cycle_done","due":110,"measured":25,"error_pools":85,
 "errors_by_class":{"rate_limited":1,"unsupported_method":84},"never_vetted_pools":70}

=== RUN 2 (port 18941, BSC_HTTP_SIM=bsc.blockrazor.xyz,bsc-dataseed1.bnbchain.org) ===
{"event":"pair.vet_restore","restored":56,...}          <-- 24 -> 56: BUG "file teo dan" DA SUA
{"event":"pair.vet_cycle_done","due":86,"measured":10,"error_pools":76,
 "errors_by_class":{"unsupported_method":76},"never_vetted_pools":60}
{"event":"rpc.connect","transport":"sim","pool_index":0,"url":"https://bsc.blockrazor.xyz/***"}
```

- **`restored` 24 → 56**: file `state/pairs_vetted.json` trước cụm này bị ghi
  đè bằng ĐÚNG pool vet trong vòng hiện tại nên teo dần (đầu phiên chỉ còn 24
  dòng / 126 pool); sau khi ghi ảnh chụp đầy đủ, lần chạy kế nạp được 56 và
  file hiện có **66** pool.
- **`still_fresh:false` là ĐÚNG**: snapshot 4127 s > `vet_max_age_sec=1200`,
  nạp lại KHÔNG nới cổng an toàn — vẫn `vet_stale` như khi không nạp.
- **Số pool KHÔNG vet được**: `missing_trie_node=89/105` trên RPC public mặc
  định — 89 pool này KHÔNG BAO GIỜ ký được trên node đó. Đây chính là con số
  mục 5 yêu cầu, và là lý do `BSC_HTTP_SIM` cần node riêng.

### 5. `/api/econ` + `/api/compete` — field mới CHẠY THẬT (RUN 2, port 18941)

```
/api/econ  buckets_bnb[">=1"] = {"count":16,"net_pos":1,"net_pos_non_cluster":1,
   "sum_net_pos_bnb":0.0525883,"sum_net_pos_bnb_non_cluster":0.0525883,...}
/api/compete by_pool = [{"pair":"0xcec13213c390d51121f82ba2ecafb8e11e0af7a3",
   "net_pos":1,"net_pos_non_cluster":1,"pct_net_pos_la_vi_cum":0.0,
   "sum_net_bnb":0.0525883,"sum_net_bnb_non_cluster":0.0525883,"competitor_touched":true}]
   net_pos_total=1  net_pos_total_non_cluster=1  pct_net_pos_la_vi_cum=0.0
```

### 6. Shadow 30 phút (RUN 2) — xem ô 10 nếu thiếu

**RUN 4** (WSL, port 18943, `--live-mode shadow`, 30 phút, binary sha256
`7eb2ddee19b095f4f627de3dee008e821d98def5b91fc0e78fbebe7ddd22861f` — bản đã
sửa đủ 2 bước bug fork-block ở mục 6b, `BSC_HTTP_SIM` trỏ node giữ state).
File đầy đủ: `baocao/evidence/decision24h_shadow30_run4.txt`.

**(a) Vet: 0 lỗi** (trước sửa: 174 lỗi / RUN 1):

```
{"event":"pair.vet_restore","restored":86,"skipped":0,"max_age_sec":5790,
 "vet_max_age_sec":1200,"still_fresh":false}
{"event":"pair.vet_cycle_done","due":16,"measured":16,"error_pools":0,
 "errors_by_class":{},"never_vetted_pools":0}
vet_error missing_trie_node = 0
vet_error rate_limited      = 0
vet_error unsupported_method= 0
vet_error other             = 0
```

`restored` 24 (đầu phiên) → 66 → **86** pool: bug "file teo dần" đã hết.

**(b) ĐỐI CHIẾU `profit_sim` vs `profit_net` — 4/4 bundle, TẤT CẢ quote USDT**
(trước cụm này: 0/9, toàn bộ `skipped:"usdt_not_supported…"`):

```
---- doi chieu profit_sim (revm) vs profit_net (V2-math) theo tung victim ----
  0x4c29f855f68f9507 quote=usdt  profit_sim=-14.48961  victim_ok=false  tax_buy/sell=0/0
  0x6bea03d7c52d6a41 quote=usdt  profit_sim=-14.48945  victim_ok=false  tax_buy/sell=0/0
  0x327cb2655553fa59 quote=usdt  profit_sim=-13.98576  victim_ok=false  tax_buy/sell=0/0
  0x71d9906b64111df4 quote=usdt  profit_sim=-14.50876  victim_ok=false  tax_buy/sell=0/0
  profit_net tuong ung (bundle.shadow_econ):
  0x4c29f855f68f9507  profit_net= 37.159 usdt  bribe=14.864
  0x6bea03d7c52d6a41  profit_net= 54.921 usdt  bribe=21.968
  0x327cb2655553fa59  profit_net=100.645 usdt  bribe=40.258
  0x71d9906b64111df4  profit_net= 47.637 usdt  bribe=19.055
```

**ĐÂY LÀ SỐ QUAN TRỌNG NHẤT CỦA CẢ CỤM, CHỈ SAU BẢNG 1a–1d**: đường nóng
V2-math nói **+37 … +101 USDT**, EVM replay nói **−14 USDT** trên CẢ 4 mẫu, vì
`victim_ok=false` (victim không thực thi được) ⇒ không có sandwich, chỉ còn
round-trip mất phí pool (−14,5/3000 = **−48 bps**, khớp phí 2 × 25 bps —
chứng tỏ kế toán nhánh USDT đúng). **Chưa phân định được** vì sao victim hỏng
(2 giả thuyết + cách phân định 1 dòng ghi ở `docs/STATE.md` mục 5b) — ghi
MISSING, KHÔNG suy diễn.

**(c) `/api/econ` cuối RUN 4:**

```
candidate=13642 net_pos=11 net_pos_non_cluster=9 best_net_bnb=0.183115
   p50_ms=0.01 p95_ms=752.15 stale_pct=0.00 decode_fail_smartrouter=395
sum_net_bnb_total_non_cluster=0.7398   rate_inverted_rejected=0   rate_unavailable=74
top_pools_by_net (SAP THEO LAI DA LOAI CUM):
  0xdfe23efb… net_pos=5 net_pos_non_cluster=4 pct_la_vi_cum=20.0% sum_nc=0.4563 BNB
  0xcec13213… net_pos=3 net_pos_non_cluster=3 pct_la_vi_cum= 0.0% sum_nc=0.1695 BNB
  0xf867ca53… net_pos=3 net_pos_non_cluster=2 pct_la_vi_cum=33.3% sum_nc=0.1140 BNB
/api/skips: competitor_victim=6  sanity_reject=0  victim_would_revert=72
            decode_fail=5173  unprofitable=661
```

**Lưu ý trung thực 2 điểm:**

1. **p95 `seen_to_decision` = 752 ms**, xấu hơn hẳn mốc BAOCAO40 (321 ms) và
   BAOCAO42 (344 ms). Nghi vấn trực tiếp: `pairs_vet_task` nay gọi thêm 1
   `eth_blockNumber` mỗi pool (mục 6b) và node sim (`bsc.blockrazor.xyz`) chậm
   hơn — nhưng CHƯA đo tách bạch, ghi CÒN NỢ, không đổ lỗi bừa.
2. RUN 4 chạy bằng `scripts/paper_run.sh` **TRƯỚC** khi Chủ chỉ ra lỗi
   backtick, nên trong file evidence còn 2 dòng
   `./scripts/paper_run.sh: line 286: decision-data-24h: command not found`.
   Lỗi này CHỈ ở phần in báo cáo (không ảnh hưởng số liệu), đã sửa, và verify
   lại bằng chính 2 dòng đó sau khi sửa:

```
---- cum 'decision-data-24h' muc 3: pair.vet_restore (nap lai snapshot vet luc boot) ----
---- cum 'decision-data-24h' muc 5: pair.vet_cycle_done (loi vet theo LOAI + so pool chua vet duoc) ----
(khong con 'command not found')
grep -n '`' scripts/paper_run.sh  ->  chi con trong dong comment '#'
```

### 7. `git status --short` SAU commit + `git log -1`

```
eb1f7a84047901818d1f7bd22d4ee7e19132a04c 2026-09-16 13:29:48 +0700
(git status --short SAU commit nay: chi con chinh file BAOCAO43.md dang duoc
viet - commit bo sung ngay sau, hash ghi o dong Commit cuoi file)
```

Binary sha256 CUOI PHIEN (sau moi thay doi, `cargo build --release`):
`2c2f85d93c7d37f97ed0034ce5bbc6c95712742e7cb01367f82c621879387954`
(RUN 4 chay bang ban `7eb2ddee…` — chenh lech duy nhat la 1 sua khoa
`restore_vet_snapshot` khong `.await` len khoa `config` khi dang giu khoa ghi
`pairbook`, khong doi hanh vi do duoc).

## 6. CHAIN — `0x38`

- `cluster_funded_scan.sh` kiểm `eth_chainId` **trước** mọi `eth_getLogs`, trả
  `0x38`, dừng ngay nếu khác (code: `[[ "$CHAIN" == "0x38" ]] || exit 1`).
- `real_rpc_simulate_sandwich_quote_usdt_leg_mechanics`:
  `assert_eq!(chain_id, 56)` — fork block THẬT `122165721`, đọc state THẬT của
  pool BNC/USDT `0xdfe23efb…` và token `0x01fbed06…`.
- Bot RUN 1/RUN 2: `chain_id=56 dry_run=true allow_live=false bot_armed=false`.
- Pin V2/V3/V4/WBNB/USDT KHÔNG đổi ở cụm này.

## 7. REGISTRY

`DEX_REGISTRY.md` **KHÔNG đổi** — cụm này không pin venue/địa chỉ mới.

## 8. KHÔNG LÀM

- Không viết/deploy contract, không gửi tx/bundle, không bật live.
- Không đụng `pairs.txt`, `.env`, `config.toml` ship, `PRIVATE_KEY`.
- Không khởi động lại bot trên VPS sau khi phát hiện nó đã chết — nó tự chết
  vì OOM, và khởi động lại sẽ lại OOM sau ~11 h; **đây là quyết định của Chủ**
  (xem ô 10).
- Không dùng subagent (luật #4), không dùng Python runtime trong
  `scripts/paper_run.sh` (dùng `jq`).
- Không kết luận chiến lược thay Chủ.

## 9. CHỮ: CHỜ GROK

## 10. CÒN NỢ / LÁT SAU

1. **RÒ RỈ BỘ NHỚ → OOM (chặn chạy 24 h)** — VPS 8 GB bị kill sau 656 phút,
   `anon-rss 7,6 GB`, ~11 MB/phút. Chưa truy nguyên. **Bot trên VPS hiện đang
   DỪNG** — phiên này KHÔNG tự khởi động lại (chạy lại sẽ lại OOM sau ~11 h);
   cần Chủ quyết: (a) sửa rò rỉ trước, (b) chạy nhiều phiên ngắn 4–6 h, hoặc
   (c) thêm `MemoryMax=` + `Restart=always` vào unit systemd.
2. **`victim_ok=false` trên 4/4 bundle shadow — CHƯA phân định nguyên nhân.**
   Đây là câu hỏi số 1 cho cụm sau, vì nó quyết định mọi con số lãi trong repo
   có thật hay không. Cách phân định (1 dòng code): khi `victim_ok=false`,
   chạy lại `simulate_sandwich_quote` với `front_in=0` trên CÙNG fork — victim
   vẫn hỏng ⇒ do state ví victim; victim sống ⇒ **chân front của ta đang giết
   victim** và phải hạ `front_in`/sửa gate `victim_would_revert`.
3. **p95 `seen_to_decision` 752 ms** (BAOCAO40: 321, BAOCAO42: 344) — chưa đo
   tách bạch nguyên nhân (nghi `eth_blockNumber` mỗi pool trong vet task +
   node sim chậm).
4. **Tỉ lệ THẮNG cuộc đua: vẫn MISSING** — mọi con số lãi đều là mô phỏng.
5. **Mục 7 của lệnh KHÔNG THỂ ĐẠT**: `paper24h.out` không bao giờ có `DONE` vì
   bot bị OOM-kill. Đã `scp` file đó về (`baocao/evidence/decision24h_vps/
   paper24h_vps.out` + `logs/vps_analysis/`) và tóm tắt ở ô 5 mục 0.
6. **`victim_in_competitor_cluster` chỉ có ở binary từ
   `bugfix-presign-and-contract-plan`** — log cũ phải dựng lại cụm bằng
   `scripts/cluster_funded_scan.sh` (cần RPC, ~2 phút cho 87k block).
7. **Bảng 1a `front_in_p80` tính theo pool × quote, KHÔNG theo giờ** — mỗi ô
   pool×quote×giờ chỉ có 1–3 mẫu, p80 theo giờ sẽ vô nghĩa. Ghi rõ thay vì
   trình bày số không có ý nghĩa thống kê.
8. **Đường `sim_engine="evm"` vẫn dùng trần gas cấu hình** (nợ cũ từ
   `real-economics-mode2`), không đụng ở cụm này.

Commit: `eb1f7a84047901818d1f7bd22d4ee7e19132a04c` (+ commit bo sung cho chinh file nay)
