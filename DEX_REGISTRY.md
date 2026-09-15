# DEX_REGISTRY.md — Pin venue trên BSC (chain 56)

Cụm `1.1+1.2+1.3` (BAOCAO02, pinned_date `2026-09-14`). Nguồn: tài liệu
chính thức `developer.pancakeswap.finance` (fetch trực tiếp, không cache cũ) +
`eth_getCode` thật qua RPC công khai `https://bsc-dataseed.binance.org/`
(máy chưa có `.env`/`BSC_HTTP` riêng — `docs/STATE.md` ghi lại quyết định
dùng RPC công khai chỉ để pin, không phải RPC runtime của bot; `2.1` sẽ nối
`alloy-provider` thật vào `.env` của chủ).

`eth_chainId` xác nhận `0x38` (56) trước khi getCode bất kỳ địa chỉ nào — dán
trong BAOCAO02 ô 6.

## Bảng pin

### Core

| Contract | Address | source_url | pinned_date | getCode (len byte) | Trạng thái |
|---|---|---|---|---|---|
| WBNB | `0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c` | (địa chỉ well-known, xác nhận qua eth_getCode; không phải trang docs riêng) | 2026-09-14 | 3124 | PINNED |
| USDT (BSC-USD) | `0x55d398326f99059fF775485246999027B3197955` | (địa chỉ BSC-USD/Tether well-known — CLAUDE.md cụm `usdt-quote-asset` đã ghi thẳng địa chỉ này; đã dùng làm token ví dụ trong `pool.rs`/`v4-pool-resolve` từ BAOCAO19, xác nhận `eth_getCode` thật phiên này, không phải trang docs riêng) | 2026-09-15 | 4413 | PINNED — quote asset thứ 2 ngang hàng WBNB (cụm `usdt-quote-asset`) |

### V2 (`1.1`)

| Contract | Address | source_url | pinned_date | getCode (len byte) | Trạng thái |
|---|---|---|---|---|---|
| Factory | `0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73` | https://developer.pancakeswap.finance/contracts/v2/addresses | 2026-09-14 | 19084 | PINNED |
| Router | `0x10ED43C718714eb63d5aA57B78B54704E256024E` | https://developer.pancakeswap.finance/contracts/v2/addresses | 2026-09-14 | 21936 | PINNED |

Fee V2: 0.25% tổng (0.17% về LP + 0.03% treasury + 0.05% buyback/burn) —
nguồn: https://docs.pancakeswap.finance/trade/trading-faq/swap-faq và
https://docs.pancakeswap.finance/earn/pancakeswap-pools. Math sandwich dùng
hệ số `9975/10000` (0.25% phí đầu vào) theo CLAUDE.md — khớp docs.

### V3 (`1.2`)

| Contract | Address | source_url | pinned_date | getCode (len byte) | Trạng thái |
|---|---|---|---|---|---|
| PancakeV3Factory | `0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865` | https://developer.pancakeswap.finance/contracts/v3/addresses | 2026-09-14 | 5151 | PINNED |
| PancakeV3PoolDeployer | `0x41ff9AA7e16B8B1a8a8dc4f0eFacd93D02d071c9` | https://developer.pancakeswap.finance/contracts/v3/addresses | 2026-09-14 | 24556 | PINNED |
| SwapRouter (v3) | `0x1b81D678ffb9C0263b24A97847620C99d213eB14` | https://developer.pancakeswap.finance/contracts/v3/addresses | 2026-09-14 | 12154 | PINNED |
| Smart Router (V2+V3+StableSwap) | `0x13f4EA83D0bd40E75C8222255bc855a974568Dd4` | https://developer.pancakeswap.finance/contracts/v3/addresses | 2026-09-14 | 24316 | PINNED |
| QuoterV2 | `0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997` | https://developer.pancakeswap.finance/contracts/v3/addresses | 2026-09-14 | 8331 | PINNED |
| v3 Universal Router (router gộp cũ, không phải Infinity UR) | `0x1A0A18AC4BECDDbd6389559687d1A73d8927E416` | https://developer.pancakeswap.finance/contracts/universal-router/addresses | 2026-09-14 | 16684 | PINNED |

Init code hash pool V3: docs không ghi giá trị hex trực tiếp trên trang
addresses đã fetch phiên này → không bịa, không cần vì decoder/resolve pool
dùng `Factory.getPool(tokenA, tokenB, fee)` on-chain (`eth_call`), không tính
CREATE2 offline. Nếu sau này cần init hash (tối ưu off-chain), ghi bổ sung
kèm nguồn cụ thể, không suy đoán.

Không pin trong phiên này (ngoài phạm vi sandwich V2/V3 swap-path, không
cần cho `2.x/3.x`): `NonfungiblePositionManager`, `V3Migrator`, `TickLens`,
`PancakeInterfaceMulticall`, `MixedRouteQuoterV1`, `TokenValidator`,
`MasterChefV3` — đều có địa chỉ BSC thật trong cùng trang docs trên nhưng là
hợp đồng quản lý LP/farm, không nằm trong đường swap bị sandwich → không pin,
không cần thiết cho mục tiêu bot.

### V4 / Infinity (`1.3`)

Trang địa chỉ chính thức: `https://developer.pancakeswap.finance/contracts/infinity/resources/addresses`
(cột "BNB & Base" = giá trị dùng cho BSC 56).

