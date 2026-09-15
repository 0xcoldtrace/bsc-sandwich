# docs/TASKS.md — Roadmap & trạng thái cụm

Nguồn roadmap gốc: `CLAUDE.md` mục "Roadmap — được làm cùng lúc". File này chỉ
theo dõi cụm nào đã xong / còn nợ, không thay thế luật trong CLAUDE.md.

| Cụm | Nội dung | Trạng thái | Phiên |
|---|---|---|---|
| 0.1 | Khung Rust + load config | XONG | BAOCAO01 |
| 0.2 | VictimBook | XONG | BAOCAO01 |
| 0.3 | State + logger + web dashboard | XONG | BAOCAO01 |
| 1.1 | Pin V2 + WBNB + factory + router | XONG | BAOCAO02 |
| 1.2 | Pin V3 | XONG | BAOCAO02 |
| 1.3 | Pin V4/Infinity + family mới hơn | XONG (Infinity pinned; "mới hơn" vẫn DISABLED, chưa deploy) | BAOCAO02 |
| 2.1 | HTTP/WSS thật | XONG | BAOCAO03 |
| 2.2 | Decoder router Pancake đã pin | XONG (phạm vi phiên: V2 Router 3 hàm, V3 SwapRouter exactInput/exactInputSingle, UR 2 command WBNB-path — xem nợ dưới) | BAOCAO03 |
| 2.3 | Resolve pool token/WBNB | XONG (V2/V3 qua eth_call thật; V4/Infinity = `hooks_unread` mọi token, xem docs/STATE.md) | BAOCAO03 |
| — | `v4-pool-resolve`: `resolve_infinity_pool` quét THẬT `Initialize` event (CL+Bin) qua `eth_getLogs`, dựng `PoolKey`/`PoolId` thật (chủ chèn sau `7.3`-nối-dây) | MỘT PHẦN (công thức `PoolId`/decode verify bit-for-bit đúng với log thật trên chain, `#[ignore]` test RPC sống chạy thật xanh — nhưng KHÔNG tìm được ví dụ pool Infinity ghép WBNB nào trong tầm với RPC free-tier phiên này, và block deploy CLPoolManager/BinPoolManager vẫn MISSING nên không có `from_block` mặc định; CHƯA wire vào sim_v3.rs/pipeline.rs [cụm khác], xem docs/STATE.md mục `v4-pool-resolve`) | BAOCAO19 |
| 3.1 | V2 math + search front | XONG | BAOCAO04 |
| 3.2 | Victim still ok | XONG | BAOCAO04 |
| 3.3 | Sim V3 (quoter đã pin) + tax stub | XONG (V3 qua QuoterV2 thật; V4/Infinity vẫn `hooks_unread` — không sim vì chưa pin `PoolKey`; tax stub = cache đúng + hàm eth_call thật nhưng KHÔNG phát hiện được fee-on-transfer thật, xem docs/STATE.md) | BAOCAO04 |
| 4.1 | Pipeline paper | XONG (phạm vi: chỉ chiều victim MUA token bằng WBNB — lõi `decide_paper` thuần/test fixture, CHƯA wire pending-tx thật vào `main.rs`, xem docs/STATE.md) | BAOCAO04 |
| — | Config chỉnh tự do + hot-reload (chủ chèn giữa `4.1` và `5.1`) | XONG (`decide_paper` đọc `Config` trực tiếp, `thin_liq`/`unprofitable`(min_profit)/`honeypot_or_tax`(max_tax) wire xong, `config_reload_sec` hot-reload runtime verify thật, xem docs/STATE.md) | BAOCAO05 |
| 5.1 | Chạy ngắn, 0 sendRaw | XONG (phạm vi: `subscribe_full_pending_transactions` + `state/inject_tx.jsonl` + `decide_paper` wire vào `main.rs`, concurrency ≤4, `/api/skips`/`/api/hits` số thật, verify runtime thật bằng RPC công khai từ `.env` chủ — `BSC_WS` rỗng nên pending thật chưa kiểm chứng được, xem docs/STATE.md) | BAOCAO06 |
| — | Tax cache inject (chủ chèn sau `5.1`) | XONG (`allow_tax_inject` field mới + `POST /api/tax` + `state/tax_inject.jsonl` + `GET /api/tax` bảng web, `TaxCache::inject_from_buy_sell_bps` điểm ghi duy nhất, hot-reload runtime verify thật — vẫn KHÔNG tự động đo tax, chỉ mở đường chủ tự điền tay, xem docs/STATE.md) | BAOCAO07 |
| 5.2 | Paper vận hành: exposure cap + pending không phụ thuộc WSS + web đủ đọc | XONG (`max_exposure_bnb` ăn vào `decide_paper` qua `effective_front_cap_wei` [0=tắt, dương=trần], boot warning `min_profit_bnb` < gas, fallback WSS→`txpool_content`→`inject_only` + field `pending_poll_ms`, `/api/status` có `pending_source`+3 ngưỡng — verify runtime THẬT lần đầu WSS pending thật kết nối được rồi rớt "channel lagged", fallback txpool_content đúng thiết kế, xem docs/STATE.md) | BAOCAO08 |
| 5.3 | RPC pool đa URL (failover) + pending bền hơn | XONG (`transport::RpcPool` round-robin failover HTTP, `subscribe_ws_heads`/`subscribe_pending_txs` nhận danh sách WSS thử lần lượt, `http_pool_health_check` bảo vệ `app_state.provider` cho mọi consumer, `pending_txpool_max_per_poll` [ship 32] chặn spam mỗi vòng poll, `PENDING_WS_CHANNEL_SIZE=4096` nâng buffer subscription, `parse_rpc_url_list` tách CSV cả trong biến gốc [phát hiện thật từ `.env` chủ] — verify runtime THẬT: URL chết cố ý bị skip, URL thật (tách từ chuỗi 34-URL CSV) được chọn, 2 URL private/maxbackrun bị lọc đúng khỏi pool đọc, xem docs/STATE.md) | BAOCAO09 |
| 7.1 | Live gate + signer | XONG (phạm vi: `executor::gate_check`/`LiveGateStatus` [8 điều kiện] + `executor::load_signer` đọc/validate `PRIVATE_KEY` từ `.env` — trả `B256` thay vì `PrivateKeySigner` vì `alloy-signer-local` chưa có bản khớp version đang pin, xem docs/STATE.md. KHÔNG ký/gửi gì, `can_send_live` cũ vẫn giữ nguyên) | BAOCAO13 |
| 7.2 | Pin calldata | XONG (`src/calldata.rs` mới — `encode_front_buy`/`encode_back_sell` dùng `alloy::sol!` [feature `sol-types` bật thêm trên dependency `alloy` đã pin, KHÔNG thêm crate mới, xem docs/STATE.md] cho V2 Router đã pin, roundtrip verify bit-for-bit qua chính `decoder::decode_swap_calldata`. CHƯA wire vào `pipeline.rs`/`executor.rs`, `amount_out_min`=tham số [dùng `0` phiên này, chưa có field slippage], `deadline`=tham số [chưa tự tính `block.timestamp`] — xem docs/STATE.md) | BAOCAO15 |
| 7.3 | Executor (gửi tx thật) | MỘT PHẦN (paper-mode, BAOCAO16, nối dây vào live loop thật ở BAOCAO18): build + LOG 2 tx front-buy/back-sell thật từ `SandwichQuote` qua `pipeline::decide_and_build_paper_v2`/`executor::build_and_log_paper_sandwich`, `deadline` (wall-clock+buffer)/`amount_out_min` (slippage) đều THẬT. `main.rs::handle_paper_tx` (BAOCAO18) ĐÃ đổi sang gọi `decide_and_build_paper_v2` — live loop pending-tx thật (WSS/txpool/inject) giờ sinh `tx.build` thật khi có candidate `Simulated`, verify runtime thật bằng binary boot thật (không chỉ test), xem docs/STATE.md mục `7.3-nối-dây`. **GỬI TX THẬT (sendRaw) VẪN CHƯA LÀM** — không có hàm ký/gửi nào trong repo (grep + test `no_send_raw_transaction_call_anywhere_in_src` xác nhận lại ở BAOCAO18), xem docs/STATE.md | BAOCAO16, BAOCAO17, BAOCAO18 |
| — | Pair-mode (`PairBook` + `decide_paper_v2` dual-branch wallet\|pair, chủ chèn sau `5.3`) | XONG (`PairBook` resolve qua `Factory.getPair` [10 concurrent, timeout 3s], `pairs_min_swap_bnb` global, `decide_paper_v2` ưu tiên wallet > pair > not_in_list, `RiskGuard` wire `max_consecutive_loss`/`gas_reserve_bnb_wei` [chỉ gate, `record_result` chờ `7.x` gọi], `GET /api/pairs`. Chiều victim BÁN token: `decide_paper_v2` GIỜ CHỈ xử lý chiều MUA — chiều bán ĐÃ BỎ HẲN khỏi pipeline ở BAOCAO14 (không phải giữ nguyên/DISABLED), xem docs/STATE.md) | BAOCAO13, BAOCAO14 |
| — | `rpc_probe` bin đo RTT/chain_ok (đứng ngoài roadmap 0.x-7.x, công cụ vận hành) | XONG (lib+bin split, đo từ máy dev VN — CHƯA đo từ VPS NJ thật, xem debt dưới) | BAOCAO10 |
| — | `universal-pair-scan` (chủ chèn sau `v4-pool-resolve`) | XONG (field mới `Config::pair_scan_universal` [bắt buộc, ship `false`], `decide_paper_v2` thêm nhánh thứ 3 wallet>pair>universal>not_in_list, dùng LẠI ngưỡng `pairs_min_swap_bnb` GLOBAL, không thêm eth_call/RPC mới, xem docs/STATE.md) | BAOCAO20 |
| — | `scripts/deploy_vps.sh`/`.ps1` (đứng ngoài roadmap, công cụ vận hành) | XONG script (BAOCAO11) — **deploy THẬT thành công lên VPS NJ thật ở phiên kế tiếp cùng ngày** (Chủ tự cấp IP/password qua chat, không lưu trong repo): source copy qua tar+SFTP (paramiko, không dùng script .sh trực tiếp vì cần password chứ không phải key), rustup+build-essential cài mới, `cargo build --release` xanh, bot chạy nền qua `systemd-run` (khác `nohup` trong script — xem nợ), `rpc_probe` đo THẬT từ VPS (~5-11ms, xem docs/STATE.md) | BAOCAO11, BAOCAO12 |
| — | `relay-bundle-builder` (đứng ngoài roadmap 0.x-7.x, chuẩn bị cho `7.3` sau) | MỘT PHẦN — `src/relay.rs` mới, build THUẦN `serde_json::Value` đúng schema `eth_sendBundle` (48 Club)/`eth_sendMevBundle` (BlockRazor) từ 2 raw tx hex fixture, log preview `bundle.build_preview`, KHÔNG gọi HTTP nào (verify bằng test tự-grep `relay.rs`). CHƯA nối vào `pipeline.rs`/`executor.rs`/live loop, CHƯA có HTTP client thật — hình dạng `params` ĐÃ VERIFY ĐÚNG cho cả 2 relay (BlockRazor ở BAOCAO23; 48 Club ở BAOCAO24 sau khi sửa đúng endpoint) — xem docs/STATE.md mục `relay-bundle-builder` + `relay-schema-verify` + `relay-finalize` | BAOCAO21, BAOCAO23, BAOCAO24 |
| — | `relay-finalize` (chủ chèn sau `relay-schema-verify`) | XONG — (A) `CLUB48_RPC_URL` sửa từ `https://rpc.48.club` (sai, `-32601`) sang `https://puissant-builder.48.club/` (đúng, verify cURL thật trả lỗi nội dung RLP `-32000`, không phải method-not-found), test mới pin giá trị; (B) test 3 `PRIVATE_TX_URL` cá nhân của Chủ — CHỈ 1/3 (endpoint BlockRazor cá nhân) hỗ trợ đúng `eth_sendMevBundle`, 2/3 còn lại (kiểu `48club`/`bsc-rpc.com`) là endpoint privacy-tx-đơn, KHÔNG dùng được cho bundle; (C) toàn bộ comment `config.toml` chuyển sang tiếng Việt có dấu đầy đủ, không đổi value nào — xem docs/STATE.md mục `relay-finalize` | BAOCAO24 |
| — | `explicit-mode-flags` (chủ chèn sau `relay-bundle-builder`) | XONG — `Config::wallet_scan_enabled`/`pair_scan_enabled` (field bắt buộc mới, ship `true`/`true` BẮT BUỘC — giữ nguyên hành vi gốc), `decide_paper_v2` gate 2 nhánh wallet/pair theo 2 cờ này (thứ tự ưu tiên wallet>pair>universal>not_in_list GIỮ NGUYÊN, `pair_scan_universal` không đổi), 9 test mới (toàn bộ 196 test cũ chạy lại không sửa 1 dòng assertion nào) — xem docs/STATE.md mục `explicit-mode-flags` | BAOCAO22 |
| — | `pairs-discovery` (chủ chèn sau `relay-finalize`) | XONG — `pairs.txt` điền THẬT 100 token (token list chính thức PancakeSwap, lọc chain 56, verify TỪNG token qua `pipeline::resolve_v2_reserves` thật qua RPC thật `.env` chủ: 120 pass/319 no_pool/542 thin_liq trên 981 ứng viên), script chạy bằng crate Rust scratch NGOÀI repo (không đụng `src/`/`Cargo.toml`), `cargo test` 206 passed không đổi — xem docs/STATE.md mục `pairs-discovery` (có ghi chú Windows Defender quarantine binary khi concurrency cao, cách né) | BAOCAO25 |
| — | `vps-2phase-dryrun` (chủ chèn sau `pairs-discovery`) | XONG — đẩy code mới nhất lên VPS BAOCAO11/12 (vẫn sống, bot cũ đã chạy liên tục ~4.3h qua đêm), rebuild release (15.12s), chạy dry-run paper 2 pha 5 phút/pha: PHA A (pair-mode đơn độc, 100 pool `pairs.txt`) = 0 candidate khớp trong cửa sổ đo; PHA B (universal-mode đơn độc, hot-reload không restart) = 57 candidate thật đi sâu tới `resolve_v2_reserves` (`below_min=24/honeypot_or_tax=25/thin_liq=8`, đều `0` ở PHA A) — bằng chứng thật đầu tiên universal-pair-scan (BAOCAO20) hoạt động đúng trên mempool thật. Vẫn 0 `Simulated` cả 2 pha (tax cache rỗng, kế thừa nợ cũ). Halt sạch cuối phiên qua API (đúng tiền lệ BAOCAO18). Từ chối 1 yêu cầu giữa phiên của Chủ (thêm `PRIVATE_TX_URL` vào `.env` VPS) vì trái CẤM lệnh + sẽ phá phép đo đang chạy — xem docs/STATE.md mục `vps-2phase-dryrun` | BAOCAO26 |
| — | `usdt-quote-asset` (chủ chèn sau `vps-latencyprobe-retest`) | MỘT PHẦN — pair mở rộng "token/WBNB hoặc token/USDT" theo CLAUDE.md diff (quote asset xác định theo giao dịch victim, không trộn quote trong 1 path). USDT pin trong `DEX_REGISTRY.md` (`eth_getCode`=4413 byte, `eth_chainId`=0x38 xác nhận thật). `QuoteAsset` enum + `decode_and_classify_quote`/`evaluate_candidate_quote`/`decide_paper_quote`/`precheck_quote_only`/`resolve_reserves_for_quote` (toàn bộ hàm MỚI trong `pipeline.rs`, song song `decode_and_classify`/`decide_paper`/`decide_paper_v2` — 0 dòng nào của 3 hàm đó bị đổi). `pool.rs`/`decoder.rs` tổng quát hoá quote generic (`resolve_v2_pair_for_quote`/`get_reserves_vs_quote`/`token_vs` — 3 hàm cũ `resolve_v2_pair`/`get_reserves_vs_wbnb`/`token_vs_wbnb` giờ là lớp mỏng gọi hàm mới, không đổi hành vi). 4 field config mới (`scan_quote_usdt` ship `false` AN TOÀN, `min_profit_usdt`/`max_front_usdt`/`min_reserve_usdt` — số khởi tạo thô do Code chọn, Chủ tự chỉnh). `not_quote_pair` skip reason mới (thêm vào `SKIP_REASONS`, giữ nguyên `not_wbnb_pair`). Math: `profit_usdt = backUSDT - frontUSDT` THUẦN (gọi `sim_v2::search_max_front_in` với `gas_wei=0`, verify bằng test đối chiếu số học chính xác với 2 lời gọi trực tiếp), gas vẫn chặn qua `RiskGuard::front_cap_after_gas_reserve` (field BNB có sẵn, không quy đổi). KHÔNG có wallet-mode cho USDT (đúng lệnh — không có `VictimBook`/ngưỡng min-swap-size riêng, chỉ `min_reserve_usdt` lọc ở tầng pool). CÙNG PHIÊN — FIX LOGGER: `main.rs::handle_paper_tx` hết log `tx.skip.token:null` sai (nợ từ BAOCAO28) — tách `token_hint_from_precheck` (hàm thuần, 2 test mới), giữ token thật khi `precheck_token_only` đã decode thành công, chỉ `None` khi chính bước decode thất bại. **CHƯA nối `decide_paper_quote` vào `main.rs` live loop** (quyết định phạm vi có chủ đích, `main.rs` chỉ đụng cho fix logger — xem docs/STATE.md mục `usdt-quote-asset`) — phiên sau nối dây nếu Grok ra lệnh. | BAOCAO29 |
| — | `quote-live-wiring-funnel-diagnostics` (chủ chèn sau `usdt-quote-asset`) | MỘT PHẦN — (2) `decide_paper_quote` ĐÃ nối vào `main.rs::handle_paper_tx` làm nhánh fallback song song (chỉ chạy khi nhánh WBNB gốc trả `not_wbnb_pair` VÀ `scan_quote_usdt=true`, hành vi WBNB gốc 0 đổi, verify thật bằng 1 tx CAKE/USDT thật qua RPC thật -> `no_pool` đúng); (3) funnel log `funnel.minute` mỗi 60s (`main.rs::FunnelCounters`, KHÔNG đụng `web.rs`) verify thật cùng lần chạy; (4) 2 unit test decode bắt buộc + `PipelineSkip::SellDirection` mới (chỉ cho nhánh quote-aware, nhánh WBNB cũ không đổi) — ĐỦ, xem docs/STATE.md mục `quote-live-wiring-funnel-diagnostics`. (1) sửa CLAUDE.md theo diff: BLOCKED — diff không có trong khối lệnh nhận được phiên này, không bịa. (5) VPS (seed tax allowlist + bật `scan_quote_usdt=true` + rerun 30 phút) + GATE: BLOCKED — sandbox phiên này không có outbound SSH (port 22), 3 IP VPS đã biết đều timeout, trong khi HTTPS ra ngoài vẫn thông bình thường (khác lỗi VPS chết — là giới hạn hạ tầng phiên này). Chữ: CHỜ GROK, xem `baocao/BAOCAO30.md`. | BAOCAO30 |
| — | `foundation-fix-then-real-sim` cụm A (chủ chèn sau `quote-live-wiring-funnel-diagnostics`) | XONG — A1 (`PendingTxRaw` thêm `to/hash/gas/gas_price/nonce`, điền đủ cho cả WS lẫn txpool), A2 (`venues::Venue`/`PANCAKE_ROUTERS`/`venue_for_router`, gate `not_pancake_router`), A3 (`decoder::SwapVenue{V2,V3{fee}}`+`two_hop`, fee đọc thật từ calldata), A4 (gate order thật a→b→c→d trong `main.rs`, SỬA BUG THẬT: V3 không còn bị đưa nhầm vào `resolve_v2_reserves`; `SEEN_CAP` 50k + xoá theo tuổi; log thêm hash/to/venue/selector/fee), A5 (FIX BUG THẬT `pairbook.rs` — thiếu cắt comment cuối dòng khiến `pairs.txt` thật fail parse 100%, đúng nguyên nhân BAOCAO26/27), A6 (`FunnelCounters` chuyển vào `AppStateInner`, field đổi hẳn theo gate order thật, `GET /api/funnel` + bảng web), A7 (4 sửa CLAUDE.md đúng lệnh, ghi chú lệch số cụm A5→A7 trong khối lệnh gốc). `cargo test --lib` 237 passed/6 ignored, `cargo build --release` xanh — xem docs/STATE.md mục `foundation-fix-then-real-sim` | BAOCAO31 |
| — | `foundation-fix-then-real-sim` cụm B1+B2 (sim EVM thật qua revm) | MỘT PHẦN — B1: thêm `revm 43.0.2` (`alloydb`+`asyncdb` feature), verify `cargo tree -i alloy-primitives` vẫn ĐÚNG 1 bản (`1.7.3`, không đổi gì so với `alloy` đã pin). B2: `src/sim_evm.rs` mới — fork 1 block qua `AlloyDB`+`CacheDB`, attacker giả cấp số dư native cục bộ, chạy front-buy (native BNB, `swapExactETHForTokensSupportingFeeOnTransferTokens`) → victim (replay đúng calldata/from/value/gas/nonce thật) → back-sell (`swapExactTokensForETHSupportingFeeOnTransferTokens`), đo kèm `buy_tax_bps`/`sell_tax_bps` qua so `getAmountsOut` (kỳ vọng) với thực nhận. PHẠM VI: quote WBNB + V2 only (USDT CÒN NỢ — không có cơ chế wrap native, cần storage-override hoặc thêm 1 hop, chưa làm). KHÔNG tìm `front_in` bằng full EVM search (CÒN NỢ, tốn RPC quá nhiều) — dùng thẳng ước lượng `sim_v2` làm điểm duy nhất đưa vào EVM thật. Verify bằng 3 lần chạy THẬT trên mempool BSC sống (`#[ignore]`, `flavor="multi_thread"` — phát hiện thật: `WrapDatabaseAsync` cần runtime multi-thread): 1 lần khớp CHÍNH XÁC 0% với `sim_v2` cho token zero-tax thật, 2 lần EVM thật tái hiện đúng revert Solidity thật (`ds-math-sub-underflow`/`INSUFFICIENT_INPUT_AMOUNT`), 1 lần phát hiện THẬT token có buy-tax ~3% qua chênh lệch `getAmountsOut` vs `balanceOf` thật. `pipeline.rs` CHƯA gọi `sim_evm` ở đâu (B3 chưa làm, production behavior 0 đổi). Xem docs/STATE.md mục `foundation-fix-then-real-sim` phần B1/B2 | BAOCAO31 |
| — | `foundation-fix-then-real-sim` cụm B4 (validate 3 sandwich thật) | BLOCKED — lệnh yêu cầu replay 3 giao dịch SANDWICH THẬT (front+victim+back đã xảy ra) dán từ BscScan; sandbox phiên này không có quyền truy cập BscScan API/trình duyệt để tìm hash thật, không bịa hash. Hướng thay thế đã thử (B2's verify test, xem trên) chứng minh CƠ CHẾ sim đúng nhưng KHÔNG PHẢI "replay sandwich thật" (front/back trong test là của bot giả lập, không phải kẻ tấn công thật trong quá khứ, không có "profit on-chain thật" để so sánh). Theo đúng luật lệnh "chưa đạt thì CHƯA XONG, không sang C" — cụm C (đo tax tự động + gate pipeline) và D (config/VPS) KHÔNG làm phiên này dù cơ chế đo tax lõi trong `sim_evm.rs` đã sẵn sàng và đã chứng minh hoạt động đúng trên dữ liệu sống. Cần lệnh Grok xác nhận tiêu chí B4 thay thế phù hợp với hạ tầng sandbox (không có BscScan), hoặc cấp kênh tìm sandwich thật khác. Xem docs/STATE.md mục `foundation-fix-then-real-sim` phần B4 | BAOCAO31 |
| — | `evm-validate-wire-tax` cụm B4' (validate tự động thay BscScan) | KHÔNG ĐẠT SỐ — 4 test `#[ignore]` mới/sửa trong `src/sim_evm.rs` (B4'.1 viết lại chạy qua MỌI candidate đo được thay vì dừng ở candidate đầu; B4'.2 giải thích số `profit=-1` bằng số liệu thật, không phải bug; B4'.3(a) dự đoán victim đơn lẻ; B4'.3(b) quét+replay sandwich thật bằng Swap event; B4'.4 ternary search EVM warm-cache + đo thời gian), chạy THẬT 5 lần trên RPC công khai, sửa 3 lỗi thật phát hiện qua chạy sống (assertion quá nghiêm ở biên làm tròn, vi phạm EIP-3607 khi probe balance của contract, panic thay vì skip khi không dò được storage slot) + 1 bug K-invariant (reset thiếu `balanceOf(pair)` thật bên cạnh reserve cache). B4'.1 PASS về cơ chế, B4'.4 đo được thời gian (yêu cầu duy nhất của mục này, chứng minh warm-cache nhanh hơn ~300-1000 lần). B4'.3(a)/(b) KHÔNG đạt ngưỡng số (cần ≥5/≥3 mẫu thật) — nguyên nhân chính là RPC công khai rate-limit (`-32005`) khi gọi `eth_getLogs`/`txpool_content` nhiều lần liên tục, không phải lỗi thiết kế. Theo đúng luật "chưa đạt B4' thì không sang B3/C/D" — CẢ 3 cụm đó KHÔNG làm phiên này. `cargo test --lib` 237 passed/9 ignored, `cargo build --release` xanh. Xem docs/STATE.md mục `evm-validate-wire-tax` | BAOCAO32 |
| — | `evm-validate-fixed-then-wire` cụm B3+C+D (BAOCAO33) + `wsl-env-rules-paperrun` (BAOCAO34) | MISSING trong bảng này — 2 phiên đã chạy (nối `sim_evm` thật vào pipeline qua `sim_engine`/`decide_with_evm`, đo tax tự động bằng EVM, `ZERO_TAX_ALLOWLIST`, xác nhận môi trường WSL + `scripts/paper_run.sh`, xem code hiện tại + `baocao/BAOCAO33.md`/`BAOCAO34.md`) nhưng CHƯA từng được ghi lại dòng riêng ở bảng này — Code phiên `exec-path-traps` phát hiện khoảng trống này khi cập nhật bảng (đúng lệnh "kể cả dòng cụm 0 còn thiếu") nhưng KHÔNG tự bịa lại chi tiết 2 phiên đó (không đọc trực tiếp `BAOCAO33.md`/`BAOCAO34.md` phiên này ngoài phần đã trích dẫn trong `BAOCAO_AUDIT_2026-09-15.md`) — cần phiên sau backfill đúng nội dung 2 BAOCAO đó nếu Grok muốn bảng đầy đủ. | BAOCAO33, BAOCAO34 |
| — | Audit toàn diện trước live (BAOCAO_AUDIT_2026-09-15.md) | XONG (đứng ngoài bảng cụm — audit đọc-only trên VPS + local, phát hiện 27 lỗi code F-01..F-27 + 12 lỗi vận hành/bảo mật V-01..V-12, xem file audit) — nguồn gốc trực tiếp của cụm `exec-path-traps` dưới đây (12 mục đầu khớp F-04/F-05/F-06/F-07/F-08/F-13/F-14/F-15/F-16/F-20/F-26 + V-06) | BAOCAO_AUDIT_2026-09-15 |
| — | `exec-path-traps` (chủ ra lệnh sau audit — chặn bẫy trên đường thực thi trước khi nối signer `7.3`) | XONG 12/12 mục lệnh gốc + mục 13 bổ sung — F-26 (`tx.build` chỉ sau EVM xác nhận khi `sim_engine="evm"`, engine="v2" vẫn build ngay, field `engine` mới trong log); F-06 (`executor_self_address()` — paper mode luôn `None` nên MỌI build hiện bị từ chối có chủ đích, log `build.refused{reason:"self_address_zero"}`, xoá `PLACEHOLDER_SELF_ADDRESS` khỏi đường production); F-07 (back-sell dùng `evm.token_received` thật qua `EvmDecision`, không còn ước lượng `sim_v2`); F-08 (`front_slippage_bps`/`back_slippage_bps` wire thật, xoá field `executor_slippage_bps` — còn trong `config.toml` thì fail load kèm thông báo đổi tên); F-05 (`Config::gate_check` là nguồn DUY NHẤT, `executor::gate_check` cũ xoá, test duyệt 256 tổ hợp cờ); F-04 (`RiskGuard::record_result` có call site thật ở `spawn_victim_validator` — paper dùng tín hiệu validator [lệch >1%/victim revert thật] làm "loss", `/api/status` có `risk_guard{consecutive_loss,exceeded}`, call site live 7.x đánh dấu sẵn bằng comment); F-13 (`transport::NonceCache`/`compare_nonce`, `eth_getTransactionCount(..,"latest")` — CHỌN "latest" thay vì literal "pending" nêu trong lệnh, lý do kỹ thuật ghi rõ trong code vì "pending" sẽ luôn coi mọi candidate hợp lệ là stale; 2 skip reason mới `nonce_stale`/`nonce_future`, quan sát THẬT trên mempool: `nonce_stale=6..10` trong 1 phút chạy); F-14 (`PipelineSkip::Deadline` mới — reason có từ trước nhưng chưa từng phát sinh, gate `deadline < now + 2*3s`, UR tx `deadline=None` không áp); F-15 (`passes_router_gate` thêm tham số `source`, `to=None` chỉ qua khi `source=="inject"`); F-16 (`decoder::venue_matches_router` cross-check router thật với selector đã decode, mismatch → `decode_fail` + `detail:"venue_mismatch"`); F-20 (4 chỗ cộng offset trong `decoder.rs` đổi sang `checked_add`, fuzz test 1000 calldata random + 20 calldata offset gần `usize::MAX`, không panic); V-06 (`halt_watch_task` mới — 3 nguồn tx tự kiểm `state_files.is_halted()` trước khi spawn `handle_paper_tx`, log `halt.triggered`/`halt.cleared` đúng 1 lần mỗi lần chuyển trạng thái, `/api/status` phản ánh `STOPPED`); mục 13 (`scripts/paper_run.sh` sửa 4 lỗi đo: chỉ đếm log lần chạy này qua `RUN_LOG()`, chờ `halt.triggered` thật tối đa 10s trước khi kill, phát hiện bot chết giữa chừng thay vì báo kết quả rỗng, dòng tổng kết DoD cuối — PHÁT HIỆN THÊM VÀ SỬA 1 bug thật ngoài 4 mục: `.env` sourced không `export` nên bot con không thấy `BSC_HTTP`/`BSC_WS`, rơi về `vps.json` placeholder → không kết nối RPC nào — thêm `set -a`/`set +a`). Xem `docs/STATE.md` mục `exec-path-traps` cho quyết định kỹ thuật chi tiết (đặc biệt mục 4(b) và 7). | BAOCAO35 |

