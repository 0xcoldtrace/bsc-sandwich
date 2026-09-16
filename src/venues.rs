use alloy::primitives::Address;
use serde::Serialize;
use std::str::FromStr;
use std::sync::LazyLock;

/// Một contract đã pin (hoặc chưa) trong một family. Khớp hàng trong
/// `DEX_REGISTRY.md`. `get_code_len` là byte length thật từ `eth_getCode`
/// dán trong BAOCAO của phiên pin — `None` khi family chưa pin.
#[derive(Debug, Clone, Serialize)]
pub struct ContractPin {
    pub name: &'static str,
    pub address: &'static str,
    pub source_url: &'static str,
    pub pinned_date: &'static str,
    pub get_code_len: Option<u64>,
}

/// Snapshot khớp với `DEX_REGISTRY.md` tại thời điểm phiên `1.1+1.2+1.3`
/// (BAOCAO02, pinned_date 2026-09-14). Khi pin lại / thêm family, cập nhật
/// cả file này lẫn `DEX_REGISTRY.md` trong cùng phiên — không để hai nơi lệch
/// nhau.
#[derive(Debug, Clone, Serialize)]
pub struct VenueInfo {
    pub family: &'static str,
    pub pinned: bool,
    pub scan_enabled: bool,
    pub live_enabled: bool,
    pub status: &'static str,
    pub contracts: Vec<ContractPin>,
}

const PINNED_DATE: &str = "2026-09-14";
const V2_SRC: &str = "https://developer.pancakeswap.finance/contracts/v2/addresses";
const V3_SRC: &str = "https://developer.pancakeswap.finance/contracts/v3/addresses";
const UR_SRC: &str = "https://developer.pancakeswap.finance/contracts/universal-router/addresses";
const INFINITY_SRC: &str = "https://developer.pancakeswap.finance/contracts/infinity/resources/addresses";

/// WBNB — Core, pin cố định theo `DEX_REGISTRY.md` (không thuộc family nào,
/// dùng chung cho mọi venue vì pair bắt buộc token/WBNB).
pub const WBNB_ADDRESS: &str = "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c";
pub const WBNB_GET_CODE_LEN: u64 = 3124;

/// USDT (BSC-USD) — quote asset thứ 2 ngang hàng WBNB, pin cụm
/// `usdt-quote-asset` (BAOCAO29, `2026-09-15`, xem `DEX_REGISTRY.md`). Không
/// thuộc family V2/V3/V4 nào (không phải router/factory) — đây là địa chỉ
/// token dùng làm quote asset, giống WBNB ở trên.
pub const USDT_ADDRESS: &str = "0x55d398326f99059fF775485246999027B3197955";
pub const USDT_GET_CODE_LEN: u64 = 4413;

/// Các address pin dùng lặp lại ở nhiều module (`pool.rs`/`sim_v3.rs`/
/// `tax.rs`) — khai báo `pub const` một chỗ duy nhất, `registry_snapshot`
/// dưới đây tham chiếu lại đúng các hằng này để tránh lệch giá trị giữa 2
/// nơi (đã pin trong `DEX_REGISTRY.md`, phiên `1.1+1.2+1.3`/BAOCAO02).
pub const V2_FACTORY_ADDRESS: &str = "0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73";
pub const V2_ROUTER_ADDRESS: &str = "0x10ED43C718714eb63d5aA57B78B54704E256024E";
pub const V3_FACTORY_ADDRESS: &str = "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865";
pub const V3_QUOTER_V2_ADDRESS: &str = "0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997";

/// Cụm `foundation-fix-then-real-sim` (A2) — router `to` mà bot chấp nhận xử
/// lý TIẾP (gate rẻ tiền, 0 RPC, chạy TRƯỚC decode). Đây là phân loại theo
/// ĐỊA CHỈ ROUTER (`tx.to`) — KHÁC `decoder::SwapVenue` (A3, phân loại theo
/// HÀM/COMMAND đã decode, quyết định dùng sim V2 hay V3). Một router (vd
/// SmartRouter/Universal Router) có thể mang CẢ 2 SwapVenue tuỳ selector bên
/// trong — 2 khái niệm tách biệt có chủ đích.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Venue {
    V2,
    V3,
    SmartRouter,
    UniversalRouter,
}

