# BAOCAO45 — cụm `verify-cluster-as-victim`

## 1. LÁT

`verify-cluster-as-victim` — xác minh bằng EVM (fork đúng block) rằng tx của
cụm `0xB406` sandwich được và lãi bao nhiêu; hoàn tất 3 phần CHƯA XONG của
BAOCAO44. Gồm mục 1→6 của lệnh, **cộng 4 BUG THẬT lộ ra giữa phiên** (việc
dính liền cùng phiên, CLAUDE.md mục "Cùng phiên — khỏi nợ"):

- **Mục 1** hoàn tất BAOCAO44: 71 dòng `mem.rss_mb` (WSL), đo lại cú nhảy
  `/api/econ`, verify BUG #4/#5 sống trên VPS.
- **Mục 2** bảng quyết định cụm đối thủ — **trả lời được, và câu trả lời đổi
  hẳn sau khi sửa 2 lỗi ĐO** (xem BUG #6/#7).
- **Mục 3** đường gửi của cụm — **giả thuyết "họ đã chuyển private" BỊ BÁC BỎ
  bằng nhóm đối chứng**.
- **Mục 4** ai đang kẹp cụm — 0/554.
- **Mục 5** `competitor::ClusterRateWatch` + `competitor.alert` + STATE.md.
- **Mục 6** p95 — tìm ra nguyên nhân và sửa, 599 ms → 0,126 ms.
- **BUG #6** `shadow.sim` fork tại block victim ĐÃ đào khi bot quyết định trễ
  (BUG #4 của BAOCAO44 sửa CHƯA ĐỦ).
- **BUG #7** ví burner của cụm được cấp vốn trong CHÍNH block đào → không block
  nào replay được victim → mọi số mục 2 trước đó là artifact.
- **BUG #8** `competitor.rs` ghi ĐỊA CHỈ POOL vào cụm như thể là ví burner.
- **BUG #9** `subscribe_ws_heads` bỏ cuộc VĨNH VIỄN khi kênh lag → `rpc.block`
  chết im lặng, `current_block` thành cũ.

**Chủ ra lệnh giữa phiên chốt sớm** (lúc 09:57 UTC) thay vì chờ đủ mốc 6 giờ
VPS. Cửa sổ VPS thật của báo cáo này là **1,62 giờ** — ghi rõ ở ô 9/ô 10, không
nhận là 6 giờ.

## 2. LỆNH NHẬN

Khối lệnh Grok `verify-cluster-as-victim`. HEAD lệnh ghi `abb352a`; HEAD THẬT
lúc mở phiên là `d742bbb` (= `abb352a` + 1 commit bổ sung chính dòng
"VPS đã redeploy" của BAOCAO44). Máy: WSL (code + đo) và VPS (`VPS-511043-157`,
unit systemd). Không contract, không live, không subagent (luật #4). IP VPS
KHÔNG ghi vào bất kỳ file nào trong repo.

## 3. FILE ĐỔI

Commit cuối: `2591b186e7d5b973aee1b9624922778a5404a4fb`. 9 commit trong phiên:

```
2591b18  scripts/cluster_check_hashes.py + 6 file bang chung
f9361ad  BUG #9 subscribe_ws_heads bo cuoc vinh vien khi kenh lag
6eb009d  muc 1b: do lai cu nhay /api/econ - ban sua BAOCAO44 KHONG an, sua that
5cf653d  test real_rpc chung minh co che nap von + bo #[test] trung
2a1a71d  BUG #8 loc POOL khoi cum + nguon su that on-chain cho phan tich
ca0a644  BUG #6+#7 sua 2 loi lam moi sim USDT cua cum vo nghia
d70429f  muc 2: log victim_out_no_front_wei de dung duoc `room`
ee8c804  muc 3+6: cong 0-RPC cho not_in_list + dau do mempool + 2 script quet
cc285d7  muc 5: canh bao khi cum doi thu bien mat + co shadow/competitor cho VPS
```

**MỚI**

- `src/bin/mempool_probe.rs` — đầu dò mempool THUẦN, ghi MỌI hash pending
  (không lọc). Cần binary riêng vì bot chỉ ghi `tx.seen` cho tx đã qua gate
  router, mà 65% tx của cụm đi tới contract riêng của họ.
- `scripts/cluster_tx_scan.py` — quét on-chain tx của cụm (`eth_getLogs` seed
  transfer → chỉ nạp block liên quan) + hàng xóm ±3 vị trí cùng pool.
- `scripts/cluster_check_hashes.py` — kiểm ĐÚNG một danh sách hash (rẻ hơn).
- `scripts/analyze_cluster45.py` — mục 3 + mục 4 từ file, không gọi RPC.
- `scripts/analyze_cluster_econ45.py` — mục 2 (bảng theo pool).
- `scripts/mem_table45.py` — in bảng `mem.rss_mb` (`len=null` in `?`, không in `0`).
- `baocao/evidence/baocao45_*.txt` — 6 file output thật.

**SỬA**

- `src/competitor.rs` — `ClusterRateWatch` (đếm candidate cụm theo phút, trần
  cứng 180 bucket, so 2 cửa sổ 60 phút, ngưỡng >80%, đường nền ≥20, cooldown
  60 phút) + 6 test.
- `src/main.rs` — `can_skip_not_in_list_without_rpc` (mục 6) áp ở CẢ 2 nhánh
  quote; `cluster_rate_watch_task` + event `competitor.alert`;
  `spawn_shadow_bundle_sim` fork tại `min(decision_block, mined_block) − 1` và
  truyền `victim_topup`; lọc POOL khỏi `competitor.funded`; `subscribe_ws_heads`
  thử lại mãi + `channel_size`; `meta.victim_out_no_front_wei` cả 2 nhánh. +3 test.
- `src/sim_evm.rs` — `run_sandwich_quote_topup` / `simulate_sandwich_quote_topup`
  / `run_sandwich_quote_cached_topup`; `read_allowance`; `EvmSandwichOutcome`
  thêm `victim_quote_balance_before`/`victim_quote_allowance`/
  `victim_quote_topped_up`; test thật `real_rpc_cluster_burner_victim_song_lai_khi_duoc_nap_von`.
- `src/pipeline.rs` — `TxLogMeta::victim_out_no_front_wei` + log vào `sim.result`.
- `src/transport.rs` — `MinedTxIndex::block_of`.
- `src/web.rs` — `cluster_rate` trong `/api/compete`; `ECON_FIELDS_CAN_DUNG` +
  `prune_econ_row`; `read_log_tail` bỏ 1 bản sao thừa; container
  `ClusterRateWatch.buckets` trong `/api/mem`; bỏ `#[test]` trùng. +1 test.
- `scripts/paper_run.sh` — cờ `--allow-competitor-victims`.
- `scripts/install_systemd_vps.sh` — cờ `--shadow` + `--allow-competitor-victims`.
- `docs/STATE.md` (+1 mục lớn, 6 tiểu mục), `docs/TASKS.md`.

**KHÔNG đụng**: `CLAUDE.md`, `config.toml` (ship giữ nguyên), `pairs.txt`,
`PRIVATE_KEY`, cờ live, contract. `.env` chỉ THÊM `BSC_HTTP_SIM` (gitignored).

## 4. LỆNH CHẠY

```bash
# --- WSL ---
cargo build --release && cargo test --release
BSC_HTTP_SIM=... cargo test --release --lib -- --ignored \
    real_rpc_cluster_burner_victim_song_lai_khi_duoc_nap_von --nocapture
scripts/paper_run.sh --minutes 70 --port 18951 --live-mode shadow --allow-competitor-victims
./target/release/mempool_probe --minutes 75 --out logs/mempool_probe45.jsonl
python3 scripts/cluster_tx_scan.py --from-block 122192710 --to-block 122198698 --out cluster_wsl.jsonl
python3 scripts/cluster_check_hashes.py --hashes vps_hashes.txt --out vps_cluster.jsonl
python3 scripts/analyze_cluster_econ45.py --log <log> --cluster-from-scan <file>
python3 scripts/analyze_cluster45.py --scan cluster_wsl.jsonl --log logs/bot.jsonl.run70
python3 scripts/mem_table45.py logs/bot.jsonl.run70

# --- VPS ---
./scripts/deploy_vps.sh --host <VPS> --identity key/bsc_vps_ed25519 --build
ssh ... './scripts/install_systemd_vps.sh --paper-thresholds --shadow --allow-competitor-victims --start'
```

## 5. OUTPUT THẬT

**Máy / hash (luật #2)**

| việc | máy | commit | sha256 binary |
|---|---|---|---|
| paper run 70 phút + p95 SAU | WSL | `ca0a644` | `c2ac102187495824bfbce2d4b046d6c4ac59c1c85e871fa8373466b8ff74ba77` |
| p95 TRƯỚC (11 650 quyết định) | WSL | `cc285d7` | (cùng cây build, `cargo run` qua `paper_run.sh`) |
| đo `/api/econ` SAU bản sửa | WSL | `f9361ad` | `71bdca78544b91758783c524362ad8ad8ef678c413736ea146852be00f671ac0` |
| cửa sổ đo VPS 1,62 h | VPS | `ca0a644` | `37a45a1ebcb6a0da16c483be20d7186a1a238791cf082f31c59ca4afda7fda52` |
| trạng thái CUỐI phiên | WSL + VPS | **`2591b18`** | WSL `71bdca78…` / VPS `2f77040a349fc874f07a2b69d8a1f6c78904e27af0f59ee61b3a7b65e7e69b05` |

Hai máy CÙNG commit `2591b186e7d5b973aee1b9624922778a5404a4fb` lúc đóng phiên
(binary khác hash là bình thường — mỗi máy tự `cargo build --release`, không
phải build tái lập bit-chính-xác).

---

### MỤC 2 — BẢNG QUYẾT ĐỊNH (đây là con số Chủ cần)

**Trước hết phải nói 2 lỗi ĐO, vì không sửa thì bảng này toàn số 0.**

`shadow.sim` của MỌI victim quote-USDT đều `TransferHelper: TRANSFER_FROM_FAILED`
**kể cả ở `front_in = 0`** (thang `victim_diag` 5/5 mức a/b/c50/c25/c10 đều
hỏng). Đối chiếu on-chain bằng `eth_getBlockReceipts` của ĐÚNG block đào:

```
0x1dec51d5  from 0x1d2da6f65f  mined 122188886 idx 4   CAP VON CUNG BLOCK tai idx 3  tu 0xb406021e07  1184.70 USDT
0xdc6c4fcc  from 0xcb3083af76  mined 122189853 idx 7   CAP VON CUNG BLOCK tai idx 6  tu 0xb406021e07   341.81 USDT
0x55df9c84  from 0xf413f50c07  mined 122190163 idx 10  CAP VON CUNG BLOCK tai idx 9  tu 0xb406021e07  1006.23 USDT
0xb55fe9e1  from 0xeefcd7b4a3  mined 122190311 idx 8   CAP VON CUNG BLOCK tai idx 7  tu 0xb406021e07  1227.80 USDT
0x32c60fe1  from 0xb789d96f1b  mined 122189837 idx 10  CAP VON CUNG BLOCK tai idx 9  tu 0xb406021e07   993.30 USDT
```

5/5 ví victim được seed cấp USDT trong CHÍNH block đào, ở `tx_index` ngay
liền trước; cả 5 tx trên chain đều `status = 0x1`. Tại `block − 1` ví đó có
**0 USDT** ⇒ replay tất nhiên hỏng. **Đây là artifact của phép đo, không phải
sự thật kinh tế** (BUG #7). Cộng thêm BUG #6: 5/5 có `fork_block` bằng ĐÚNG
`mined_block` vì bot quyết định trễ.

**Chứng minh cơ chế trên chain thật** (`real_rpc_cluster_burner_victim_song_lai_khi_duoc_nap_von`,
WSL, file đầy đủ `baocao/evidence/baocao45_realrpc_topup.txt`):

```
MAU THAT: block dao=122194457 fork=122194456 victim=0x5875a1b7... from=0x22cace84... amount_in=39.46 USDT
  (a) KHONG nap von: victim_ok=false reason=Some("TransferHelper: TRANSFER_FROM_FAILED") so_du_quote_cua_victim_tai_fork=0
  (b) CO nap von:    victim_ok=true  reason=None victim_out=137593520041394294029085 allowance=2^256-1
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 416 filtered out; finished in 30.17s
```

`allowance = 2^256−1` đọc THẬT từ chain (sim KHÔNG BAO GIỜ ghi đè allowance)
⇒ loại hẳn giả thuyết allowance.

**BẢNG MỤC 2 — VPS, cửa sổ 08:50:55 → 10:12 UTC = 1,32 h, commit `ca0a644`**
(file đầy đủ `baocao/evidence/baocao45_muc2_vps.txt`; phân loại cụm lấy từ
quét on-chain, KHÔNG lấy từ cờ sống của bot — xem BUG #8):

```
sim.result=59  shadow.sim=59  bundle.shadow_econ=59   cua so=1.32 h
co CUM: bot nhan dien luc chay = 6 ; on-chain that su = 58 ; bot BO SOT = 52

---- CUM DOI THU: 58 candidate Simulated ----
hash                 quote            amountOutMin      room  front_gated    profit_v2   profit_sim  ok_evm
0xf58fa20bcf500e3d   usdt   939978678608444014985216    1.6667  2999.995000    59.316148    59.473667    True
0xe811f42cf12b60ab   usdt  2848342712668629843836928    1.8783  2999.995000   117.142941   110.025482    True
0x4a301abf9579ef84   usdt  1786208660659444291469312    1.5872  2999.995000   126.238512   129.815826    True
0x88794174f705a255   usdt  4238762438479373513261056    1.5629  2999.995000   167.058812   172.928145    True
0x73cd3e58f15a4310   usdt  5540302929143649831747584    1.5383  2999.995000   212.077235   221.189884    True
0xb581fc1471b0c82b   usdt   335584278580196611719168    1.6574  2999.995000     1.262496     1.464044    True
0xf05be869d8953840   usdt   275063315978515457769472    1.6902  2983.919773     0.061198     0.117764    True
   … (58 dòng, bảng đầy đủ trong file evidence)

  BANG THEO POOL (CUM DOI THU):
  pool                   quote     n  n_evm  %ok_evm     p50_sim     p90_sim     tong_sim    tong/gio
  0xdfe23efbdb03ea985c   usdt     24     24    100.0   42.705678  116.505200  1528.383005 1161.316145
  0xcec13213c390d51121   usdt     18     18    100.0   43.537270   59.473667   935.425635  710.767451
  0xf867ca539dbb9d5304   usdt     16     16    100.0   25.585369   51.442607   498.386547  378.690643
  TONG CUM DOI THU: n=58 co_shadow.sim=58 victim_ok_evm=58 tong_profit_sim=2962.195187 USDT (2250.774240 USDT/gio)

---- NGOAI CUM: 1 candidate Simulated ----
  0x13a9dafa6c65c88f53   wbnb      1      1    100.0    0.009956    0.009956     0.009956 BNB (0.007565 BNB/gio)
```

**ĐỌC ĐÚNG — 5 điều kiện phải nói kèm, không được bỏ:**

1. **98,3 % (58/59) cơ hội có lãi là ví burner của CHÍNH cụm đối thủ.** Ngoài
   cụm chỉ đúng 1 dòng, lãi 0,00996 BNB (≈ 0,01 USD). Đây là xác nhận mạnh
   hơn con số 481/497 của BAOCAO43.
2. **`victim_quote_topped_up = true` cho 75/77 dòng.** Con số lãi vì vậy là
   *"lãi NẾU victim có tiền như lúc họ thật sự chạy"*, KHÔNG phải lãi đã xác
   nhận trên state thật của block fork.
3. **`front_gated = 2999,995 USDT` ở gần như mọi dòng** = chạm trần
   `max_front_usdt = 3000`. Tức số lãi này đòi **3000 USDT vốn xoay mỗi lần**.
4. **`profit_v2` và `profit_sim` khớp nhau rất sát** (59,32 vs 59,47 / 50,31 vs
   51,18 / 212,08 vs 221,19). Đây là lần đầu công thức đóng V2 của đường nóng
   được EVM xác nhận trên mẫu lớn — `sim_engine="v2"` là lựa chọn đúng.
5. **`room` = 1,50 … 1,89** — `amountOutMin` của cụm đặt rất lỏng (họ chấp nhận
   trượt tới ~40%). Đây chính là lý do kẹp được: slippage tolerance của họ rộng.

`gas` KHÔNG nằm trong `profit_sim` (sim đặt `gas_price = 0`). Gas thật đã trừ
trong `profit_v2` (F-03) và 2 số vẫn khớp, nên sai lệch do gas nhỏ.

---

### MỤC 3 — ĐƯỜNG GỬI CỦA CỤM (giả thuyết "private" BỊ BÁC BỎ)

File đầy đủ: `baocao/evidence/baocao45_muc3_duong_gui.txt`.

```
dau do mempool (mempool_probe, WSL, subscribe DUNG BSC_WS bot dung, KHONG loc gi):
  646897 hash duy nhat, 0 lan lagged, cua so 08:51:22Z -> 10:14:42Z
  moc block tu rpc.block cua bot: 1366 block, 122192701..122194066

tx cum on-chain trong cua so quet: 554  |  trong do co CA moc block cua bot: 119

-- TOAN BO tx cum trong cua so (n=119) --
   theo loi vao: {'to_cluster': 77, 'from_seed': 18, 'from_funded': 24}
   dau do THAY trong mempool  = 53 (44.5%)
   dau do KHONG thay          = 66 (55.5%)
   thay truoc luc block len (ms): n=53 p10=-206 p50=376 p90=50878 min=-3776 max=63300

-- DOI CHUNG (bat buoc): dau do thay bao nhieu % MOI tx cua 30 block ngau nhien --
   tong tx trong 30 block = 3189
   dau do THAY            = 1546 (48.5%)
```

**Kết luận bằng số: 44,5 % (cụm) so với 48,5 % (mặt bằng chung) — tx của cụm
KHÔNG kín đáo hơn tx thường. KHÔNG có bằng chứng họ gửi private.** Nếu chỉ nhìn
`tx.seen` của bot thì tỉ lệ ra 17,6 % và rất dễ kết luận sai; nhóm đối chứng là
thứ bác bỏ nó.

**Lý do THẬT bot bỏ sót cụm**: **77/119 (65 %) tx của cụm đi tới contract riêng
của họ** (`tx.to = 0x8180…`, selector `0xecb65f51`) nên bị chặn ngay ở gate đầu
tiên `not_pancake_router`. Chỉ nhánh `from_funded`
(`swapExactTokensForTokens` tới V2 Router đã pin) là bot decode được.

`p90 = 50 878 ms` và các giá trị âm KHÔNG đọc được — mốc `rpc.block` của lần
chạy WSL đó bị hỏng giữa chừng bởi BUG #9 (xem dưới). Số dùng được là **p50 =
376 ms sớm hơn lúc block lên**, đủ để front-run. Đo lại mốc này sạch là CÒN NỢ.

Gas price KHÔNG dùng được làm bằng chứng: **44/44 tx cụm đều `0,05 gwei`**,
nhưng đó đúng bằng mức tối thiểu hiện hành của BSC.

---

### MỤC 4 — AI ĐANG KẸP CỤM

File đầy đủ: `baocao/evidence/baocao45_muc4_ai_kep_cum.txt`.

```
-- Tren TOAN BO 554 tx cum quet duoc --
   co tx khac cham CUNG pool trong +-3 vi tri = 112 (20.2%)
   co dia chi dung CA TRUOC lan SAU tren cung pool = 0 (0.0%)
   dia chi nghi kep: KHONG CO

   Top 8 dia chi hay dung CUNG POOL, LIEN KE tx cum:
     0x287f83ac94fb367d6c9af6894a30f0aeceb77237  x2 (CHINH CUM)
     0x54e31bc94bf59e334ae1fb1611a5fd24f185a080  x2 (CHINH CUM)
     … ca 8/8 deu la vi cua CHINH cum
```

**0/554 — KHÔNG thấy bot nào đang kẹp cụm này.** Hàng xóm cùng pool hầu hết là
chính các ví của cụm (họ tự xếp nhiều swap liên tiếp trên cùng pool). Giới hạn
phép đo phải ghi rõ: chỉ xét ±3 vị trí và chỉ `Swap` V2 — cú kẹp đặt xa hơn 3
vị trí hoặc qua V3/V4 sẽ KHÔNG bị bắt. Vì thế đây là **"không thấy"**, không
phải **"chắc chắn không có"**.

---

### MỤC 6 — p95: TÌM RA NGUYÊN NHÂN VÀ SỬA

**TRƯỚC** (WSL, commit `cc285d7`, 11 650 quyết định —
`baocao/evidence/baocao45_p95_truoc.txt`):

```
p50=0.01 p90=426.73 p95=599.33 p99=736.38 max=2540.29 (ms)
reason cua 200 mau cham nhat: [('not_in_list', 177), ('sell_direction', 11), ('unprofitable', 10), ...]
```

**177/200 mẫu chậm nhất là `not_in_list`** — bot đã trả tiền `getPair` +
`getReserves` rồi mới phát hiện token không có trong `pairs.txt`. Ở ship mode-2
kết cục của những tx đó LUÔN là `not_in_list`.

**SAU** (WSL, commit `ca0a644`, 70 phút, 62 745 quyết định —
`baocao/evidence/baocao45_p95_sau.txt`):

```
n=62745 p50=0.011 p90=0.025 p95=0.126 p99=410.972 max=2897.0 (ms)
reason toan bo: {'decode_fail': 27479, 'sell_direction': 19058, 'not_in_list': 11105,
                 'unprofitable': 2417, 'not_quote_pair': 1640, 'victim_would_revert': 506,
                 'venue_unpinned': 434, 'simulated': 103, 'deadline': 3}
so quyet dinh > 1 ms = 2700 (4.30%) -> day la nhung tx THAT SU phai goi RPC
  trong nhom do: p50=239.0 p95=847.8 max=2897.0 ms
```

**p95 599 → 0,126 ms, ĐẠT mốc ≤350 ms của lệnh.** Nhưng phải đọc kèm câu này,
không được giấu: **p95 thấp như vậy vì 95,7 % quyết định nay KHÔNG chạm RPC
nữa. Trong 4,3 % thật sự đi tới sim, p95 vẫn là 848 ms.** Hai con số đo hai
thứ khác nhau; con số có ý nghĩa cho cuộc đua là con số thứ hai.

**Hiệu quả đo được ở chỗ quan trọng hơn** — `latency.decision_vs_mined` trên
VPS (77 mẫu):

```
   delta  -1 : 49
   delta  +0 : 28
  quyet dinh TRE (delta>0) = 0/77 (0%)    [BAOCAO44 do duoc 18%]
```

**Quyết định trễ: 18 % → 0 %.** Và cả 77 dòng `shadow.sim` đều có
`mined_block = null` (bot quyết định TRƯỚC khi victim lên block).

---

### MỤC 1 — 3 PHẦN CÒN NỢ CỦA BAOCAO44

**(a) 60 dòng `mem.rss_mb` (WSL): ĐỦ — 71 dòng**
(`baocao/evidence/baocao45_mem_rss.txt`):

```
     0s rss=     8.00 peak=     8.12 d=       ? | reserve=0 seen=0 mined=0 rate_buckets=0
    60s rss=    17.32 peak=    17.38 d=   9.324 | reserve=0 seen=637 mined=3305 rate_buckets=2
   120s rss=    25.70 peak=    25.70 d=   8.375 | reserve=14 seen=1582 mined=3119 rate_buckets=3
   180s rss=    28.20 peak=    28.20 d=   2.500 | reserve=15 seen=2529 mined=3227 rate_buckets=4
   …
  2040s rss=    41.66 peak=    42.34 d=   0.125 | reserve=20 seen=32401 mined=3477 rate_buckets=39
  2100s rss=   289.91 peak=   318.53 d= 248.250 | reserve=16 seen=33345 mined=3365 rate_buckets=40  <-- 1 loi goi /api/econ
  2160s rss=   290.03 peak=   318.53 d=   0.125
  …
  4200s rss=   293.16 peak=   318.53 d=   0.125 | reserve=12 seen=50000 mined=3171 rate_buckets=79
```

Bỏ đúng 1 lời gọi `/api/econ` ra thì **RSS đi ngang**: 41,53 MB lúc 1920 s →
41,66 MB lúc 2040 s. Mọi container đứng yên trong lúc RSS nhảy 248 MB.

**(b) Đo lại cú nhảy `/api/econ` sau `ECON_EVENTS_CAN_DUNG`: BẢN SỬA CỦA
BAOCAO44 KHÔNG ĂN** (`baocao/evidence/baocao45_muc1b_econ_mem.txt`, CÙNG MỘT
file log 30 087 622 byte cho cả 2 lần đo):

```
TRUOC (binary c2ac1021, da co ECON_EVENTS_CAN_DUNG cua BAOCAO44):
  rss 41.66 -> 289.91 MB (dinh 318.53)  = +248.25 MB, 0.790 s, KHONG tut lai
SAU (binary 71bdca78, prune_econ_row + String::from_utf8 khong copy):
  rss 21.32 -> 112.00 MB (dinh 140.85)  = +90.68 MB, 0.341 s
  loi goi THU HAI: 112.00 -> 209.58 MB  = +97.58 MB nua
```

Giảm **2,74 lần** (248 → 91 MB), thời gian 0,79 → 0,34 s. **CÒN NỢ**: mỗi lần
gọi vẫn cộng thêm ~90–98 MB và không trả lại OS. Vẫn là O(kích thước log);
ngoại suy sang log 366 MB thì 1 lời gọi vẫn tốn ~1,1 GB (trước là ~3 GB).

**(c) VPS + verify BUG #4/#5 sống** (`baocao/evidence/baocao45_muc1_vps_bug45.txt`,
cửa sổ 08:50:55 → 10:28 UTC = **1,62 h**, KHÔNG phải 6 h — Chủ chốt sớm):

```
shadow.sim = 77 dong
  con TRANSFER_FROM_FAILED = 0        (BAOCAO44 de nghi: phai = 0)   -> DUNG
  victim_ok=true           = 77 / 77
  decision_block - fork_block: {1: 77} (BAOCAO44 de nghi: luon = 1)  -> DUNG
  victim_quote_topped_up=true = 75 / 77
mem.rss_mb tren VPS: 98 dong; dau 5.71 MB -> cuoi 72.50 MB (peak 72.50 MB), uptime 5820 s
  tu moc 10 phut (600s, 49.65 MB) toi cuoi: tang 22.85 MB trong 87 phut
systemctl: ActiveState=active MainPID=392224 NRestarts=0 MemoryCurrent=9789440 (sau restart cuoi)
```

RSS cuối 72,50 MB sau 1,62 h, tăng 22,85 MB trong 87 phút kể từ mốc 10 phút —
so với ~11 MB/phút lúc OOM ở BAOCAO43 thì đây là **0,26 MB/phút**, giảm ~42
lần. Chưa đủ dài để tuyên bố hết rò rỉ.

---

### MỤC 5 — RỦI RO PHẢN ỨNG CỦA CỤM

Đã ghi vào `docs/STATE.md` (mục 5 của phần `verify-cluster-as-victim`): 3 kịch
bản (chuyển private / đổi ví seed / đổi pool) đều hiện ra với bot dưới CÙNG
một dấu hiệu — số candidate nhận diện được là của cụm mỗi giờ tụt mạnh.

Code: `competitor::ClusterRateWatch` + `main.rs::cluster_rate_watch_task` —
đếm theo từng phút (trần cứng 180 bucket), mỗi 5 phút so cửa sổ 60 phút gần
nhất với 60 phút liền trước, ghi `competitor.alert` khi tụt **> 80 %**. Chống
báo giả: đường nền ≥ 20 candidate + cooldown 60 phút. Sự kiện luôn kèm
`cur_total_60m`/`prev_total_60m` để phân biệt "cụm bỏ đi" với "bot mất WS".
Đọc được ở `GET /api/compete` khối `cluster_rate`; container có trần trong
`GET /api/mem` (`competitor::ClusterRateWatch.buckets`, cap 180 phút). 6 test.

Trong cửa sổ đo: `competitor.alert = 0` (cụm vẫn hoạt động bình thường) —
đúng như mong đợi, và là bằng chứng cổng không báo giả.

---

### 4 BUG THẬT lộ ra giữa phiên

**BUG #6 — `shadow.sim` fork tại block victim ĐÃ đào khi bot quyết định trễ.**
BUG #4 của BAOCAO44 đổi fork sang `current_block − 1`, nhưng khi
`decision_block > mined_block` (BAOCAO44 đo 18 %) thì đó VẪN là block victim đã
đào. Đo thật: 5/5 victim USDT có `fork_block` bằng ĐÚNG `mined_block`. Sửa: tra
`mined_index` (0 RPC) rồi fork tại `min(decision_block, mined_block) − 1`.

**BUG #7 — ví burner của cụm được cấp vốn trong CHÍNH block đào.** Trạng thái
"đã cấp vốn, chưa swap" chỉ tồn tại GIỮA hai tx trong cùng một block;
`AlloyDB` đọc state ở CUỐI block nên không bao giờ chạm tới. Sửa:
`run_sandwich_quote_topup` nạp cho victim đúng lượng quote họ sắp tiêu.
Allowance KHÔNG bị ghi đè.

**BUG #8 — `competitor.rs` ghi ĐỊA CHỈ POOL vào cụm.** 3 seed không chỉ cấp
vốn, chúng còn TỰ SWAP; khi đó router gọi `transferFrom(seed → POOL, amountIn)`
sinh đúng log `Transfer` mà bộ lọc cụm bắt. Đo thật: `0xdfe23efb…`,
`0xf867ca53…`, `0xcec13213…` đều nằm trong `competitor.funded` nhưng cả 3 là
POOL trong `pairs.txt`. Sửa: bỏ qua địa chỉ nhận là pool bot đã biết.

*Giới hạn KHÔNG sửa được bằng code*: cờ `victim_in_competitor_cluster` LUÔN
`false` cho mẫu hình "cấp vốn + swap trong cùng block" (lúc bot quyết định,
block đó chưa đào). Đo thật: **bot nhận diện 6/58, bỏ sót 52**. Vì vậy mọi
phân tích "% cơ hội là của cụm" phải phân loại ngoại tuyến từ quét on-chain.

**BUG #9 — `subscribe_ws_heads` bỏ cuộc VĨNH VIỄN khi kênh lag.** Lúc 09:02:54
subscription rớt với `channel lagged by 8`, hàm ghi `rpc.skip` rồi `return`.
Từ đó `rpc.block` ngừng hẳn (dừng ở 1366 dòng) trong khi bot vẫn chạy và
dashboard vẫn xanh; `last_block` chỉ còn được cập nhật bởi
`http_pool_health_check`. Sửa: vòng ngoài thử lại mãi + `break` thay `return`
+ `channel_size(PENDING_WS_CHANNEL_SIZE)`. VPS trong cửa sổ đó KHÔNG dính
(`rpc.block` = 7211 dòng, cập nhật tới lúc kiểm tra — đã đối chiếu, không suy diễn).

---

### `cargo test --release` (WSL, HEAD `2591b18`)

```
test result: ok. 399 passed; 0 failed; 18 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

417 pass (399 lib + 18 main), 18 ignored, 0 failed. **Sửa một con số của
BAOCAO44**: `#[test]` trùng trên `compute_econ_net_pos_luon_kem_2_cong_victim_ok`
khiến cargo ĐĂNG KÝ test đó HAI LẦN (đối chiếu
`cargo test --lib -- --list` tại `abb352a`: tên đó xuất hiện 2 dòng), nên
"399 passed" của BAOCAO44 đếm thừa 1. Số test lib THẬT ở đầu phiên là 409, cụm
này thêm 8 (6 `ClusterRateWatch` + 1 `real_rpc` + 1 `prune_econ_row`) = 417.

`real_rpc_*` ĐÃ CHẠY THẬT với output RPC thật dán kèm (luật #3), gồm cả một
lần trả `MISSING` đúng nghĩa khi ngân sách quét 60 block không gặp mẫu hình.

## 6. CHAIN — `0x38`

- `real_rpc_cluster_burner_victim_song_lai_khi_duoc_nap_von`: `eth_chainId`
  assert `= 56`; fork THẬT tại block 122194456; V2 Router pin
  `0x10ED43C718714eb63d5aA57B78B54704E256024E`, USDT `0x55d398…7955`.
- `eth_getBlockReceipts` THẬT của 5 block đào (122188886 / 122189853 /
  122190163 / 122190311 / 122189837) — số dán ở mục 2.
- `scripts/cluster_tx_scan.py` + `cluster_check_hashes.py`: assert
  `eth_chainId == "0x38"` trước khi quét; 554 tx cụm / 5 989 block; 59/59 hash
  VPS tra được on-chain.
- `mempool_probe`: `eth_chainId = 56 (0x38)`, 1 525 664 hash, 0 lần lagged.
- Bot WSL + VPS: `chain_id=56 dry_run=true allow_live=false bot_armed=false`
  (dán nguyên văn dòng boot của systemd ở mục 5).
- Pin V2/V3/V4/WBNB/USDT KHÔNG đổi ở cụm này.

## 7. REGISTRY

`DEX_REGISTRY.md` **KHÔNG đổi** — cụm này không pin venue/địa chỉ mới.

## 8. KHÔNG LÀM

- Không viết/deploy contract, không gửi tx/bundle, không bật live.
- Không đụng `CLAUDE.md`, `pairs.txt`, `config.toml` ship, `PRIVATE_KEY`.
- Không dùng subagent (luật #4).
- Không xoá log: VPS giữ `logs/bot.jsonl.24h_baocao43` (10,92 h) và
  `logs/bot.jsonl.baocao45_window` (cửa sổ đo của cụm này, 109 548 dòng).
- Không chạy đủ 6 giờ VPS — **Chủ ra lệnh chốt sớm lúc 09:57 UTC**, ghi rõ
  cửa sổ thật là 1,62 h.
- Không kết luận chiến lược thay Chủ. Không bịa số lãi: mọi con số đi kèm
  `victim_quote_topped_up` và điều kiện vốn 3000 USDT.

## 9. CHỮ: CHỜ GROK

Lý do: cả 6 mục của lệnh đều có output thật dán kèm và có file evidence.
Phần duy nhất KHÔNG đạt đúng chữ của lệnh là **cửa sổ VPS 1,62 h thay vì
≥ 6 h** — do Chủ ra lệnh chốt sớm giữa phiên, không phải do thiếu sót kỹ
thuật. Mọi đòi hỏi còn lại của `ĐẠT CẦN DÁN` (bảng mục 2 theo pool, mục 3–4
bằng số, 3 phần BAOCAO44, p95, git HEAD + sha256 hai máy) đều đã có.

## 10. CÒN NỢ / LÁT SAU

1. **Cửa sổ VPS mới 1,62 h, lệnh yêu cầu ≥ 6 h.** Bot đang chạy tiếp ở commit
   `2591b18` với `--shadow --allow-competitor-victims`; phiên sau chỉ cần chạy
   lại `scripts/analyze_cluster_econ45.py` + `cluster_check_hashes.py` trên
   `/root/bsc-sandwich/logs/bot.jsonl` là có bảng mục 2 trên cửa sổ dài.
2. **`/api/econ` vẫn O(kích thước log)** — giảm 2,74 lần nhưng mỗi lời gọi vẫn
   +90…98 MB và không trả lại OS. Bước triệt để (cộng dồn theo dòng, không
   dựng `Vec<Value>`) CHƯA làm. Đây vẫn là đường dẫn tới OOM trên log lớn.
3. **Mốc "sớm hơn block bao nhiêu ms" của mục 3 chưa sạch** — lần chạy WSL đó
   dính BUG #9 nên `rpc.block` hỏng giữa chừng; p50 = 376 ms dùng được, p90
   KHÔNG. Chạy lại 60 phút trên binary đã sửa BUG #9 là có số sạch.
4. **p95 của nhóm THẬT SỰ đi tới sim vẫn 848 ms** (4,3 % số quyết định). Con số
   này mới là con số của cuộc đua; chưa tối ưu.
5. **Mục 4 chỉ xét ±3 vị trí và chỉ `Swap` V2** — cú kẹp xa hơn hoặc qua V3/V4
   sẽ không bị bắt. Kết luận đúng là "không thấy", không phải "không có".
6. **Số lãi mục 2 phụ thuộc `victim_quote_topped_up`** — muốn con số không có
   giả định nào thì phải mô phỏng ở mức TRONG BLOCK (áp lần lượt các tx trước
   victim), chưa làm.
7. **Vốn 3000 USDT/lần và khả năng chen vào trước victim CHƯA được kiểm** —
   victim được cấp vốn ở `idx i` và swap ở `idx i+1`; ta phải đứng ở `idx < i`.
   Đây là câu hỏi cho cụm 6 (`strategy-exec`), không trả lời được bằng sim.
8. **Tỉ lệ THẮNG cuộc đua: vẫn MISSING** (nợ cũ).
9. **Đường `sim_engine="evm"` vẫn dùng trần gas cấu hình** (nợ cũ).
10. **Thang ladder giới hạn trong `pairs.txt`: chưa chạy** (nợ cũ BAOCAO44).

Commit: `ae4efea4acb15033df211652a608a510ff77999f` (+ 1 commit bổ sung cho
chính dòng này). Commit `2591b18` là commit CODE cuối; `ae4efea` chỉ thêm
`baocao/BAOCAO45.md` + `docs/TASKS.md`, không đổi một dòng Rust nào — binary
đang chạy trên VPS (`2f77040a349fc874f07a2b69d8a1f6c78904e27af0f59ee61b3a7b65e7e69b05`)
và trên WSL (`71bdca78544b91758783c524362ad8ad8ef678c413736ea146852be00f671ac0`)
vì vậy vẫn đúng với HEAD.

**Trạng thái lúc đóng phiên**: hai máy CÙNG commit `ae4efea`; VPS
`ActiveState=active MainPID=392224 NRestarts=0`, đang chạy shadow +
`allow_competitor_victims=true` để tích cửa sổ dài cho phiên sau;
`git status --short` rỗng trên WSL.