| Contract | Address | source_url | pinned_date | getCode (len byte) | Trạng thái |
|---|---|---|---|---|---|
| Vault (accounting layer) | `0x238a358808379702088667322f80aC48bAd5e6c4` | https://developer.pancakeswap.finance/contracts/infinity/resources/addresses | 2026-09-14 | 8347 | PINNED |
| CLPoolManager (concentrated liquidity) | `0xa0FfB9c1CE1Fe56963B0321B32E7A0302114058b` | https://developer.pancakeswap.finance/contracts/infinity/resources/addresses | 2026-09-14 | 20885 | PINNED |
| BinPoolManager (liquidity book / LB) | `0xC697d2898e0D09264376196696c51D7aBbbAA4a9` | https://developer.pancakeswap.finance/contracts/infinity/resources/addresses | 2026-09-14 | 23821 | PINNED |
| CLQuoter | `0xd0737C9762912dD34c3271197E362Aa736Df0926` | https://developer.pancakeswap.finance/contracts/infinity/resources/addresses | 2026-09-14 | 6998 | PINNED |
| BinQuoter | `0xC631f4B0Fc2Dd68AD45f74B2942628db117dD359` | https://developer.pancakeswap.finance/contracts/infinity/resources/addresses | 2026-09-14 | 6839 | PINNED |
| Universal Router (Infinity, router gộp CL+LB+V2+V3) | `0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB` | https://developer.pancakeswap.finance/contracts/universal-router/addresses | 2026-09-14 | 24350 | PINNED |

Không pin trong phiên này (ngoài phạm vi swap-path chính): `CLPositionManager`,
`BinPositionManager`, `MixedQuoter` (có địa chỉ thật trong cùng trang, dùng
cho quản lý vị thế LP / route trộn — chưa cần cho decoder `2.2`).

Infinity StableSwap (họ con của Infinity, factory riêng — ghi nhận, chưa bật
scan riêng vì `config.toml` không có cờ `scan_*` tách stableswap; nằm trong
phạm vi `scan_v4` khi decoder `2.x` đụng tới):

| Contract | Address | source_url | pinned_date | getCode (len byte) | Trạng thái |
|---|---|---|---|---|---|
| CLStableSwapPoolFactory | `0x3669dDD1a9ee009dB9Eb2174C5C760FFfc66cfeF` | https://developer.pancakeswap.finance/contracts/infinity-stableswap/addresses | 2026-09-14 | 3993 | PINNED (ghi nhận, chưa dùng trong `scan_v4`) |

### Bản mới hơn (nếu deploy)

| Family | Trạng thái |
|---|---|
| — | DISABLED — đã rà soát `developer.pancakeswap.finance` (sidebar đầy đủ: Infinity, v3, v2, Limit order, Infinity StableSwap, Universal Router, Aggregator, Permit2, PancakeSwap X, StableSwap, MasterChef, Cake, IFO, Lottery — không có mục family AMM nào mới hơn Infinity) + tìm kiếm web ngày 2026-09-14 ("PancakeSwap new AMM version 2026 after Infinity") — kết quả xác nhận Infinity (ra mắt 04/2025) vẫn là bản AMM mới nhất, PancakeSwap tháng 7/2026 chỉ mở rộng V2/V3 sang Robinhood chain (không phải BSC, không phải family mới). Không xoá khỏi mục tiêu — phiên sau nếu PancakeSwap công bố family mới trên BSC thì pin bổ sung. |

## Ghi chú

- `chain_id` mục tiêu duy nhất: `56` (BSC mainnet). Không dùng testnet cho pin.
- Cột `source_url` là link docs chính thức PancakeSwap fetch trực tiếp phiên này
  (không phải cache/suy đoán).
- Cột `getCode (len byte)` là kết quả `eth_getCode` thật trên `https://bsc-dataseed.binance.org/`
  (chain 56, xác nhận `eth_chainId=0x38` trước khi gọi) — xem BAOCAO02 ô 6 để có
  lệnh cURL + response rút gọn.
- `scan_v2/scan_v3/scan_v4=true` trong `config.toml`: sau phiên này cả 3 family
  đều đã pin → runtime KHÔNG còn skip `venue_unpinned` cho V2/V3/V4 nữa (skip đó
  chỉ áp dụng khi family thật sự chưa pin — không còn family nào trong 3 family
  chính ở trạng thái đó). `src/venues.rs::registry_snapshot` phản ánh đúng bảng
  này (`pinned=true` cho V2/V3/V4, `pinned=false` cho "Bản mới hơn").
- Pin ở đây KHÔNG bật live (`live_v2/live_v3/live_v4` vẫn `false` trong
  `config.toml` — không đổi trong phiên này).
- USDT (cụm `usdt-quote-asset`, phiên `2026-09-15`, BAOCAO29): `eth_chainId`
  xác nhận lại `0x38` + `eth_getCode` qua cùng RPC công khai
  `https://bsc-dataseed.binance.org/` (đúng tiền lệ BAOCAO02, không phải RPC
  runtime của bot). USDT KHÔNG thuộc family V2/V3/V4 nào — đây là QUOTE ASSET
  thứ 2 (ngang hàng WBNB, theo CLAUDE.md mục "Sản phẩm"), không phải router/
  factory/pool contract, nên không thêm hàng vào bảng V2/V3/V4 phía trên,
  đứng riêng ở mục Core cùng WBNB. `scan_quote_usdt=false` ship mặc định
  (`config.toml`) — pin ở đây chỉ mở đường, chưa bật quét mặc định.