impl Venue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Venue::V2 => "v2",
            Venue::V3 => "v3",
            Venue::SmartRouter => "smart_router",
            Venue::UniversalRouter => "universal_router",
        }
    }
}

/// Đúng 5 địa chỉ đã pin trong `DEX_REGISTRY.md` (V2 Router, V3 SwapRouter,
/// SmartRouter, UR v3-cũ, UR Infinity) — KHÔNG thêm router chưa pin.
pub static PANCAKE_ROUTERS: LazyLock<[(&str, Venue); 5]> = LazyLock::new(|| {
    [
        (V2_ROUTER_ADDRESS, Venue::V2),
        ("0x1b81D678ffb9C0263b24A97847620C99d213eB14", Venue::V3), // SwapRouter (v3)
        ("0x13f4EA83D0bd40E75C8222255bc855a974568Dd4", Venue::SmartRouter), // Smart Router
        ("0x1A0A18AC4BECDDbd6389559687d1A73d8927E416", Venue::UniversalRouter), // UR v3, cu
        ("0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB", Venue::UniversalRouter), // UR Infinity
    ]
});

/// `None` = `to` không phải 1 trong 5 router Pancake đã pin -> gate
/// `not_pancake_router` ở `main.rs`, 0 RPC, không decode.
pub fn venue_for_router(to: Address) -> Option<Venue> {
    PANCAKE_ROUTERS.iter().find_map(|(addr, venue)| {
        (Address::from_str(addr).expect("router da pin phai la address hop le") == to).then_some(*venue)
    })
}

pub fn registry_snapshot(scan_v2: bool, scan_v3: bool, scan_v4: bool, live_v2: bool, live_v3: bool, live_v4: bool) -> Vec<VenueInfo> {
    vec![
        VenueInfo {
            family: "V2",
            pinned: true,
            scan_enabled: scan_v2,
            live_enabled: live_v2,
            status: "PINNED",
            contracts: vec![
                ContractPin { name: "Factory", address: V2_FACTORY_ADDRESS, source_url: V2_SRC, pinned_date: PINNED_DATE, get_code_len: Some(19084) },
                ContractPin { name: "Router", address: V2_ROUTER_ADDRESS, source_url: V2_SRC, pinned_date: PINNED_DATE, get_code_len: Some(21936) },
            ],
        },
        VenueInfo {
            family: "V3",
            pinned: true,
            scan_enabled: scan_v3,
            live_enabled: live_v3,
            status: "PINNED",
            contracts: vec![
                ContractPin { name: "PancakeV3Factory", address: V3_FACTORY_ADDRESS, source_url: V3_SRC, pinned_date: PINNED_DATE, get_code_len: Some(5151) },
                ContractPin { name: "PancakeV3PoolDeployer", address: "0x41ff9AA7e16B8B1a8a8dc4f0eFacd93D02d071c9", source_url: V3_SRC, pinned_date: PINNED_DATE, get_code_len: Some(24556) },
                ContractPin { name: "SwapRouter", address: "0x1b81D678ffb9C0263b24A97847620C99d213eB14", source_url: V3_SRC, pinned_date: PINNED_DATE, get_code_len: Some(12154) },
                ContractPin { name: "SmartRouter", address: "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4", source_url: V3_SRC, pinned_date: PINNED_DATE, get_code_len: Some(24316) },
                ContractPin { name: "QuoterV2", address: V3_QUOTER_V2_ADDRESS, source_url: V3_SRC, pinned_date: PINNED_DATE, get_code_len: Some(8331) },
                ContractPin { name: "UniversalRouter (v3, cu)", address: "0x1A0A18AC4BECDDbd6389559687d1A73d8927E416", source_url: UR_SRC, pinned_date: PINNED_DATE, get_code_len: Some(16684) },
            ],
        },
        VenueInfo {
            family: "V4/Infinity",
            pinned: true,
            scan_enabled: scan_v4,
            live_enabled: live_v4,
            status: "PINNED",
            contracts: vec![
                ContractPin { name: "Vault", address: "0x238a358808379702088667322f80aC48bAd5e6c4", source_url: INFINITY_SRC, pinned_date: PINNED_DATE, get_code_len: Some(8347) },
                ContractPin { name: "CLPoolManager", address: "0xa0FfB9c1CE1Fe56963B0321B32E7A0302114058b", source_url: INFINITY_SRC, pinned_date: PINNED_DATE, get_code_len: Some(20885) },
                ContractPin { name: "BinPoolManager", address: "0xC697d2898e0D09264376196696c51D7aBbbAA4a9", source_url: INFINITY_SRC, pinned_date: PINNED_DATE, get_code_len: Some(23821) },
                ContractPin { name: "CLQuoter", address: "0xd0737C9762912dD34c3271197E362Aa736Df0926", source_url: INFINITY_SRC, pinned_date: PINNED_DATE, get_code_len: Some(6998) },
                ContractPin { name: "BinQuoter", address: "0xC631f4B0Fc2Dd68AD45f74B2942628db117dD359", source_url: INFINITY_SRC, pinned_date: PINNED_DATE, get_code_len: Some(6839) },
                ContractPin { name: "UniversalRouter (Infinity)", address: "0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB", source_url: UR_SRC, pinned_date: PINNED_DATE, get_code_len: Some(24350) },
            ],
        },
        VenueInfo {
            family: "Ban moi hon",
            pinned: false,
            scan_enabled: false,
            live_enabled: false,
            status: "DISABLED (chua co family AMM Pancake nao moi hon Infinity tren BSC, ra soat 2026-09-14)",
            contracts: vec![],
        },
    ]
}

