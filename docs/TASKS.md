# docs/TASKS.md — Roadmap & trạng thái cụm

Nguồn roadmap gốc: `CLAUDE.md` mục "Roadmap — được làm cùng lúc". File này
chỉ theo dõi cụm nào đã xong / còn nợ, không thay thế luật trong CLAUDE.md.
Mô tả kỹ thuật đầy đủ từng cụm (quyết định, số liệu, code path) nằm ở
`docs/STATE.md` (mục cùng tên cụm) — bảng dưới đây chỉ tóm tắt trạng thái.

Lưu ý về cột **Commit**: lịch sử git trước `BAOCAO34` đã bị gộp vào 1 commit
baseline (`6d50a31`, "Baseline sau BAOCAO33 + audit 2026-09-15") khi audit
độc lập bắt đầu — vì vậy mọi cụm từ `0.1` tới `BAOCAO33` đều chỉ có chung 1
hash `6d50a31`, không có hash riêng từng cụm (không phải thiếu sót ghi
chép, mà là giới hạn lịch sử git thật của repo).

## Bảng cụm

| Cụm | BAOCAO | Commit | Trạng thái |
|---|---|---|---|
| 0.1 Khung Rust + load config | 01 | 6d50a31 | XONG |
| 0.2 VictimBook | 01 | 6d50a31 | XONG |
| 0.3 State + logger + web dashboard | 01 | 6d50a31 | XONG |
| 1.1 Pin V2 + WBNB + factory + router | 02 | 6d50a31 | XONG |
| 1.2 Pin V3 | 02 | 6d50a31 | XONG |
| 1.3 Pin V4/Infinity + family mới hơn | 02 | 6d50a31 | XONG (Infinity pinned; "mới hơn" DISABLED — chưa deploy trên BSC) |
| 2.1 HTTP/WSS thật | 03 | 6d50a31 | XONG |
| 2.2 Decoder router Pancake đã pin | 03 | 6d50a31 | XONG (phạm vi hẹp ban đầu — mở rộng dần ở các cụm sau, xem nợ `decoder-coverage`) |
| 2.3 Resolve pool token/WBNB | 03 | 6d50a31 | XONG (V2/V3 qua `eth_call` thật; V4/Infinity ban đầu `hooks_unread` mọi token) |
| `v4-pool-resolve` (quét THẬT `Initialize` event V4/Infinity) | 19 | 6d50a31 | MỘT PHẦN — công thức `PoolId` verify đúng, chưa tìm được ví dụ pool ghép WBNB thật để verify nhánh "tìm thấy" |
| 3.1 V2 math + search front | 04 | 6d50a31 | XONG |
| 3.2 Victim still ok | 04 | 6d50a31 | XONG |
| 3.3 Sim V3 (quoter đã pin) + tax stub | 04 | 6d50a31 | XONG (V3 qua QuoterV2 thật; tax stub không phát hiện được fee-on-transfer thật — đã đóng thay thế sau ở `foundation-fix-then-real-sim`) |
| 4.1 Pipeline paper | 04 | 6d50a31 | XONG (phạm vi: chỉ chiều victim MUA token bằng WBNB) |
| `config-hot-reload` | 05 | 6d50a31 | XONG |
| 5.1 Chạy ngắn, 0 sendRaw | 06 | 6d50a31 | XONG |
| `tax-cache-inject` | 07 | 6d50a31 | XONG |
| 5.2 Paper vận hành: exposure cap, gas warning, pending fallback | 08 | 6d50a31 | XONG |
| 5.3 RPC pool đa URL (failover) + pending bền hơn | 09 | 6d50a31 | XONG |
| `rpc_probe` bin đo RTT/chain_ok | 10 | 6d50a31 | XONG (đo VPS thật ở BAOCAO12) |
| Pair-mode (`PairBook` + `decide_paper_v2`) | 13, 14 | 6d50a31 | XONG (chiều victim BÁN đã XOÁ hẳn, xem "Hoãn, lý do") |
| 7.1 Live gate + signer | 13 | 6d50a31 | XONG (chỉ gate/load key, không ký/gửi) |
| `deploy_vps.sh`/`.ps1` (`.ps1` đã XOÁ ở `docs-cleanup-mode2`) | 11, 12 | 6d50a31 | XONG script + deploy THẬT thành công lên VPS 1 lần |
| 7.2 Pin calldata | 15 | 6d50a31 | XONG |
| 7.3 Executor (gửi tx thật) | 16, 17, 18 | 6d50a31 | MỘT PHẦN — build+log paper-mode nối dây xong vào live loop; **gửi tx thật (sendRaw) CHƯA LÀM** |
| `universal-pair-scan` (mode 3) | 20 | 6d50a31 | XONG về code, **HOÃN theo `strategy-lock-mode2`** (xem "Hoãn, lý do") |
| `relay-bundle-builder` | 21, 23, 24 | 6d50a31 | MỘT PHẦN — build request đúng schema 2 relay, CHƯA nối `pipeline.rs`/`executor.rs`, cần signer thật (`7.3`) |
| `relay-finalize` | 24 | 6d50a31 | XONG |
| `explicit-mode-flags` | 22 | 6d50a31 | XONG (giá trị ship `wallet_scan_enabled` sau đó đổi ở `strategy-lock-mode2`) |
| `pairs-discovery` (điền `pairs.txt` 100 token thật) | 25 | 6d50a31 | XONG (nội dung sau đó được Chủ tự vet lại ở `strategy-lock-mode2`) |
| `vps-2phase-dryrun` | 26 | 6d50a31 | XONG |
| `vps-lowthreshold-retest` | 27 | 6d50a31 | XONG (phát hiện Alchemy WS không stream pending BSC) |
| `vps-latencyprobe-retest` | 28 | 6d50a31 | XONG (phát hiện BSC_WS local chết hẳn) |
| `usdt-quote-asset` | 29 | 6d50a31 | MỘT PHẦN — hàm quote-aware sẵn sàng, CHƯA nối vào `main.rs` live loop lúc đó (nối ở cụm sau) |
| `quote-live-wiring-funnel-diagnostics` | 30 | 6d50a31 | MỘT PHẦN — (2)(3)(4) xong; (1) CLAUDE.md diff BLOCKED (thiếu nội dung); (5) VPS BLOCKED (không có SSH) |
| `foundation-fix-then-real-sim` cụm A (sửa nền) | 31 | 6d50a31 | XONG |
| `foundation-fix-then-real-sim` cụm B1+B2 (sim EVM qua revm) | 31 | 6d50a31 | MỘT PHẦN — cơ chế đúng, verify RPC thật; CHƯA nối vào pipeline (đó là ý định cụm C, xem "Hoãn, lý do") |
| `foundation-fix-then-real-sim` cụm B4 (validate 3 sandwich thật qua BscScan) | 31 | 6d50a31 | BLOCKED — sandbox không có BscScan API/trình duyệt |
| `evm-validate-wire-tax` cụm B4' (validate tự động thay BscScan) | 32 | 6d50a31 | KHÔNG ĐẠT SỐ — cơ chế đúng (B4'.1/.2/.4 đạt), B4'.3(a)/(b) thiếu mẫu vì RPC công khai rate-limit |
| `evm-validate-fixed-then-wire` cụm B3+C+D + `wsl-env-rules-paperrun` | 33, 34 | 6d50a31 (33) / e24a834 (34) | Nội dung đầy đủ CHƯA được backfill vào bảng này (khoảng trống lịch sử đã biết từ `exec-path-traps`) — xem `baocao/BAOCAO33.md`/`BAOCAO34.md` trực tiếp. **Hướng "cụm C: EVM mỗi tx trên đường nóng" của B3+C+D này đã bị `strategy-lock-mode2` THAY THẾ**, xem "Hoãn, lý do". |
| Audit toàn diện trước live | AUDIT_2026-09-15 | 6d50a31 | XONG (27 lỗi code F-01..F-27 + 12 lỗi vận hành V-01..V-12) |
| `exec-path-traps` (chặn 12 bẫy thực thi trước signer `7.3`) | 35 | 3694908 | XONG 12/12 + mục 13 (sửa `paper_run.sh`) |
| `strategy-lock-mode2` (Chủ chốt mode 2 only) | 36 | 5675f81 | XONG |
| `docs-cleanup-mode2` (dọn tài liệu vận hành cho mode 2) | 37 | *(phiên này, xem BAOCAO37 ô 3)* | ĐANG LÀM |
| `real-economics-mode2` (F-03 gas thật, validator V2, tinh chỉnh vet nền) | — | — | **CHƯA LÀM** |

