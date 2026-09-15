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
| `evm-validate-fixed-then-wire` cụm B3+C+D + `wsl-env-rules-paperrun` | 33, 34 | 6d50a31 (33) / e24a834 (34) | **Backfill (cụm `real-economics-mode2`, đọc trực tiếp BAOCAO33/34):** BAOCAO33 sửa bug decoder thật (thêm 3 selector `*SupportingFeeOnTransferTokens` — decoder cũ `decode_fail` 100% mempool BSC thật vì mọi tx V2-buy dùng nhóm này), nối `sim_evm` vào pipeline (`decide_with_evm`/`BlockForkCache`), C1 đo tax thật bằng revm (`measure_tax_evm`, phát hiện 1 honeypot thật `TRANSFER_FROM_FAILED`), C3 `ZERO_TAX_ALLOWLIST` 8 token verify on-chain, B3.4 validator nhúng `/api/validate`. B4''.2 (dự đoán victim khớp on-chain) ĐẠT 9/9=100% lệch 0.000000%. B4''.3 (replay ≥3 sandwich thật lệch ≤2%) KHÔNG đạt số (1/3 replay được) vì 2 giới hạn hạ tầng thật: cửa sổ state ~128 block của RPC công khai, và 28/28 bộ sandwich thật tìm được đều route qua CONTRACT riêng (không phải EOA) nên `profit_real` đo bằng EOA-balance-delta ra `0 vs 0` vô nghĩa — đây chính là bằng chứng sớm nhất trong repo cho kết luận sau này của audit ("EOA + 2 router call là mô hình đã bị đào thải", F1). BAOCAO34 xác nhận môi trường WSL, thêm 3 luật BAOCAO (#1/#2/#3), tạo `scripts/paper_run.sh` dùng chung WSL/VPS, smoke test 0 phút (chưa chạy 30 phút thật, chưa chạy `real_rpc_*` — ghi CÒN NỢ đúng lúc đó). **Hướng "cụm C: EVM mỗi tx trên đường nóng" của B3+C+D này đã bị `strategy-lock-mode2` THAY THẾ**, xem "Hoãn, lý do". |
| Audit toàn diện trước live | AUDIT_2026-09-15 | 6d50a31 | XONG (27 lỗi code F-01..F-27 + 12 lỗi vận hành V-01..V-12) |
| `exec-path-traps` (chặn 12 bẫy thực thi trước signer `7.3`) | 35 | 3694908 | XONG 12/12 + mục 13 (sửa `paper_run.sh`) |
| `strategy-lock-mode2` (Chủ chốt mode 2 only) | 36 | 5675f81 | XONG |
| `docs-cleanup-mode2` (dọn tài liệu vận hành cho mode 2) | 37 | bfd992b | XONG |
| `real-economics-mode2` cụm B (fix bug tax-gate BAOCAO37, F-03 gas thật, `/api/econ`, F-27 validator, nonce_future test) | 38 | *(phiên này, xem BAOCAO38 ô 3)* | MỘT PHẦN — xem "Nợ CÒN THẬT" dưới cho danh sách chưa làm (gas thật cho `sim_engine="evm"`, V4 sim, decoder-coverage...) |

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

- **BUG cổng tax pair-mode — ĐÃ SỬA** (cụm `real-economics-mode2`, BAOCAO38):
  `honeypot_or_tax=95/phút`/`unprofitable=0` phát hiện ở BAOCAO37 do đường
  nóng tra nhầm `TaxCache` (luôn rỗng) cho token đã vet tay trong `pairs.txt`.
  `PairBook::is_tax_ok` + `pipeline::evaluate_candidate(skip_tax_gate)` bỏ
  qua `TaxCache` khi pool đã vet + chưa bị vet nền loại; pool `vet_failed`
  giờ route rõ ràng vào `honeypot_or_tax` (detail `"vet_fail"`) thay vì rơi
  im lặng xuống `not_in_list`.
- **F-27 (validator `/api/validate`) — ĐÃ SỬA**: bộ đếm `within_1pct` cũ chỉ
  tính dòng `isolated:true` (audit: trả `2` trong khi đếm tay ra `12/18`).
  `web::ValidateStats`/`ValidateGroupStats` giờ tách riêng 2 nhóm
  (`isolated`/`non_isolated`), mỗi nhóm có `n`/`within_1pct`/`p50_lech_pct`/
  `p95_lech_pct`; tổng gốc `within_1pct` giờ cộng đúng cả 2 nhóm.
- **nonce_future — có test cơ chế, CHƯA wire vào đường nóng `sim_engine="v2"`**:
  `transport::compare_nonce`/`NonceCache` đã có test kịch bản đầy đủ
  (`nonce_future_then_ok_after_k_confirms_same_sender`, BAOCAO38), nhưng gate
  nonce (F-13) hiện CHỈ được gọi trong `main.rs::run_evm_decision`
  (`sim_engine="evm"`) — đường nóng mặc định (`sim_engine="v2"`,
  `decide_paper_v2`/`evaluate_candidate`) KHÔNG kiểm nonce victim. Cần lệnh
  riêng nếu Chủ muốn thêm gate này vào đường nóng v2 (thêm 1 `eth_call`
  `eth_getTransactionCount` mỗi candidate trước khi sim).

- **F-03 (gas thật) — XONG cho đường nóng `sim_engine="v2"`** (cụm
  `real-economics-mode2`, BAOCAO38): `pipeline::compute_gas_cost_wei` dùng
  `max(eth_gasPrice qua GasOracle, gas_price của chính victim)` × gas unit đo
  1 lần lúc boot bằng revm (`sim_evm::measure_gas_units`, fallback config
  `gas_units_front`/`gas_units_back` nếu đo lỗi) — `front_max_gas_bnb_wei`/
  `back_max_gas_bnb_wei` giờ CHỈ còn là TRẦN (skip `gas_cap` khi vượt), không
  còn dùng thẳng làm chi phí trừ vào profit. **CÒN NỢ**: đường `sim_engine="evm"`
  (`main.rs::run_evm_decision`/`pipeline::decide_with_evm`, KHÔNG phải đường
  nóng mode 2 mặc định) VẪN dùng `cfg.gas_wei()` (trần) làm chi phí trực
  tiếp như cũ — chưa nối `GasOracle`/gas unit đo thật vào đường này (chỉ dùng
  cho vet nền/pre-sign/validator/đối chiếu thủ công, không phải hot path).
- **F-03 mục 1.d (gas USDT) — XONG một phần**: `evaluate_candidate_quote`/
  `decide_paper_quote` nhận `gas_cost_in_quote_wei` đã quy đổi sẵn (BAOCAO38),
  `main.rs` quy đổi qua `pipeline::convert_gas_cost_bnb_to_usdt` (reserve
  WBNB/USDT thật). Chưa verify bằng `real_rpc_*`/paper run thật với
  `scan_quote_usdt=true` (ship `false`, nhánh USDT không phải hot path).
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