/// Enum reason bắt buộc theo CLAUDE.md mục "Skip".
///
/// `not_quote_pair` — cụm `usdt-quote-asset` (BAOCAO29): dùng bởi entrypoint
/// quote-aware MỚI (`pipeline::decide_paper_quote`, song song `decide_paper`/
/// `decide_paper_v2` — 2 hàm đó vẫn dùng `not_wbnb_pair` y hệt cũ, KHÔNG đổi)
/// khi path đã decode được nhưng không khớp WBNB lẫn USDT (hoặc USDT bị tắt
/// qua `scan_quote_usdt=false`). Giữ CẢ HAI reason trong danh sách — không
/// xoá `not_wbnb_pair` (vẫn còn dùng ở 2 hàm cũ).
pub const SKIP_REASONS: &[&str] = &[
    "not_in_list",
    "below_min",
    "decode_fail",
    "not_wbnb_pair",
    "not_quote_pair",
    "sell_direction",
    "not_pancake_router",
    "venue_unpinned",
    "no_pool",
    // Cum `hotpath-fix-then-decoder-ur` (A3) - tach khoi "no_pool".
    "rpc_error",
    "thin_liq",
    "deadline",
    "victim_would_revert",
    "unprofitable",
    "honeypot_or_tax",
    "hooks_unread",
    // Cum `evm-validate-fixed-then-wire` (B3.2)
    "sim_error",
    // Cum `exec-path-traps` (F-13)
    "nonce_stale",
    "nonce_future",
    // Cum `real-economics-mode2` (F-03)
    "gas_cap",
    // Cum `bugfix-presign-and-contract-plan` (A2) - cong tinh tao truoc
    // `Simulated` (front_in <=10% reserve, profit_net <=2% reserve,
    // victim amount_in <= reserve), xem `pipeline::sanity_check`.
    "sanity_reject",
    // Cum `bugfix-presign-and-contract-plan` (A3) - tx.from thuoc cum doi thu
    // da nhan dien + allow_competitor_victims=false + live_mode != "off".
    "competitor_victim",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_v3_v4_are_pinned_after_registry_session() {
        let venues = registry_snapshot(true, true, true, false, false, false);
        assert_eq!(venues.len(), 4);
        let pinned_families: Vec<&str> = venues.iter().filter(|v| v.pinned).map(|v| v.family).collect();
        assert_eq!(pinned_families, vec!["V2", "V3", "V4/Infinity"]);
    }

    #[test]
    fn newer_family_still_disabled_not_deleted() {
        let venues = registry_snapshot(true, true, true, false, false, false);
        let newer = venues.iter().find(|v| v.family == "Ban moi hon").expect("phai con hang Ban moi hon");
        assert!(!newer.pinned);
        assert!(newer.status.contains("DISABLED"));
    }

    #[test]
    fn every_pinned_contract_has_nonzero_get_code_len() {
        let venues = registry_snapshot(true, true, true, false, false, false);
        for v in venues.iter().filter(|v| v.pinned) {
            assert!(!v.contracts.is_empty(), "family {} pinned nhung khong co contract nao", v.family);
            for c in &v.contracts {
                assert!(
                    c.get_code_len.unwrap_or(0) > 0,
                    "contract {} family {} thieu get_code_len > 0",
                    c.name,
                    v.family
                );
                assert!(c.address.starts_with("0x") && c.address.len() == 42);
                assert!(!c.source_url.is_empty());
            }
        }
    }

    #[test]
    fn wbnb_pinned_and_nonzero() {
        assert_eq!(WBNB_ADDRESS.len(), 42);
        assert!(WBNB_GET_CODE_LEN > 0);
    }

    #[test]
    fn usdt_pinned_and_nonzero() {
        assert_eq!(USDT_ADDRESS.len(), 42);
        assert!(USDT_GET_CODE_LEN > 0);
    }

    #[test]
    fn skip_reasons_contains_both_not_wbnb_pair_and_not_quote_pair() {
        assert!(SKIP_REASONS.contains(&"not_wbnb_pair"), "khong duoc xoa reason cu");
        assert!(SKIP_REASONS.contains(&"not_quote_pair"), "phai co reason moi cho quote-aware path");
    }

    /// Cụm `exec-path-traps` (F-13) — ĐẠT CẦN DÁN "Thêm 2 reason vào
    /// SKIP_REASONS".
    #[test]
    fn skip_reasons_contains_nonce_stale_and_nonce_future() {
        assert!(SKIP_REASONS.contains(&"nonce_stale"));
        assert!(SKIP_REASONS.contains(&"nonce_future"));
    }

    #[test]
    fn skip_reasons_contains_not_pancake_router_and_sell_direction() {
        assert!(SKIP_REASONS.contains(&"not_pancake_router"));
        assert!(SKIP_REASONS.contains(&"sell_direction"));
    }

    /// ĐẠT CẦN DÁN: tx tới router giả (Biswap/ApeSwap-style, không nằm trong
    /// 5 router đã pin) -> `None`, đúng gate `not_pancake_router`.
    #[test]
    fn venue_for_router_none_for_unknown_router() {
        let biswap_like = Address::from_str("0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8").unwrap();
        assert_eq!(venue_for_router(biswap_like), None);
    }

    #[test]
    fn venue_for_router_matches_all_5_pinned_routers() {
        let v2 = Address::from_str(V2_ROUTER_ADDRESS).unwrap();
        assert_eq!(venue_for_router(v2), Some(Venue::V2));
        let v3 = Address::from_str("0x1b81D678ffb9C0263b24A97847620C99d213eB14").unwrap();
        assert_eq!(venue_for_router(v3), Some(Venue::V3));
        let smart = Address::from_str("0x13f4EA83D0bd40E75C8222255bc855a974568Dd4").unwrap();
        assert_eq!(venue_for_router(smart), Some(Venue::SmartRouter));
        let ur_old = Address::from_str("0x1A0A18AC4BECDDbd6389559687d1A73d8927E416").unwrap();
        assert_eq!(venue_for_router(ur_old), Some(Venue::UniversalRouter));
        let ur_infinity = Address::from_str("0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB").unwrap();
        assert_eq!(venue_for_router(ur_infinity), Some(Venue::UniversalRouter));
    }

    #[test]
    fn scan_and_live_flags_pass_through_unchanged() {
        let venues = registry_snapshot(false, true, false, false, true, false);
        let v2 = venues.iter().find(|v| v.family == "V2").unwrap();
        let v3 = venues.iter().find(|v| v.family == "V3").unwrap();
        let v4 = venues.iter().find(|v| v.family == "V4/Infinity").unwrap();
        assert!(!v2.scan_enabled && !v2.live_enabled);
        assert!(v3.scan_enabled && v3.live_enabled);
        assert!(!v4.scan_enabled && !v4.live_enabled);
    }
}