## Hoãn, lý do (không phải "chưa làm" — có chủ đích, cần lệnh Chủ mới đổi)

- **Mode 1 (wallet-mode, `victims.txt`)** — HOÃN từ `strategy-lock-mode2`
  (2026-09-15). Code giữ nguyên (`wallet_scan_enabled=false`), bật lại chỉ
  cần đổi cờ nếu Chủ ra lệnh.
- **Mode 3 (universal-pair-scan)** — HOÃN cùng quyết định, cùng cơ chế
  (`pair_scan_universal=false`).
- **`fork-actor-perf`** (F-11 fork actor theo block, F-12 backpressure) —
  HẠ ƯU TIÊN xuống SAU cụm 6 `strategy-exec` (đổi từ vị trí ngay sau cụm 2
  trong roadmap gốc). Lý do: đường nóng mode 2 không mở fork EVM mỗi tx
  (`sim_engine="v2"`) nên không còn nghẽn cần tối ưu gấp — fork actor giờ
  chỉ phục vụ 3 việc nền/live (vet định kỳ, đo lại trước ký, validator).
- **Chiều victim BÁN token** — HOÃN VÔ THỜI HẠN, không phải "chưa làm": code
  liên quan (`sim_v2::search_max_front_in_sell` và tương đương ở
  `pipeline.rs`) đã bị XOÁ HẲN ở BAOCAO14 sau khi phát hiện toán học front
  mua trước/back bán sau victim bán luôn lỗ. Kiến trúc đúng cho chiều này
  là back-run (cần tồn kho token/flashloan) — NGOÀI SCOPE "1 signer, cấm
  flashloan" của CLAUDE.md, cần lệnh riêng nếu Chủ muốn làm.