## Nợ / MISSING hiện tại

- V2/V3/V4-Infinity đã pin (BAOCAO02) — `venue_unpinned` không còn áp dụng
  cho 3 family này. "Bản mới hơn" vẫn DISABLED (chưa deploy trên BSC).
- `/api/status` `last_block` chỉ khác `null` khi chủ điền `BSC_HTTP`/`BSC_WS`
  thật vào `.env` (hoặc `vps.json` fallback không còn là placeholder) — code
  `2.1` đã nối thật (`src/main.rs::connect_rpc`), verify bằng RPC công khai
  trong BAOCAO03, nhưng máy phiên này chưa có `.env` nên boot mặc định vẫn
  log `rpc.skip` (đúng thiết kế, không phải lỗi).
- `src/decoder.rs` (2.2) CHƯA có: pending-tx pipeline thật (subscribe mempool
  qua WSS rồi gọi decoder) — mới có hàm decode thuần + fixture test, chưa
  wire vào `main.rs`/vòng lặp watch (thuộc `3.x/4.1`). Universal Router chỉ
  decode 1 command/1 input mỗi tx (không multicall); biến thể V2-style của
  SmartRouter (4 tham số, không `deadline`) chưa pin chữ ký chính xác — xem
  `docs/STATE.md`.
- `src/pool.rs` (2.3) V4/Infinity: **ĐÃ ĐÓNG MỘT PHẦN ở `v4-pool-resolve`
  (BAOCAO19)** — `resolve_infinity_pool` giờ quét THẬT `eth_getLogs` trên
  `CLPoolManager`/`BinPoolManager`, không còn trả `hooks_unread` cứng cho
  mọi token nữa (chỉ trả `hooks_unread` khi THẬT SỰ không tìm thấy log nào
  trong khoảng block quét). Còn nợ: (a) block deploy 2 pool manager MISSING
  (không tra được qua eth_getCode/BscScan/web — xem docs/STATE.md), nên
  `from_block` không có mặc định, caller phải tự truyền; (b) chưa tìm được
  ví dụ pool Infinity ghép WBNB thật nào trong tầm RPC free-tier để verify
  nhánh "tìm thấy" end-to-end (nhánh "không tìm thấy" + công thức PoolId ĐÃ
  verify thật). V2/V3 đã resolve được qua `eth_call` thật (verify real RPC
  trong BAOCAO03), không đổi.