/// Cụm `evm-validate-fixed-then-wire` (C3) — 8 token quote/blue-chip mà tax
/// fee-on-transfer chắc chắn = 0, cho phép BỎ QUA bước đo tax bằng EVM (tiết
/// kiệm 1 fork + 4 tx EVM cho mỗi candidate dùng các token này).
///
/// **Đã verify THẬT phiên này** (`eth_getCode` + `symbol()` qua
/// `bsc-rpc.publicnode.com`, bảng dán ở BAOCAO33 ô 6) — KHÔNG chép từ trí nhớ:
/// mọi địa chỉ đều có `getCode > 0` và `symbol()` khớp tên dưới đây. WBNB
/// (`3124`) và USDT (`4413`) khớp đúng `WBNB_GET_CODE_LEN`/`USDT_GET_CODE_LEN`
/// đã pin từ cụm registry `1.1+1.2+1.3` — một lớp đối chiếu chéo nữa.
///
/// Danh sách này CHỈ miễn bước ĐO TAX; nó KHÔNG miễn bất kỳ cổng nào khác
/// (`thin_liq`, `min_profit`, `victim_would_revert`, cổng live...).
pub const ZERO_TAX_ALLOWLIST: [(&str, &str); 8] = [
    ("WBNB", WBNB_ADDRESS),
    ("USDT", USDT_ADDRESS),
    ("USDC", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
    ("BUSD", "0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56"),
    ("USD1", "0x8d0D000Ee44948FC98c9B98A4FA4921476f08B0d"),
    ("CAKE", "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82"),
    ("BTCB", "0x7130d2A12B9BCbFAe4f2634d864A1Ee1Ce3Ead9c"),
    ("ETH", "0x2170Ed0880ac9A755fd29B2688956BD959F933F8"),
];

/// `true` nếu `token` nằm trong `ZERO_TAX_ALLOWLIST` (so sánh theo `Address`
/// đã parse, KHÔNG so chuỗi — tránh lệch hoa/thường checksum EIP-55).
pub fn is_zero_tax_allowlisted(token: alloy::primitives::Address) -> bool {
    use std::str::FromStr as _;
    ZERO_TAX_ALLOWLIST
        .iter()
        .any(|(_, a)| alloy::primitives::Address::from_str(a).map(|x| x == token).unwrap_or(false))
}

/// Địa chỉ WBNB/USDT đã parse (dùng ở live loop cụm `evm-validate-fixed-then-wire`).
pub fn wbnb_addr() -> alloy::primitives::Address {
    use std::str::FromStr as _;
    alloy::primitives::Address::from_str(WBNB_ADDRESS).expect("WBNB_ADDRESS da pin hop le")
}
pub fn usdt_addr() -> alloy::primitives::Address {
    use std::str::FromStr as _;
    alloy::primitives::Address::from_str(USDT_ADDRESS).expect("USDT_ADDRESS da pin hop le")
}
