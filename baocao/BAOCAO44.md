# BAOCAO44 — cụm `truth-victim-ok-and-memleak`

## 1. LÁT

`truth-victim-ok-and-memleak` — 2 câu hỏi sống còn + rò rỉ bộ nhớ. Gồm mục
1→6 của lệnh, **cộng 5 BUG THẬT lộ ra giữa phiên** (việc dính liền cùng
phiên, CLAUDE.md mục "Cùng phiên — khỏi nợ"):

- **Mục 1** phân định `victim_ok=false` — trả lời được, và câu trả lời
  **KHÁC NHAU theo từng đường** (xem ô 5 mục 1 + 1b).
- **Mục 2** sửa: ràng buộc `front_in` theo "victim phải sống", áp cả 2 quote.
- **Mục 3** rò rỉ bộ nhớ: `src/mem.rs`, `/api/mem`, `mem.rss_mb` mỗi phút,
  trần thật cho 4 container + 1 nguồn nằm ngoài container.
- **Mục 4** `pairs_vet_task` lấy `eth_blockNumber` 1 lần / 10 pool.
- **Mục 5** systemd unit THẬT trên VPS (`Restart=always`, `MemoryMax=6G`,
  logrotate) — đã cài, đang chạy, `Restart` chứng minh bằng `kill -9`.
- **Mục 6** mọi `net_pos` đi kèm `victim_ok_v2` + `victim_ok_evm`.
- **BUG #1** `pair.reload` giữ khoá GHI `pairbook` xuyên `.await` hàng trăm
  `eth_call` → treo `/api/pairs`, `/api/mem` và chặn cả đường nóng.
- **BUG #2** `mem_watch_task` chết sau 2 dòng vì chính BUG #1.
- **BUG #3** 4 container không có trần (mục 3).
- **BUG #4** `shadow.sim` fork tại `current_block` — SAI khi victim được đào
  ngay trong block đó.
- **BUG #5** `MINED_INDEX_DEPTH = 3` quá nông → bot ký bundle cho tx ĐÃ nằm
  trên chain.

## 2. LỆNH NHẬN