- `src/pipeline.rs::decide_paper` (4.1) là lõi quyết định THUẦN (không cần
  RPC), test bằng fixture tay — CHƯA nối vào vòng lặp pending-tx thật trong
  `main.rs` (transport.rs mới có `subscribe_blocks`, chưa có
  `eth_subscribe newPendingTransactions`) — đó là việc của `5.1+`.
- `pipeline.rs` chỉ xử lý chiều victim MUA token bằng WBNB (`path.token_a ==
  WBNB`) — chiều BÁN token lấy WBNB chưa có model sandwich tương ứng (xem
  docs/STATE.md).
- `src/tax.rs::measure_roundtrip_via_router` là plumbing `eth_call` thật
  nhưng KHÔNG phát hiện được fee-on-transfer tax thật (chứng minh toán học
  trong docs/STATE.md, không phải giả định) — cần hợp đồng "probe" 1
  `eth_call` (kỹ thuật honeypot-detector chuẩn), NGOÀI PHẠM VI phiên này.
  `honeypot_or_tax` vẫn là default an toàn khi cache trống/hết hạn.
- `src/sim_v3.rs` chỉ sim V3 single-hop qua `QuoterV2.quoteExactInputSingle`
  (khớp decoder chỉ decode 1-hop). V4/Infinity KHÔNG sim — `pool.rs::resolve_infinity_pool`
  (từ `v4-pool-resolve`, BAOCAO19) giờ CÓ THỂ tìm ra `PoolKey` thật, nhưng
  CHƯA có hàm nào gọi `CLQuoter`/`BinQuoter` để sim giá từ `PoolKey` đó —
  `sim_v3.rs` KHÔNG được đụng ở BAOCAO19 (đúng CẤM), việc sim V4 vẫn là cụm
  riêng, ngoài phạm vi. Comment cũ trong `sim_v3.rs` mô tả `resolve_infinity_pool`
  "luôn trả hooks_unread" vẫn đúng cho MỌI caller hiện có (chưa ai gọi hàm
  mới), chỉ không còn đúng nếu đọc thẳng `pool.rs`.