- **Cụm C/D của `foundation-fix-then-real-sim` / `evm-validate-fixed-then-wire`**
  (nối EVM thật vào ĐƯỜNG NÓNG mỗi tx) — HOÃN VĨNH VIỄN, đã bị
  `strategy-lock-mode2` THAY THẾ (không phải song song, xem CLAUDE.md mục
  "Chiến lược đã chốt"). Cơ chế `sim_evm.rs`/revm bên dưới cụm này KHÔNG bỏ
  đi — chuyển sang phục vụ vet nền/pre-sign/validator.

## Nợ CÒN THẬT theo chiến lược mode 2 (cần làm hoặc cần lệnh Chủ)

- **F-03 (gas thật)** — `front_max_gas_bnb_wei`/`back_max_gas_bnb_wei` hiện
  là TRẦN cấu hình tĩnh, cao hơn thực tế 10–100 lần; số `unprofitable`/
  `profit` trên đường nóng V2 chưa dùng được để kết luận kinh tế chính xác
  cho tới khi `real-economics-mode2` sửa (đổi sang `eth_gasPrice` × gas đo
  qua revm nền).
- **V4/Infinity chưa sim được** — `pool.rs::resolve_infinity_pool` tìm ra
  `PoolKey` thật (từ `v4-pool-resolve`) nhưng chưa có hàm nào gọi
  `CLQuoter`/`BinQuoter` để sim giá — việc sim V4 vẫn ngoài phạm vi mọi cụm
  đã làm, là nợ riêng nếu Chủ muốn có sim V4 đầy đủ.
- **`decoder-coverage`** (cụm 4, CHƯA LÀM) — F-09 multicall, SmartRouter
  không deadline, UR đa lệnh, sentinel `CONTRACT_BALANCE`, `payerIsUser`.
  Mục tiêu `venue_v3 > 0`, `decode_fail < 2%`.
- **`test-hygiene`** (cụm 5, CHƯA LÀM) — F-17 `real_rpc_*` không pass rỗng,
  F-22/F-23/F-25 dead code & doc, F-21 redact subdomain.
- **`strategy-exec`** (cụm 6, CHƯA LÀM) — F-01 bundle nguyên tử
  `[front, victim, back]`, F-02 mô hình gas-price/bribe, executor
  contract (sandwich) HOẶC backrun-only. Chỉ bắt đầu sau khi Chủ chốt
  chiến lược bằng số liệu `real-economics-mode2`.
- **`relay-bundle-builder`** chưa nối vào `pipeline.rs`/`executor.rs` — cần
  (1) signer thật ký raw tx (`7.1`/`7.3`), (2) HTTP client thật gửi relay,
  (3) lệnh Chủ riêng cân nhắc rủi ro tiền thật trước khi bật.
- **`7.3` gửi tx thật (sendRaw)** — vẫn CHƯA có hàm ký/gửi nào trong repo,
  chỉ có build+log paper-mode. Cần cụm `strategy-exec` xong trước.
- **`max_consecutive_loss`/`gas_reserve_bnb_wei`** — validate lúc load
  đúng nhưng CHƯA có logic risk-guard nào tiêu thụ 2 field này (chờ
  executor thật `7.x`).
- **`RiskGuard::record_result`** chưa được gọi tự động ở đâu trong paper
  loop (không có giao dịch thật để biết lỗ/lãi thật) — chờ `7.x`.
- **`pairs.txt` — sự cố transient phiên `docs-cleanup-mode2` (BAOCAO37)**:
  một agent nghiên cứu ra ngoài phạm vi ĐỌC-ONLY được giao khiến
  `pairs.txt` bị thấy ở trạng thái revert về `git HEAD` cũ giữa phiên. Cuối
  phiên đã xác nhận LẠI toàn bộ nội dung (đọc hết file + so `git` blob
  hash) — NGUYÊN VẸN, không mất dữ liệu. Khuyến nghị Chủ tự mở `pairs.txt`
  đối chiếu 1 lần cho chắc — xem `baocao/BAOCAO37.md`.
- **Deploy VPS** — bản đang chạy trên VPS (nếu còn) là từ trước
  `exec-path-traps`/`strategy-lock-mode2`/`docs-cleanup-mode2`; cần deploy
  lại + xác nhận `git log -1`/`sha256sum` khớp WSL trước khi coi VPS "đã
  đúng bản mới nhất".