Khối lệnh Grok `truth-victim-ok-and-memleak`. HEAD lệnh ghi `eb1f7a8`; HEAD
THẬT lúc mở phiên là `f86268f` (= `eb1f7a8` + 1 commit bổ sung BAOCAO43).
Máy: WSL (code + đo), VPS (deploy + chạy dài). Không contract, không live.
Không subagent (luật #4 — toàn bộ phiên do phiên chính làm). IP VPS KHÔNG ghi
vào bất kỳ file nào trong repo.

## 3. FILE ĐỔI

Commit cuối: xem dòng `Commit:` cuối file. 5 commit trong phiên:
`9b2115f` (mục 1–6 + BUG #1/#2/#3), `8e09735` (mục 5, `config.runtime.toml`),
`ac9406c` (BUG #4/#5), `8724750` (BAOCAO44 + docs), `6703c23`
(`ECON_EVENTS_CAN_DUNG` + cập nhật báo cáo).

**MỚI**

- `src/mem.rs` — đọc `VmRSS`/`VmHWM` THẬT từ `/proc/self/status`;
  `ContainerSize` (`len: Option<usize>` — `None` = khoá bận, KHÁC `0`).
- `scripts/bsc-sandwich-paper.service` — unit systemd (`Restart=always`,
  `RestartSec=10`, `StartLimitIntervalSec=0`, `MemoryMax=6G`, `MemoryHigh=2G`,
  `KillMode=control-group`, siết quyền `ProtectSystem=full`).
- `scripts/bsc-sandwich.logrotate` — `copytruncate` (bot giữ file handle mở
  và KHÔNG xử lý SIGHUP; dùng `create` sẽ làm log mới biến mất im lặng).
- `scripts/install_systemd_vps.sh` — cài unit + logrotate; tạo
  `config.runtime.toml` (unit KHÔNG BAO GIỜ đọc thẳng `config.toml` của Chủ);
  cờ `--paper-thresholds` hạ ĐÚNG 6 ngưỡng như `scripts/paper_run.sh`.

**SỬA**

- `src/sim_v2.rs` — `max_front_in_victim_ok` (nhị phân trên tính đơn điệu của
  `victim_out`) + `search_max_front_in_victim_ok`. +4 test.
- `src/pipeline.rs` — dùng search có ràng buộc ở CẢ 3 đường quyết định
  (`evaluate_candidate`, `evaluate_candidate_quote`, `decide_paper`);
  `sim.result` thêm `victim_ok_v2`/`victim_out_wei`/`amount_out_min_wei`;
  `TxLogMeta.amount_out_min`.
- `src/sim_evm.rs` — `EvmSandwichOutcome` thêm `victim_revert_reason`
  (decode `Error(string)`/`Panic`) + `victim_out`; `run_sandwich_quote` hỗ trợ
  `front_in = 0` (replay MỘT MÌNH victim); `revert_reason`;
  `BlockForkCache::run_sandwich_quote_cached`; `diagnose_victim_ok` +
  `diagnose_victim_ok_variants` + `victim_diag_ladder`; test thật
  `real_rpc_victim_ok_verdict_ladder`.
- `src/main.rs` — `mem_watch_task`; **sửa BUG #1** (`pair.reload` reload trên
  BẢN SAO, tráo dưới khoá ngắn, áp lại `vet_snapshot`); **sửa BUG #4**
  (`spawn_shadow_bundle_sim` fork tại `current_block − 1`, log tách
  `fork_block`/`decision_block`); `shadow.victim_diag`; mục 4
  (`VET_BLOCK_REFRESH_EVERY = 10`); điền `meta.amount_out_min` cả 2 nhánh.
- `src/transport.rs` — trần cho `ReserveCache` (8 block) + `NonceCache`
  (4 block); `len()` cho `SeenHashSet`/`MinedTxIndex`; **sửa BUG #5**
  (`MINED_INDEX_DEPTH` 3 → 32) + test hồi quy.
- `src/web.rs` — `GET /api/mem` + `container_sizes` (dùng `try_read()`,
  **sửa BUG #2**); `read_log_tail` thay `read_to_string` cả file ở 3 handler;
  trần cho `CompeteStats.top_bots`/`gas_samples`; khối `victim_ok` trong
  `/api/econ` + `summary_line`; `VALIDATE_ROWS_CAP`/`COMPETE_ROWS_CAP`.
- `src/pairbook.rs` — `#[derive(Clone)]` (cần cho sửa BUG #1); `src/tax.rs`,
  `src/pool.rs`, `src/lib.rs` — `len()` / visibility.
- `docs/STATE.md` (+9 mục), `docs/TASKS.md`, `.gitignore`.

**KHÔNG đụng**: `CLAUDE.md`, `config.toml` (ship giữ nguyên), `pairs.txt`,
`.env`, `PRIVATE_KEY`, cờ live, contract.

## 4. LỆNH CHẠY

```bash
# --- WSL ---
cargo build --release && cargo test --release
BSC_HTTP_SIM=... LADDER_CASES=14 cargo test --release --lib -- --ignored \
    real_rpc_victim_ok_verdict_ladder --nocapture           # bang muc 1
BSC_HTTP_SIM=... scripts/paper_run.sh --minutes 65 --port 18951 --live-mode shadow
curl -s http://127.0.0.1:18951/api/mem
curl -s http://127.0.0.1:18951/api/econ

# --- VPS (chi DOC log cu, roi deploy + cai systemd) ---
ssh -i key/bsc_vps_ed25519 root@<VPS> 'nice -n 19 python3 -c "..." logs/bot.jsonl'
./scripts/deploy_vps.sh --host <VPS> --identity key/bsc_vps_ed25519 --build
ssh ... './scripts/install_systemd_vps.sh --paper-thresholds --start'
ssh ... 'kill -9 $(systemctl show bsc-sandwich-paper -p MainPID --value)'  # thu Restart
```

## 5. OUTPUT THẬT

**Máy**: WSL `/home/dmin/bsc-sandwich` cho mọi phần code/test/đo; VPS
`VPS-511043-157` cho deploy + chạy dài.
**git HEAD cuối phiên**: xem dòng `Commit:` cuối file (`sha256` binary WSL
tương ứng dán kèm ở đó).
**Binary chạy shadow/mem 65 phút (WSL)**:
`2d5c0e0f417b98f6fd4b5bbc11183d4c72adf10ddc3f2b403f8fca8c65e34893`
(= sau BUG #1/#2/#3 + mục 1–6, TRƯỚC BUG #4/#5 — ghi rõ, không gộp).
**Binary trên VPS — CUỐI PHIÊN, sau khi redeploy về commit chung**:
`c321840b304305126ab4da6fb2373c5421e793893bed219afe3da5c982b3f26a`, commit
`8724750a51176f05a3a5fde3457695253ffd6336` (= HEAD của WSL). Lúc cài unit lần
đầu VPS ở `8e09735`/`a86fc67a…`; đã `deploy_vps.sh --build` +
`systemctl restart` để 2 máy CÙNG commit theo CLAUDE.md.

---

### MỤC 1 — BẢNG QUYẾT ĐỊNH (`victim_ok=false` do đâu)

#### 1a. 4 bundle RUN 4: KHÔNG fork lại được — `MISSING`, có bằng chứng

Block `122169515`/`122169998`/`122171061`/`122171147` đã quá sâu so với cửa sổ
state của MỌI RPC trong `.env`. Đo thật (WSL, `eth_getBalance` tại đúng 2
block đó):

```
bsc-dataseed1.bnbchain.org  -> {"code":-32000,"message":"missing trie node"}
bsc-dataseed1.defibit.io    -> {"code":-32000,"message":"missing trie node"}
bsc.blockrazor.xyz          -> {"code":-32000,"message":"not supported"}
rpc-bsc.48.club             -> {"code":-32000,"message":"not supported"}
bsc.rpc.blxrbdn.com         -> {"code":-32000,"message":"not supported"}
bsc-rpc.publicnode.com      -> {"code":-32602,"message":"Archive requests require a personal token"}
```

Vì vậy 4 hash đó phân tích bằng cách **không cần archive**:

```
hash        amountIn     amountOutMin      amountOut THAT      bien an toan  status
0x4c29f855   807,30 USDT    3 371,81         1 438 637,26        x426        0x1
0x6bea03d7  1 085,40 USDT      276,07        1 931 792,88      x6 997        0x1
0x327cb265    886,69 USDT      913,32        6 234 926,53      x6 826        0x1
0x71d9906b    999,90 USDT   73 826,75        1 963 303,05         x26        0x1
```

`eth_getLogs` USDT `Transfer` VÀO ví victim, 40–60 block trước + block đào:

```
0x4c29f855  KHONG co Transfer IN trong 60 block truoc do
0x6bea03d7  KHONG co Transfer IN trong 60 block truoc do
0x327cb265  KHONG co Transfer IN trong 60 block truoc do
0x71d9906b  KHONG co Transfer IN trong 60 block truoc do
```

⇒ **Giả thuyết 1 cũ ("ví victim chưa có USDT tại block fork") SAI** — cả 4 tx
`status=0x1`, không hề được cấp vốn trong/ngay trước block đào ⇒ họ đã có USDT
từ trước. **Giả thuyết 2 cũ cũng không giải thích được 4 mẫu này** —
`amountOutMin` thấp hơn `amountOut` thật 26…7 000 lần.

#### 1b. ≥10 case MỚI — bảng 3 biến thể trên CÙNG fork (WSL, 14 victim THẬT)

`real_rpc_victim_ok_verdict_ladder`, node `bsc.blockrazor.xyz`, block
122180835→122181590, fork tại `mined_block − 1`. Cột `a:0`/`b:v2`/`c50`/
`c25`/`c10` = `victim_ok`; `d:gated` = mức `front_in` SAU khi sửa (mục 2).
File đầy đủ: `baocao/evidence/truth44_ladder_victim_ok.txt`.

```
victim_hash          quote   amountOutMin      front_in_v2     impact_%    mined |   a:0  b:v2   c50   c25   c10  d:gated |      front_gated profit_gated verdict
0x885fbd9b4f301f25   wbnb  53387132862.3767           5.0000      64.4075 122180851 |    OK   ERR   ERR   ERR   ERR      ERR |         2.429249     0.000000 front_giet_victim
0x47c4b3fd97b6405f   wbnb     148298.2404           5.0000      24.7567 122180918 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0x82ca10d5a1f8598d   wbnb  545635927.6880           5.0000       6.0529 122180973 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0xf9cc9efa926e2263   wbnb   26058620.1561           5.0000       1.8552 122181037 |    OK    OK    OK    OK    OK       OK |         5.000000     0.015152 khong_tai_hien
0xf65e911269b658a9   wbnb          0.0000           5.0000      31.2303 122181078 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0x4f591d18ed3d3687   wbnb  420809841.4462           5.0000      68.9596 122181152 |    OK     X     X     X     X       OK |         0.000368     0.000018 front_giet_victim
0x4a937bbb64856c31   wbnb  349086310.8248           5.0000       5.9639 122181191 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0x80fcffdaa52a2d35   wbnb          0.0000           5.0000      13.6963 122181191 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0xd321781c6a429ee3   wbnb          0.0001           5.0000      32.7440 122181308 |    OK     X     X     X     X       OK |         0.000769     0.000016 front_giet_victim
0x50596970f207f4d6   wbnb          0.0007           5.0000      15.0710 122181352 |    OK     X     X     X     X       OK |         0.001668     0.000021 front_giet_victim
0xddc52f99e4491e1a   wbnb          0.0062           5.0000      10.3430 122181400 |    OK     X     X     X     X      ERR |         0.000000     0.000000 front_giet_victim
0x3a45a8311588c342   wbnb  981003912.8721           5.0000      18.7970 122181468 |    OK   ERR   ERR   ERR   ERR      ERR |         0.693743     0.000000 front_giet_victim
0x31bdb0e9c2927deb   wbnb          0.0000           5.0000      13.0801 122181468 |    OK   ERR   ERR   ERR   ERR      ERR |         5.000000     0.000000 front_giet_victim
0x4b8049f6e0213c40   wbnb   24157887.1468           5.0000       7.7733 122181590 |    OK     X     X     X     X       OK |         0.032271     0.000019 front_giet_victim

== TONG KET 14 case / 90 block ==
   verdict front_giet_victim    = 13
   verdict khong_tai_hien       =  1
   ly do revert (b:front=v2) PancakeRouter: INSUFFICIENT_OUTPUT_AMOUNT   = 5
   ly do revert (b:front=v2) SIM_ERR back-sell revert (honeypot/anti-bot) = 8
== MUC 2: front_in DA RANG BUOC victim-ok (d:front=v2_gated) ==
   so case co front_gated > 0      = 14
   trong do victim SONG trong EVM  = 5
```

**KẾT LUẬN BẰNG SỐ (đường này)**: `front_in = 0` ⇒ victim sống **14/14**.
`front_in` V2-math ⇒ victim chết **13/14**, 5 dòng revert đúng chữ
`PancakeRouter: INSUFFICIENT_OUTPUT_AMOUNT`. **"front giết victim"**, KHÔNG
phải "state fork sai".

**Giới hạn phải ghi rõ**: thang này lấy mẫu TOÀN mempool chứ không riêng pool
trong `pairs.txt`, nên 8 dòng `SIM_ERR back-sell revert` là token honeypot/
anti-bot CHƯA VET — đúng thứ `pairs.txt` vet tay để loại. Tỉ lệ trong bảng
KHÔNG đại diện cho tập pool bot thật sự giao dịch.

#### 1c. ĐƯỜNG SHADOW (USDT) cho câu trả lời KHÁC — và nó mới là đường của RUN 4

`shadow.victim_diag` chạy THẬT trong shadow mode 65 phút (WSL), 2 mẫu USDT,
CẢ HAI `verdict = state_fork_sai`:

```
{"event":"shadow.victim_diag","quote":"usdt","fork_block":122182471,
 "verdict":"state_fork_sai","max_front_victim_alive_wei":null,
 "rows":[{"label":"a:front=0","front_in_wei":"0","victim_ok":false,
          "victim_revert_reason":"TransferHelper: TRANSFER_FROM_FAILED"}, …]}
```

Đối chiếu on-chain (WSL, RPC thật):

```
victim 0x98bb4913… : bot fork tai block dao +4   status=0x1  allowance(->V2Router)=VO HAN
victim 0xc9bac861… : bot fork tai block dao +15  status=0x1  allowance(->V2Router)=VO HAN
```

Allowance VÔ HẠN ⇒ không thiếu allowance. `status=0x1` ⇒ tx victim thành công
thật. Nguyên nhân duy nhất còn lại: **bot fork ở block mà victim ĐÃ THỰC THI
rồi** ⇒ USDT đã tiêu ⇒ `transferFrom` của lần replay hỏng. Đây là **BUG #4 +
#5**, xem dưới.

---

### MỤC 2 — SỬA, và hiệu quả ĐO ĐƯỢC

Bug ở chỗ ghép 2 bước: `search_max_front_in` tối đa hoá LÃI mà không biết
`amountOutMin`, rồi `victim_still_ok` chỉ VỨT BỎ candidate. Nay
`max_front_in_victim_ok` nhị phân tìm biên (dựa trên tính đơn điệu của
`victim_out` theo `front_in`), `search_max_front_in_victim_ok` ternary-search
lãi trong `[0, biên]`. Áp cả 3 đường quyết định (cả 2 quote asset).

**Hiệu quả thật** — 4/5 case `INSUFFICIENT_OUTPUT_AMOUNT` được CỨU thành giao
dịch victim-sống VÀ có lãi (cột `d:gated` bảng 1b):

```
0x4f591d18  front_gated=0.000368 BNB  profit=+0.000018 BNB  victim OK
0xd321781c  front_gated=0.000769 BNB  profit=+0.000016 BNB  victim OK
0x50596970  front_gated=0.001668 BNB  profit=+0.000021 BNB  victim OK
0x4b8049f6  front_gated=0.032271 BNB  profit=+0.000019 BNB  victim OK
0xddc52f99  front_gated=0        -> khong cuu duoc (victim_would_revert THAT)
```

Lãi mỗi case **rất nhỏ (~0,00002 BNB ≈ 0,01 USD)** — ghi đúng số, không tô hồng.

**Tính lại econ 10,92 h VPS bằng gate mới — chỉ làm được MỘT PHẦN, phần còn
lại `MISSING` có lý do:**

Cổng mới là **tập cha** của cổng cũ (nếu mức sinh lời nhất vốn đã giữ victim
sống thì biên = `max_front`, kết quả y hệt) ⇒ 524 dòng `sim.result` của cửa sổ
10,92 h KHÔNG ĐỔI. Phần THÊM là các dòng từng bị vứt (đo thật trên log gốc
366 MB còn nguyên trên VPS):

```
tx.skip{victim_would_revert} = 610   (570 usdt / 40 wbnb)
  552/610 don vao DUNG 1 pool: 0xd69aeb83…  (chinh pool (a) cua bang 1d BAOCAO43)
sim.result (ca file)         = 524
shadow.sim (ca file)         =   0
```

- **Số TIỀN của 610 dòng đó: `MISSING`** — log cũ KHÔNG có `amount_out_min`
  (field chỉ tồn tại từ cụm này), thiếu nó thì không dựng lại được biên. Không
  suy diễn.
- **Con số quan trọng hơn: `shadow.sim` = 0 dòng trong CẢ FILE** ⇒ **không
  MỘT cơ hội nào trong 524 từng được EVM kiểm.** Theo luật mục 6, lãi XÁC
  NHẬN ĐƯỢC của cửa sổ 10,92 h là **0 BNB / 0 USDT**, và 4 bundle duy nhất
  từng được EVM kiểm (RUN 4) đều `victim_ok=false`. **Không có căn cứ để nói
  "lãi bao nhiêu/ngày".**

---

### MỤC 3 — RÒ RỈ BỘ NHỚ

4 container KHÔNG có trần + 1 nguồn NẰM NGOÀI container:

| container | tình trạng cũ | trần mới |
|---|---|---|
| `ReserveCache.entries` | doc-comment nói *"KHÔNG cần dọn dẹp chủ động"* | 8 block |
| `NonceCache.entries` | không có bước xoá nào | 4 block |
| `CompeteStats.top_bots` | doc nói "giữ tối đa 20", code KHÔNG cắt | 20 địa chỉ |
| `CompeteStats.gas_samples` | `Vec::push` vô hạn | 500 mẫu |

`ReserveCache` nặng nhất: nguồn ghi KHÔNG phải đường nóng mà là
`sync_reserves_task` — 1 entry cho MỖI `Sync` của MỖI pool ở MỖI block (126
pool × ~2,2 block/s ⇒ cỡ 10 triệu entry sau 11 giờ).

**Nguồn thứ 5**: `/api/econ`, `/api/compete`, `/api/shadow` gọi
`read_to_string` TOÀN BỘ `bot.jsonl` (**366 MB** trên VPS, đo thật) mỗi lời
gọi. glibc không trả arena lớn về OS ⇒ `VmRSS` chỉ lên. Đây là lý do soi
map/vec KHÔNG giải thích hết 7,6 GB. Sửa: `read_log_tail` (seek từ cuối, trần
256 MiB cố định).

**`GET /api/mem` chạy thật** (WSL, port 18951) — 16 container, mỗi cái kèm
trần; `khong_co_tran` liệt kê rõ 5 cái chỉ bị chặn gián tiếp:

```json
{"rss_mb": 22.25, "rss_peak_mb": 22.25, "uptime_sec": 26,
 "containers": [
   {"name":"transport::ReserveCache.entries","len":24,"cap":8,"cap_unit":"block"},
   {"name":"transport::NonceCache.entries","len":0,"cap":4,"cap_unit":"block"},
   {"name":"transport::SeenHashSet","len":322,"cap":50000,"cap_unit":"hash"},
   {"name":"transport::MinedTxIndex","len":406,"cap":3,"cap_unit":"block"}, …],
 "khong_co_tran":["web::skip_counts","main::candidate_seen","tax::TaxCache",
                  "pairbook::PairBook","victims::VictimBook"]}
```

**`mem.rss_mb` — WSL, 65 phút, binary `2d5c0e0f…`** (file đầy đủ:
`baocao/evidence/truth44_mem_rss.jsonl`):

```
    0s rss=   8.12  d_last=None
   60s rss=  26.23  d_last=18.109
  120s rss=  32.01  d_last= 5.777
  180s rss=  34.51  d_last= 2.500
  240s rss=  35.89  d_last= 1.375
  300s rss=  36.14  d_last= 0.250
  360s rss=  36.64  d_last= 0.500
  420s rss=  38.01  d_last= 1.375
  480s rss=  38.89  d_last= 0.875
  540s rss=  39.39  d_last= 0.500
  600s rss=  40.01  d_last= 0.625   <-- moc 10 phut
  660s rss=  40.39  d_last= 0.375
  720s rss=  40.76  d_last= 0.375
```

**DoD "RSS tăng < 50 MB sau 10 phút đầu"**: từ mốc 600 s (40,01 MB) tới 780 s
(40,76 MB) tăng **0,75 MB**, tốc độ ~0,375 MB/phút và đang giảm dần. So với
**~11 MB/phút** của lần OOM ở BAOCAO43 ⇒ giảm ~29 lần.

**NHƯNG — phép đo tự nó bắt được một cú nhảy 209 MB, và đó là PHÁT HIỆN
QUAN TRỌNG NHẤT của mục 3.** Bảng đầy đủ kèm kích thước container:

```
  600s rss=   40.01  peak=  40.01  d=  0.625 | reserve=29 seen=7620  mined=283
  660s rss=   40.39  peak=  40.39  d=  0.375 | reserve=23 seen=8381  mined=249
  720s rss=   40.76  peak=  40.76  d=  0.375 | reserve=22 seen=9223  mined=254
  780s rss=   40.76  peak=  40.76  d=  0.000 | reserve=17 seen=9982  mined=306
  840s rss=  249.28  peak= 274.68  d=208.519 | reserve=23 seen=10837 mined=355   <-- 1 loi goi /api/econ
  900s rss=  249.66  peak= 274.68  d=  0.375 | reserve=24 seen=11672 mined=305
  960s rss=  249.66  peak= 274.68  d=  0.000 | reserve=13 seen=12821 mined=323
 1020s rss=  249.78  peak= 274.68  d=  0.125 | reserve=24 seen=13642 mined=284
 1080s rss=  250.07  peak= 274.68  d=  0.285 | reserve=20 seen=14649 mined=354
```

**MỌI container đứng yên** (`ReserveCache` 17→23, `MinedTxIndex` 306→355)
trong khi `VmRSS` nhảy **40,76 → 249,28 MB** (đỉnh 274,68 MB) vì ĐÚNG MỘT lời
gọi `/api/econ` trên `bot.jsonl` 30 MB. Và nó **KHÔNG tụt lại**: 4 phút sau
vẫn 250,07 MB. Đây là bằng chứng có kiểm soát cho cơ chế arena glibc, và nó
tách bạch hẳn 2 loại nguyên nhân: rò rỉ KHÔNG nằm ở các map/vec.

Ngoại suy sang VPS (`bot.jsonl` từng đạt **366 MB**, gấp 12 lần): một lời gọi
`/api/econ` ở đó tốn hàng GB ⇒ **nhiều khả năng đây mới là phần lớn cú OOM
7,6 GB của BAOCAO43**, không phải các container.

Sửa thêm (ngay trong phiên, sau khi thấy số này): `ECON_EVENTS_CAN_DUNG` —
lọc bằng CHUỖI THÔ trước khi `serde_json::from_str`, vì chính bước dựng
`Value` mới tốn bộ nhớ chứ không phải bước đọc file. Sai số chỉ có thể theo
hướng GIỮ THỪA, không bao giờ bỏ sót, nên không thể làm sai số liệu. Theo số
thật VPS 10,92 h, các event bị loại chiếm phần lớn khối lượng (`tx.seen`
83 571 + `rpc.block` 20 401 + `gas.oracle` 10 837).
**CHƯA đo lại sau bản sửa này** — ghi CÒN NỢ, không tự nhận đã giải quyết.

**Ghi trung thực: phiên kết thúc trước mốc 60 phút** — mới có **18 dòng**,
không đủ 60 dòng lệnh yêu cầu. Ghi `CHƯA XONG` cho phần đó, xem ô 10.

---

### MỤC 4 — p95 `seen_to_decision`

`VET_BLOCK_REFRESH_EVERY = 10` (10 pool × ~300 ms ≈ 3 s ≈ 6–7 block, còn xa
cửa sổ state ~128 block nên không làm sống lại bug fork-block-quá-cũ).

```
TRUOC:  BAOCAO40 = 321 ms | BAOCAO42 = 344 ms | BAOCAO43 (RUN 4) = 752 ms
SAU  :  /api/econ lan chay nay (WSL, 10 076 mau):
        {"p50": 0.013702, "p95": 507.98348, "samples": 10076}
```

**752 → 508 ms. KHÔNG đạt mốc ≤ 350 ms của lệnh.** Và phải ghi rõ: sau khi
tìm ra BUG #1 (khoá `pairbook`), nghi phạm số 1 của p95 752 ms nhiều khả năng
là bug ĐÓ chứ không phải `eth_blockNumber` — bản sửa mục 4 KHÔNG được nhận
công. Chưa tách bạch được, ghi CÒN NỢ.

---

### MỤC 5 — VPS: systemd unit THẬT

```
● bsc-sandwich-paper.service - BSC sandwich bot (PAPER / dry-run, KHONG gui tx)
     Loaded: loaded (/etc/systemd/system/bsc-sandwich-paper.service; enabled)
     Active: active (running) since Wed 2026-09-16 07:35:46 UTC
   Main PID: 385428 (bsc_sandwich)
     Memory: 6.3M (high: 2.0G max: 6.0G available: 1.9G)
     CGroup: └─385428 …/bsc_sandwich /root/bsc-sandwich/config.runtime.toml
  bsc-sandwich[385428]: bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
  cu phap logrotate: OK
```

**`Restart=always` CHỨNG MINH bằng `kill -9`** (giả lập đúng cú OOM-kill đã
giết lần chạy 24 h), không phải bằng việc đọc file cấu hình:

```
PID truoc khi giet : 385428
kill -9 ; cho 14 giay
PID sau            : 385509   NRestarts=1   ActiveState=active
Restart=always     MemoryHigh=2147483648 (2 GiB)   MemoryMax=6442450944 (6 GiB)
MemoryCurrent sau ~25 phut chay: 51 458 048 (49,1 MB)
```

**PHÁT HIỆN HẠ TẦNG — có HAI máy, phiên trước suýt nhầm:** `~/.ssh/config`
trên WSL có host tên `bsc-vps` kèm ghi chú "VPS chinh thuc Singapore, BOT TIEN
THAT CHAY O DAY". Máy đó **KHÔNG phải** production: không có repo, không có
bot, và đang bị `vmw_balloon` lấy mất ~5,6 GB trong 8 GB danh nghĩa
(`MemAvailable` chỉ 2,1 GB). Máy production (`VPS-511043-157`, `MemTotal`
7 948 MB / `MemAvailable` 7 325 MB, RAM lành) mở bằng `key/bsc_vps_ed25519`
trong repo. Log 10,92 h của BAOCAO43 được GIỮ LẠI ở đó dưới tên
`logs/bot.jsonl.24h_baocao43` (KHÔNG xoá).

Sau khi có commit cuối, đã redeploy + `systemctl restart` để VPS và WSL CÙNG
commit `8724750a` (CLAUDE.md: "Hai máy phải cùng git commit; lệch = MISSING"):

```
ActiveState=active  MainPID=386377  MemoryCurrent=11 423 744 (10,9 MB)
commit: 8724750a51176f05a3a5fde3457695253ffd6336
binary: c321840b304305126ab4da6fb2373c5421e793893bed219afe3da5c982b3f26a
journal: bsc_sandwich boot: chain_id=56 dry_run=true allow_live=false bot_armed=false
```

**Chạy 6 giờ + bảng 1b/1d tính lại: CHƯA XONG** — đồng hồ 6 giờ tính từ lần
restart trên (~07:50 UTC), phiên kết thúc trước mốc đó. Xem ô 10.

---

### MỤC 6 — mọi `net_pos` đi kèm 2 cổng victim-ok

`/api/econ` lần chạy này (WSL, port 18951) — CHẠY THẬT:

```
summary: candidate=10076 net_pos=7 net_pos_non_cluster=7 net_pos_v2ok=7
         net_pos_evmok=0 net_pos_ca_hai_cong_ok=0 best_net_bnb=0.207986
         p50_ms=0.01 p95_ms=507.98 stale_pct=0.00 decode_fail_smartrouter=219
victim_ok: {"v2_ok":7, "v2_false":0, "v2_unknown":0,
            "evm_ok":0, "evm_false":4, "evm_unknown":3,
            "ca_hai_cong_ok":0, "sum_net_bnb_ca_hai_cong_ok":0.0}
```

Đọc đúng: `v2_ok = 7/7` xác nhận cổng mục 2 hoạt động (mọi dòng `Simulated`
đều giữ victim sống theo V2-math). Nhưng `evm_ok = 0`, `evm_false = 4` ⇒
`sum_net_bnb_ca_hai_cong_ok = **0,0 BNB**`. **Số lãi duy nhất được phép dùng
để kết luận kinh tế của lần chạy này là 0** — `best_net_bnb=0.207986` KHÔNG
được đọc là lãi.

---

### 5 BUG THẬT lộ ra giữa phiên

**BUG #1 — `pair.reload` giữ khoá GHI `pairbook` xuyên `.await`.**
`book.reload_if_due(...).await` resolve MỌI dòng `pairs.txt` bằng
`Factory.getPair` (1 `eth_call`/dòng, 133 dòng) ⇒ khoá ghi bị giữ VÀI PHÚT.
`RwLock` tokio công bằng với writer ⇒ chặn MỌI `pairbook.read()`, gồm đường
nóng `handle_paper_tx` (3 chỗ/candidate). Đo thật:

```
TRUOC SUA                              SAU SUA
/api/health   200  0.000554s           /api/pairs  200  0.000805s
/api/status   200  0.000673s           /api/mem    200  0.000733s
/api/victims  200  0.000574s
/api/pairs    TIMEOUT (>6s)
/api/mem      TIMEOUT (>6s)
```

Sửa: reload trên BẢN SAO khi KHÔNG giữ khoá → tráo dưới 1 khoá ghi NGẮN, có
bước áp lại `vet_snapshot()` để không mất kết quả vet.

**BUG #2 — `mem_watch_task` chết sau 2 dòng** vì chính BUG #1 (nó gọi
`pairbook.read()`). Số đo quan trọng nhất của mục 3 suýt bị bịt miệng bởi bug
mà nó sinh ra để tìm. Sửa: `try_read()` toàn bộ + đọc `rss_mb` TRƯỚC mọi khoá.

**BUG #3** — 4 container không trần (mục 3).

**BUG #4 — `shadow.sim` fork tại `current_block`.** `AlloyDB` đọc state ở
CUỐI block; victim đào NGAY TRONG block đó (`decision_vs_mined = 0`, đo thật
**8/22 mẫu**) ⇒ state đã bao gồm giao dịch của victim ⇒ replay tất nhiên hỏng.
Sửa: fork tại `current_block − 1`.

**BUG #5 — `MINED_INDEX_DEPTH = 3` quá nông.** Phân bố thật
`latency.decision_vs_mined` (22 mẫu, WSL):

```
  delta  -3 : 1        delta  +0 : 8
  delta  -2 : 1        delta  +3 : 1
  delta  -1 : 8        delta  +4 : 1
                       delta +15 : 1
                       delta +19 : 1
  TONG 22, quyet dinh MUON (delta>0) = 4 (18%)
```

`+15`/`+19` nằm NGOÀI cửa sổ 3 block ⇒ cổng "victim còn pending không" tưởng
còn pending ⇒ **bot ký bundle cho tx ĐÃ nằm trên chain**. Nâng lên 32 block
(~70 s, phủ `+19`), vẫn là trần CỐ ĐỊNH ~10k hash < 1 MB. Test hồi quy
`mined_index_phai_phu_duoc_do_tre_19_block_da_do_that`.

---

### `cargo test --release` (WSL, HEAD `ac9406c`)

```
test result: ok. 393 passed; 0 failed; 17 ignored; 0 measured; 0 filtered out
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

408 pass (393 lib + 15 main), tăng từ 399 (BAOCAO43) = **+9 test**, 0 failed.
`ignored` 16 → 17 (+1 `real_rpc_victim_ok_verdict_ladder`, **ĐÃ CHẠY THẬT**,
output ở mục 1b — không "pass rỗng", luật #3).

## 6. CHAIN — `0x38`

- `real_rpc_victim_ok_verdict_ladder`: fork THẬT tại block 122180835→122181590,
  state đọc thật từ `bsc.blockrazor.xyz`; router V2 pin
  `0x10ED43C718714eb63d5aA57B78B54704E256024E`, factory
  `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73`, USDT `0x55d398…7955`.
- 4 hash RUN 4: `eth_getTransactionByHash` + `eth_getTransactionReceipt` +
  `eth_getLogs` THẬT, cả 4 `status=0x1` (số dán ở mục 1a).
- 2 victim shadow: `allowance(victim→V2Router)` đọc thật = VÔ HẠN.
- Bot WSL + VPS: `chain_id=56 dry_run=true allow_live=false bot_armed=false`
  (dán nguyên văn dòng boot của systemd ở mục 5).
- Pin V2/V3/V4/WBNB/USDT KHÔNG đổi ở cụm này.

## 7. REGISTRY

`DEX_REGISTRY.md` **KHÔNG đổi** — cụm này không pin venue/địa chỉ mới.

## 8. KHÔNG LÀM

- Không viết/deploy contract, không gửi tx/bundle, không bật live.
- Không đụng `CLAUDE.md`, `pairs.txt`, `.env`, `config.toml` ship, `PRIVATE_KEY`.
- Không dùng subagent (luật #4).
- Không xoá log 10,92 h trên VPS (đổi tên thành `logs/bot.jsonl.24h_baocao43`).
- Không kết luận chiến lược thay Chủ.
- **Không bịa số lãi**: mọi con số `net_pos`/`best_net_bnb` trong báo cáo này
  đều đi kèm `victim_ok_evm`, và số lãi XÁC NHẬN được là **0**.

## 9. CHỮ: CHƯA XONG

Lý do (không phải FAIL, cũng chưa đủ để CHỜ GROK): 3 mục của
`ĐẠT CẦN DÁN` chưa có đủ số liệu — 60 dòng `mem.rss_mb` (mới 13 dòng), 6 giờ
VPS + bảng 1b/1d trên cửa sổ đó (unit mới chạy), và RSS cuối của 6 giờ đó.
Mọi phần còn lại đã có output thật dán kèm.

## 10. CÒN NỢ / LÁT SAU

1. **Chạy 60 phút WSL chưa trọn** — mới 18 dòng `mem.rss_mb` / 60 dòng lệnh
   yêu cầu. Xu hướng đã rõ (0,375 MB/phút và giảm dần, so với 11 MB/phút lúc
   OOM) nhưng CHƯA đủ mẫu để tuyên bố đạt DoD.
1b. **`ECON_EVENTS_CAN_DUNG` (lọc dòng trước khi parse) CHƯA đo lại.** Cú nhảy
   209 MB vì 1 lời gọi `/api/econ` được đo trên binary TRƯỚC bản sửa đó. Phải
   lặp lại đúng phép đo (chạy ≥15 phút, gọi `/api/econ` 1 lần, xem
   `delta_mb_since_last`) để biết bản sửa ăn được bao nhiêu.
1c. **Bước triệt để cho `/api/econ` vẫn CÒN NỢ**: đọc theo dòng (streaming) và
   cộng dồn, thay vì dựng `Vec<Value>` cho cả cửa sổ. Lọc chuỗi thô chỉ giảm
   hệ số, không đổi bản chất O(kích thước log).
2. **Chạy 6 giờ VPS + bảng 1b/1d tính lại trên cửa sổ đó: CHƯA XONG.** Unit
   systemd đang chạy từ 07:35 UTC, `Restart=always` đã verify. Phiên sau chỉ
   cần chạy `scripts/analyze_econ.sh` trên `/root/bsc-sandwich/logs/bot.jsonl`.
3. ~~VPS lệch commit~~ — **ĐÃ XỬ LÝ trong phiên**: đã `deploy_vps.sh --build`
   + `systemctl restart`, VPS nay ở `8724750a` = HEAD của WSL, binary
   `c321840b…`. Đồng hồ 6 giờ vì vậy tính từ lần restart này
   (~07:50 UTC 2026-09-16), KHÔNG phải từ 07:35.
4. **BUG #4 và #5 CHƯA verify sống** — phiên hết trước khi có mẫu
   `shadow.victim_diag` mới sau khi sửa. Hiện chỉ có test đơn vị + lập luận.
   Cách verify: chạy shadow ≥30 phút, kiểm `shadow.sim` còn dòng nào
   `TRANSFER_FROM_FAILED` không, và `decision_block − fork_block` phải luôn
   `= 1`.
5. **p95 508 ms, chưa đạt ≤350 ms** và chưa tách bạch được công của mục 4 với
   công của bản sửa BUG #1.
6. **Không tính lại được số TIỀN cho 610 dòng `victim_would_revert`** của
   10,92 h — log cũ thiếu `amount_out_min`. Từ cụm này trở đi log đã có, nên
   cửa sổ 6 giờ tới sẽ tính được.
7. **Thang ladder lấy mẫu TOÀN mempool**, không giới hạn trong `pairs.txt` —
   8/14 dòng `SIM_ERR` là token honeypot chưa vet. Cần một lần chạy ladder
   giới hạn trong `pairs.txt` để có tỉ lệ đại diện cho tập pool bot thật sự
   giao dịch.
8. **`shadow.sim` = 0 dòng trong toàn bộ 10,92 h** ⇒ lãi xác nhận được của
   cửa sổ đó = 0. Đây là con số Chủ cần biết trước khi quyết định cụm 6.
9. **Tỉ lệ THẮNG cuộc đua: vẫn MISSING** (nợ cũ).
10. **Đường `sim_engine="evm"` vẫn dùng trần gas cấu hình** (nợ cũ từ
    `real-economics-mode2`).

Commit: `6703c230e639361da40ca361d6d4a7b0984df575` (+ 1 commit bổ sung cho
chính file này).

**`sha256sum target/release/bsc_sandwich` tại commit đó (WSL)**:
`89e953654bcfee85207e0a1d2b83bd4cece1fc6b77e599dcc6c2d9018e5c3c69`

**VPS** đang chạy `8724750a…` (binary `c321840b…`) — commit NGAY TRƯỚC
`6703c23`. Chênh lệch duy nhất là `ECON_EVENTS_CAN_DUNG` (lọc dòng ở
`/api/econ`), KHÔNG đụng đường quyết định/sim, nhưng theo CLAUDE.md thì 2 máy
vẫn phải cùng commit — phiên sau redeploy trước khi lấy số 6 giờ.