- `config.rs::max_consecutive_loss`/`gas_reserve_bnb_wei` (phiên
  config-hot-reload): đã validate lúc load (số âm fail, đúng luật) nhưng
  CHƯA có logic nào trong repo so sánh/tiêu thụ 2 field này — chưa hề có
  ngay cả trước phiên này (không phải hardcode bị bỏ sót, mà CHƯA TỒN TẠI
  risk-guard/wallet-balance-check nào để wire vào). `max_exposure_bnb` ĐÃ
  wire ở `5.2` (xem `docs/STATE.md`), không còn trong danh sách này. Sẽ wire
  2 field còn lại khi cụm risk-guard/live thật được viết (`7.x`), không bịa
  logic giả định trước.
- `5.1` ĐÃ wire (BAOCAO06), `5.2` (BAOCAO08) thêm fallback `txpool_content` +
  verify runtime THẬT lần đầu WSS pending thật kết nối được (`.env` chủ đã
  điền `BSC_WS` phiên này) — nhưng subscription rớt sau ~1s ("channel
  lagged", tốc độ mempool BSC thật vượt tốc độ xử lý), fallback sang
  `txpool_content` cũng chỉ chạy được vài giây trước khi RPC công khai
  rate-limit (`-32005`). Nghĩa là pending THẬT liên tục, ổn định lâu dài vẫn
  CHƯA được chứng minh (cần RPC riêng không rate-limit + có thể cần tăng
  buffer subscription — xem `docs/STATE.md` mục "5.2" phần "Ghi nhận cho
  phiên sau"). `resolve_v2_reserves` vẫn gộp mọi lỗi RPC + "không có pool"
  thành `no_pool`, chưa log riêng chi tiết lỗi RPC ở bước này nếu cần chẩn
  đoán sâu (không đổi ở `5.2`).
- `CLAUDE.md` mục "Config — thiếu field = fail load" ĐÃ được cập nhật ở `5.3`
  (lệnh `5.3` cho phép rõ ràng) — thêm `pending_poll_ms` + `pending_txpool_max_per_poll`
  vào danh sách field bắt buộc, không còn lệch với `config.rs` thật.
- `PENDING_WS_CHANNEL_SIZE=4096` (transport.rs, `5.3`) nâng buffer subscription
  pending-tx WSS từ mặc định `alloy` (16) — CHƯA đo được có đủ chịu tốc độ
  mempool BSC thật lâu dài hay không (verify runtime `5.3` chỉ chạy ~5-6s,
  không lặp lại được "channel lagged" để so sánh trước/sau). Cần 1 phiên
  chạy dài hơn (nhiều phút) để đo, xem docs/STATE.md mục "5.3".
- `resolve_v2_reserves`/`pipeline.rs::decide_paper` KHÔNG được `http_pool`
  bảo vệ trực tiếp (vẫn gộp mọi lỗi RPC + "không có pool" thành `no_pool`,
  kế thừa `5.1`) — `5.3` chỉ bảo vệ GIÁN TIẾP qua `http_pool_health_check`
  (task nền 5s/lần giữ `app_state.provider` sống), không sửa chữ ký
  `pipeline.rs` (giữ tách lõi thuần/RPC thật, xem docs/STATE.md mục "5.3").
- Tax cache inject (BAOCAO07) mở đường CHỦ TỰ ĐIỀN `tax_cache` tay lúc bot
  chạy (`POST /api/tax`/`state/tax_inject.jsonl`) — nhưng KHÔNG phải đo tax tự
  động: `measure_roundtrip_via_router` vẫn không được gọi trong live loop
  (quyết định `3.3` giữ nguyên), và `combine_roundtrip_bps` chỉ kết hợp 2 số
  bps chủ TỰ CUNG CẤP (không tự đo được đúng/sai của số chủ nhập) — cụm đo tax
  thật (hợp đồng probe) vẫn NGOÀI PHẠM VI, còn nợ như cũ (xem docs/STATE.md
  mục "Tax stub"). 1 dòng `state/tax_inject.jsonl` đọc lúc `allow_tax_inject=false`
  bị bỏ qua VĨNH VIỄN (không tự áp dụng lại khi cờ bật sau đó) — chủ cần ghi
  lại dòng mới nếu cần.
- `vps.json` phiên `rpc-probe` phát hiện đã bị ghi đè thành thông tin đăng
  nhập SSH root thật của VPS (file KHÔNG gitignored) — đã sửa về đúng schema
  `chain_id`/`region_hint`/RPC placeholder, không còn secret nào trong file
  (xem docs/STATE.md mục "vps.json"). Chủ nên đổi mật khẩu VPS đó và không
  dán thông tin đăng nhập vào bất kỳ file nào trong repo (kể cả file
  gitignored) ở các phiên sau.
- Phiên `deploy-vps` (BAOCAO11): không có `VPS_HOST` (hay tương đương) trong
  biến môi trường phiên này, nên KHÔNG chạy được `scripts/deploy_vps.sh`
  thật lên VPS NJ — chỉ viết script + kiểm cú pháp (`bash -n`, PowerShell
  AST parser) + hoàn thiện README, chưa có lần deploy thật nào thành công.
  Máy dev CÓ sẵn `ssh`/`scp`/`tar` qua Git for Windows (`C:\Program
  Files\Git\usr\bin`) nhưng PowerShell gốc KHÔNG có `ssh.exe`/`scp.exe`
  (OpenSSH Client Windows chưa cài) — `deploy_vps.ps1` đã tự dò fallback
  sang bản Git for Windows nếu PATH thiếu, xem `docs/STATE.md` mục
  `deploy-vps`. Việc chạy thật cần chủ/Grok cung cấp `--host`/`-VpsHost`
  (và key/agent đã cấp quyền) ở phiên sau.
- `rpc_probe` (phiên `rpc-probe`) mới đo được từ máy dev Windows tại VN —
  CHƯA có lần đo THẬT nào chạy trên VPS New Jersey (`vps.json::region_hint`).
  Số RTT hiện tại (BAOCAO10/docs/STATE.md) không dùng được để quyết định thứ
  tự failover thật vì cộng thêm độ trễ xuyên lục địa. **ĐÃ ĐÓNG** ở
  BAOCAO12: `rpc_probe` chạy thật trên VPS NJ, RTT ~5-11ms (5 URL công khai
  bootstrap, xem docs/STATE.md mục `deploy-vps-live`). Việc "đổi thứ tự URL
  theo kết quả đo" vẫn CHƯA làm (đúng lệnh, `rpc_probe` chỉ đo + gợi ý).
- Phiên `deploy-vps-live` (BAOCAO12, kế tiếp `deploy-vps`/BAOCAO11 cùng
  ngày): Chủ dán IP/password VPS thật vào chat — bot paper ĐANG CHẠY THẬT
  trên VPS đó (`systemd-run --unit=bsc-sandwich-paper`, transient, KHÔNG
  enable khi reboot VPS). `.env` trên VPS là bản TỐI THIỂU tự viết trực tiếp
  trên VPS (chỉ 5 URL RPC công khai không token + `PRIVATE_KEY` rỗng) —
  KHÔNG phải `.env` thật của máy dev (bị hệ thống chặn copy vì lý do
  "Data Exfiltration", đúng luật `CLAUDE.md`). Chủ nên tự SSH ghi đè `.env`
  đó bằng cấu hình RPC riêng (trả phí/nhanh hơn) nếu muốn, và cân nhắc dùng
  `systemctl enable` (thủ công) nếu muốn bot tự chạy lại sau khi VPS reboot
  (transient unit hiện KHÔNG tự enable). Xem chi tiết đầy đủ + 2 hành động
  bị chặn bởi bộ phân loại an toàn ở `docs/STATE.md` mục `deploy-vps-live`.
- `scripts/deploy_vps.sh`/`.ps1` nhánh `--run` hiện dùng `nohup ... &
  disown` — phiên `deploy-vps-live` phát hiện mẫu này TREO kênh SSH
  `paramiko`/có thể treo cả `ssh` thường khi gọi qua kênh exec không có tty
  (xem docs/STATE.md). **ĐÃ SỬA ở BAOCAO13**: nhánh `--run`/`-Run` đổi sang
  `systemd-run --collect ...` (fallback `nohup` + cảnh báo nếu VPS không có
  `systemd-run`) — verify cú pháp (`bash -n` + PowerShell parser) xanh, CHƯA
  chạy thật lên VPS phiên này (không có VPS mới được cấp).
- **Cụm pair-mode/sell-direction (BAOCAO13) — ĐÃ ĐÓNG ở BAOCAO14**: lệnh Grok
  sau khi đọc phát hiện toán học (front mua trước/back bán sau victim bán
  luôn lỗ, số tay ở BAOCAO13) là BỎ HẲN chiều victim bán khỏi pipeline, không
  giữ code chết. `sim_v2::search_max_front_in_sell`/`simulate_front_then_victim_sell`/
  `quote_at_sell` và `pipeline::SwapDirection` ĐÃ XOÁ khỏi repo (không phải
  DISABLED) — `decide_paper_v2` giờ chỉ nhận chiều victim MUA token, y hệt
  phạm vi `decide_paper` gốc. Kiến trúc "back-run" đúng cho chiều bán (front
  bán trước cần tồn kho token/flashloan) NGOÀI SCOPE "1 signer, cấm flashloan"
  của CLAUDE.md — cần lệnh Grok riêng nếu muốn làm sau. Xem quyết định đầy đủ
  ở `docs/STATE.md` mục "QUYẾT ĐỊNH — chiều victim bán, phiên BAOCAO14".
- **`RiskGuard::record_result` CHƯA được gọi tự động ở đâu** (BAOCAO13) —
  `decide_paper_v2` chỉ ĐỌC (`consecutive_loss_exceeded`/
  `front_cap_after_gas_reserve`), không GHI, vì paper loop không có giao dịch
  thật để biết lỗ/lãi thật. `consecutive_loss` vì vậy luôn `0` trong dry-run —
  chờ tầng executor thật (`7.x`) gọi `record_result` sau khi biết kết quả
  on-chain thật.
- ~~Nợ đơn vị `evaluate_candidate` (BAOCAO13): so `amount_in` phía chiều BÁN...~~
  **ĐÃ HẾT ÁP DỤNG ở BAOCAO14** — chiều victim bán đã bị bỏ hẳn khỏi
  `evaluate_candidate`/`decide_paper_v2` (xem `docs/STATE.md`), `amount_in`
  giờ LUÔN là WBNB wei (chiều mua duy nhất còn lại) nên không còn tình huống
  lệch đơn vị này nữa.
- `PairBook`/`decide_paper_v2` khiến MỌI tx decode được (không chỉ tx trong
  `victims.txt`) đều tốn 1 `eth_call resolve_v2_reserves` (cần biết
  `pair_addr` để tra `PairBook`) — tăng tải RPC so với `5.1` (trước đây lọc
  `not_in_list` sớm, không tốn RPC cho tx ngoài `victims.txt`). Vẫn được bảo
  vệ bởi `pending_semaphore`(≤4)/`pending_txpool_max_per_poll` có sẵn, chưa
  đo tải THẬT dài hạn với `pairs.txt` có nhiều entry thật.
- **`7.2` ĐÃ ĐÓNG ở BAOCAO15, phần "nối vào pipeline thật" ĐÃ ĐÓNG THÊM ở
  BAOCAO16**: `src/calldata.rs::encode_front_buy`/`encode_back_sell` giờ được
  gọi thật từ `executor::build_and_log_paper_sandwich` (qua
  `pipeline::decide_and_build_paper_v2`) — `amount_in`/`amount_out_min`/
  `deadline` đều lấy từ `SandwichQuote`/`Config`/wall-clock thật (không còn
  `0`/tham số tay), xem docs/STATE.md mục `7.3`. ~~CÒN NỢ: `main.rs::handle_paper_tx`
  CHƯA đổi sang gọi `decide_and_build_paper_v2`~~ **ĐÃ ĐÓNG ở BAOCAO18** —
  `main.rs:777` giờ gọi `decide_and_build_paper_v2`, live loop thật (WSS/
  txpool/inject) đã verify runtime thật sinh `tx.build` khi có candidate
  `Simulated`, xem docs/STATE.md mục `7.3-nối-dây`. `to` vẫn placeholder
  `Address::ZERO` (chưa dẫn xuất được địa chỉ ví thật từ `B256`, xem `7.1`).
  **`7.3` (gửi tx thật/sendRaw) vẫn CHƯA LÀM** — không có hàm ký/gửi giao dịch
  nào trong repo (`executor::tests::no_send_raw_transaction_call_anywhere_in_src`
  xác nhận bằng grep toàn `src/`, BAOCAO16, xác nhận LẠI ở BAOCAO18).
- `relay-bundle-builder` (BAOCAO21) — `src/relay.rs` build request
  `eth_sendBundle`/`eth_sendMevBundle` THUẦN. CHƯA nối vào `pipeline.rs`/
  `executor.rs` — muốn gửi bundle thật cần thêm (1) signer thật ký
  `front_raw_hex`/`back_raw_hex` thật (nợ từ `7.1`/`7.3`), (2) HTTP client
  thật + xử lý response/lỗi relay thật, (3) lệnh Grok riêng cân nhắc rủi ro
  tiền thật.
- `relay-schema-verify`/`relay-finalize` (BAOCAO23+BAOCAO24) — **ĐÃ ĐÓNG**:
  hình dạng `params` (bọc mảng `[bundle_object]`) ĐÃ VERIFY ĐÚNG cho CẢ 2
  relay. `CLUB48_RPC_URL` đã sửa đúng thành `https://puissant-builder.48.club/`
  (BAOCAO24, verify cURL thật trả lỗi RLP nội dung, không phải method-not-found).
  Nợ MỚI phát sinh từ verify `PRIVATE_TX_URL` cá nhân của Chủ (BAOCAO24, mục
  B): 2/3 URL cá nhân (kiểu `48club`/`bsc-rpc.com`) KHÔNG hỗ trợ
  `eth_sendBundle` (là endpoint privacy-tx-đơn, không phải bundle relay) —
  Chủ cần tự xác nhận với 48 Club nguồn/loại endpoint cá nhân đúng nếu muốn
  submit bundle riêng qua tài khoản đó (KHÔNG bịa/đoán route — cần chủ hỏi
  trực tiếp 48 Club support). Xem docs/STATE.md mục `relay-finalize`.
- `vps-lowthreshold-retest` (BAOCAO27) — **PHÁT HIỆN**: `BSC_WS=Alchemy`
  nhận subscribe pending nhưng KHÔNG stream mempool trên BSC (0 pending
  tx/9 phút) → bot đói input, 0 Simulated dù ngưỡng=0 + tax-inject=0bps đã
  áp đúng. Nợ: (1) cần khối lệnh Grok đổi `BSC_WS` về node stream full-pending
  (publicnode/blockrazor-stream/bloXroute) rồi rerun; (2) có thể inject
  `state/inject_tx.jsonl` giả để chứng minh đường sim end-to-end đã thông
  (ngưỡng=0+tax=0); (3) `PRIVATE_TX_URL` Chủ dán vẫn CHƯA ghi (cần lệnh
  riêng); (4) `pairs.txt` trên VPS lỗi parse toàn bộ (`pair.parse_error=900`),
  soi phiên sau. SSH key ed25519 đã tạo (`key/`, gitignored) + nạp VPS,
  keyless login verify OK — phiên sau không cần password. Xem docs/STATE.md
  mục `vps-lowthreshold-retest`.
- `vps-latencyprobe-retest` (BAOCAO28) — đổi `BSC_HTTP`/`BSC_WS` VPS sang
  giá trị LOCAL rồi rerun 5 phút: VẪN 0 Simulated, nhưng nguyên nhân KHÁC
  BAOCAO27 — `BSC_WS` LOCAL (`wss://bsc-dataseed1.bnbchain.org`) CHẾT HẲN
  (404) cho cả pending-subscribe lẫn head-subscribe (domain đó chỉ có HTTP).
  Pending vẫn chảy được nhờ fallback HTTP `txpool_content` có sẵn (không
  phải nhờ WS) — mempool KHÔNG còn là rào cản. Rào cản còn lại: mẫu 5 phút
  không chắc trúng đúng 1 trong 3 token đã inject tax (hoặc trúng nhưng vào
  gap stale cache ~6s/chu kỳ, nợ cũ từ BAOCAO27 chưa sửa) — KHÔNG kết luận
  dứt điểm được vì `tx.skip` log không ghi `token` thật (luôn `null`, kể cả
  ở nhánh đã biết token như `honeypot_or_tax`/`no_pool`). Nợ: (1) sửa logger
  `handle_paper_tx`/`decide_paper_v2` ghi `token` thật vào `tx.skip` (cần
  khối lệnh riêng, có đụng code sản phẩm); (2) chọn `BSC_WS` khác thật sự hỗ
  trợ WSS nếu muốn khôi phục log `rpc.block`/head-subscribe (hiện `last_block`
  vẫn cập nhật qua `http_pool_health_check` không log JSONL, nhưng
  `rpc.block` event đã ngừng từ `22:12:43Z` — không ảnh hưởng pipeline paper,
  chỉ ảnh hưởng khả năng đo block-latency từ log); (3) `pairs.txt` VPS vẫn
  lỗi parse (chưa đụng, ngoài phạm vi). Đã thêm `scripts/latency_probe.sh` +
  `scripts/block_latency.sh` (bash thuần, đo ngoài, không sửa code sản
  phẩm) cho phiên sau tái dùng. Xem docs/STATE.md mục
  `vps-latencyprobe-retest`.
- `usdt-quote-asset` (BAOCAO29): `decide_paper_quote`/`precheck_quote_only`/
  `resolve_reserves_for_quote` CHƯA nối vào `main.rs::handle_paper_tx` (live
  loop pending-tx thật) — hàm sẵn sàng, test đầy đủ (thuần, không cần RPC
  sống), nhưng chưa có candidate USDT thật nào chạy qua mempool. `executor.rs`/
  `calldata.rs` (build tx front-buy/back-sell paper) CŨNG CHƯA hỗ trợ quote
  USDT (chỉ build được calldata V2 Router hướng WBNB, xem `7.2`/`7.3`) — cần
  khối lệnh riêng nếu muốn `tx.build` log cho candidate USDT. Không có
  wallet-mode/ngưỡng min-swap-size riêng cho USDT (đúng lệnh gốc, chỉ
  `min_reserve_usdt` lọc ở tầng pool) — nếu Chủ muốn ngưỡng đó sau này, cần
  field config mới + lệnh riêng. `tx.skip.token` (BAOCAO28) ĐÃ ĐÓNG — xem
  dòng bảng `usdt-quote-asset` phía trên.
- `venues.rs::registry_snapshot`/`GET /api/venues` KHÔNG liệt kê USDT (đúng
  vì USDT không phải router/factory family — đứng ở `DEX_REGISTRY.md` mục
  Core cùng WBNB) — nếu Chủ/Grok muốn dashboard hiển thị trạng thái
  `scan_quote_usdt`/ngưỡng USDT, cần thêm khối riêng cho `web.rs`/`web/`
  (ngoài phạm vi BAOCAO29, không đụng `web.rs` phiên này).
- `quote-live-wiring-funnel-diagnostics` (BAOCAO30): (1) CLAUDE.md diff
  ("revm bắt buộc, ĐO TAX, QUOTE_SET") KHÔNG áp được — nội dung diff không
  có trong khối lệnh nhận được phiên này (không bịa) — cần Grok dán lại
  nguyên văn ở lệnh sau. (2)(3)(4) đã đóng, xem bảng trên. (5) VPS/GATE
  BLOCKED — sandbox phiên này không có outbound SSH (port 22 timeout tới cả
  3 IP VPS đã biết từ `~/.ssh/known_hosts`, trong khi HTTPS ra ngoài vẫn
  thông) — cần Chủ/Grok cấp lại kênh SSH hoặc xác nhận IP VPS còn sống ở
  lệnh sau để seed tax allowlist + bật `scan_quote_usdt=true` TRÊN VPS +
  rerun 30 phút + trả lời GATE thật. `sell_direction` (skip reason mới)
  chưa có trong `venues.rs::SKIP_REASONS` (ngoài `ĐƯỢC ĐỤNG` phiên này) nên
  không hiện trên `/api/skips` dashboard, dù vẫn đếm đúng trong log JSONL
  thô/`skip_counts`. `two_hop` trong `funnel.minute` LUÔN bằng `quote_ok`
  (chưa có tín hiệu multihop riêng — xem docs/STATE.md, quyết định có chủ
  đích tránh sửa `decoder.rs` rủi ro phá test cũ). Không có `Multihop`/
  `NoInventory` skip reason (chưa cần/không áp dụng, xem docs/STATE.md).
- `foundation-fix-then-real-sim` (BAOCAO31): `sim_evm.rs` chỉ hỗ trợ quote
  WBNB + pool V2 (USDT còn nợ — cần storage-override hoặc thêm 1 hop
  WBNB→USDT, chưa làm). Không có full EVM ternary search cho `front_in` —
  dùng thẳng ước lượng `sim_v2` (đóng, rẻ) làm điểm duy nhất đưa vào EVM
  thật (đánh đổi RPC cost, ghi rõ trong docs/STATE.md). `pipeline.rs` CHƯA
  gọi `sim_evm` ở bất kỳ đâu (B3 chưa làm) — quyết định `Simulated` trong
  live loop vẫn dùng `sim_v2`/`TaxCache` cũ y hệt trước phiên này. B4 (3
  sandwich thật từ BscScan) BLOCKED — sandbox không có BscScan API/trình
  duyệt. Theo đúng luật "chưa đạt B4 thì không sang C" — cụm C (đo tax tự
  động + gate pipeline, dù cơ chế lõi `buy_tax_bps`/`sell_tax_bps` đã có
  sẵn trong `sim_evm.rs` và đã verify hoạt động đúng trên mempool thật) và
  cụm D (config/VPS, D1 phụ thuộc thiết kế TTL của C2) ĐỀU CHƯA LÀM. Cần
  lệnh Grok cho B4 (tiêu chí thay thế khi không có BscScan, hoặc cấp
  quyền/kênh khác) trước khi tiếp tục.
- `evm-validate-wire-tax` (BAOCAO32): B4'.3(a) cần RPC không rate-limit để
  gom đủ ≥5 tx mined+đủ điều kiện trong 1 cửa sổ chạy (máy dev phiên này
  chỉ có RPC công khai `bsc-dataseed.binance.org`, không có `.env`/RPC
  riêng — xem docs/STATE.md). B4'.3(b) cơ chế đã tổng quát hoá đúng (Swap
  event, không lệ thuộc `tx.to()==v2_router`) nhưng cùng lý do rate-limit
  khi `eth_getLogs` 300 block liên tục — cần RPC riêng để phân biệt "thật
  sự hiếm" với "quét không hết vì bị chặn". B4'.4: back-sell của 1 candidate
  cụ thể revert `INSUFFICIENT_INPUT_AMOUNT` sau khi front-buy đã thành
  công (sau khi sửa bug K-invariant) — nghi ngờ token có cơ chế nội bộ phi
  chuẩn (reflection/anti-bot) làm lệch state khi ghi đè `balanceOf` trực
  tiếp qua storage, CHƯA xác định nguyên nhân gốc, không chặn kết luận vì
  B4'.4 không có ngưỡng PASS/FAIL. `sim_evm.rs` có 3 hàm mới dùng lại được
  cho B3.3 (USDT storage-override): `probe_erc20_balance_slot`/
  `set_erc20_balance`/`mapping_storage_key` (dò+ghi balance ERC20 bất kỳ
  qua storage, đã verify hoạt động đúng qua sentinel test thật). Cần lệnh
  Grok: (a) cấp RPC riêng không rate-limit để chạy lại B4'.3(a)/(b) đủ
  ngưỡng số, hoặc (b) hạ ngưỡng số cho phù hợp hạ tầng sandbox, hoặc (c)
  coi B4'.1+B4'.2+B4'.4 (đã đạt/đo được) là đủ để sang B3, trước khi Code
  tiếp tục B3/C/D.
