//! Cụm `bugfix-presign-and-contract-plan` (A3) — nhận diện victim thuộc CỤM
//! ĐỐI THỦ MEV đã trinh sát được (BAOCAO41 mục 1 + "SỬA GIỮA PHIÊN").
//!
//! # Cụm này là gì (bằng chứng on-chain THẬT, không suy diễn)
//!
//! Trinh sát ở `competitor-recon-and-strategy` xác nhận một hệ thống bot
//! ĐA-VÍ đang hoạt động trên BSC:
//!
//! - `0xB406021E07b31E1f7850FCcCD7076094f18d07eF` — EOA "kho" (nonce >350.000),
//!   nguồn vốn USDT.
//! - `0xa739Dfab40ef6585f1174fcE90EC96330669758c` — contract "dispatcher":
//!   mỗi lời gọi `0x5aab2274` phát ra một loạt cặp
//!   `(Transfer USDT từ 0xB406 → ví X, Approval(0xB406, 0xa739))`, tức CẤP
//!   VỐN TỨC THÌ cho nhiều ví "burner" cùng lúc (verify thật: tx
//!   `0x4916caa0f1…` block `122082156` cấp vốn 3 ví, cả 3 swap ngay 3 vị trí
//!   tx_index kế tiếp trên CÙNG pool).
//! - `0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54` — ví TRUNG TÂM (nhận vốn
//!   2850 lần / 50.000 block, áp đảo ví xếp thứ 2 chỉ 5 lần).
//!
//! Vì các ví thực sự SWAP là ví dùng-một-lần (không thể liệt kê tĩnh), nhận
//! diện phải ĐỘNG: **bất kỳ ví nào nhận Transfer quote-asset từ một địa chỉ
//! seed trong block hiện tại hoặc block liền trước đều được coi là thuộc
//! cụm** — đúng cơ chế "cấp vốn tức thì rồi swap ngay" đã quan sát.
//!
//! # Dùng để làm gì
//!
//! Đây KHÔNG phải nạn nhân bình thường: front-run một chân của bot khác là
//! đối đầu trực tiếp với hệ thống có sẵn hạ tầng gửi bundle riêng. Cờ
//! `victim_in_competitor_cluster` được ghi vào `sim.result`, đếm riêng trong
//! `/api/econ`, và khi `allow_competitor_victims=false` (ship) + `live_mode`
//! khác `"off"` thì candidate nhóm này KHÔNG được `Simulated` (skip
//! `competitor_victim`).

use alloy::primitives::{Address, B256};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// 3 địa chỉ SEED (lệnh A3) — địa chỉ ĐẦY ĐỦ đã verify on-chain ở BAOCAO41.
pub const SEED_ADDRESSES: [&str; 3] = [
    "0xB406021E07b31E1f7850FCcCD7076094f18d07eF", // EOA kho USDT
    "0xa739Dfab40ef6585f1174fcE90EC96330669758c", // contract dispatcher (selector 0x5aab2274)
    "0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54", // vi trung tam (2850 lan nhan von/50k block)
];

/// `keccak256("Transfer(address,address,uint256)")` — suy runtime, KHÔNG
/// hardcode chuỗi hex (cùng nguyên tắc `pool::sync_topic0`).
pub fn transfer_topic0() -> B256 {
    alloy::primitives::keccak256(b"Transfer(address,address,uint256)")
}

/// Địa chỉ 20 byte nằm ở 12 byte cuối của 1 topic 32 byte (chuẩn ABI index).
pub fn address_from_topic(topic: B256) -> Address {
    Address::from_slice(&topic.as_slice()[12..])
}

pub fn seed_set() -> HashSet<Address> {
    SEED_ADDRESSES.iter().filter_map(|s| Address::from_str(s).ok()).collect()
}

/// Số block giữ lại danh sách ví vừa được seed cấp vốn — lệnh A3 yêu cầu
/// "block hiện tại/trước", tức 2 block.
pub const FUNDED_WINDOW_BLOCKS: usize = 2;

/// Chỉ mục cụm đối thủ: seed TĨNH + ví được cấp vốn ĐỘNG theo block.
#[derive(Debug, Default)]
pub struct ClusterIndex {
    seeds: HashSet<Address>,
    /// `block -> tập ví nhận Transfer quote từ seed trong block đó`.
    funded: HashMap<u64, HashSet<Address>>,
    /// Tổng số lần ghi nhận 1 ví mới được cấp vốn (thống kê cho `/api/compete`).
    funded_total: u64,
}

impl ClusterIndex {
    pub fn new() -> Self {
        Self { seeds: seed_set(), funded: HashMap::new(), funded_total: 0 }
    }

    /// Ghi nhận 1 ví vừa nhận quote-asset từ seed tại `block`.
    pub fn note_funded(&mut self, block: u64, wallet: Address) {
        self.funded.entry(block).or_default().insert(wallet);
        self.funded_total += 1;
        // Chỉ giữ `FUNDED_WINDOW_BLOCKS` block MỚI NHẤT.
        if self.funded.len() > FUNDED_WINDOW_BLOCKS {
            let mut blocks: Vec<u64> = self.funded.keys().copied().collect();
            blocks.sort_unstable();
            let drop_n = blocks.len() - FUNDED_WINDOW_BLOCKS;
            for b in blocks.into_iter().take(drop_n) {
                self.funded.remove(&b);
            }
        }
    }

    /// `true` khi `addr` là seed, HOẶC vừa nhận quote từ seed trong
    /// `current_block`/block liền trước.
    pub fn contains(&self, addr: Address, current_block: u64) -> bool {
        if self.seeds.contains(&addr) {
            return true;
        }
        for b in [current_block, current_block.saturating_sub(1)] {
            if self.funded.get(&b).map(|s| s.contains(&addr)).unwrap_or(false) {
                return true;
            }
        }
        false
    }

    pub fn is_seed(&self, addr: Address) -> bool {
        self.seeds.contains(&addr)
    }

    pub fn funded_total(&self) -> u64 {
        self.funded_total
    }

    pub fn funded_now(&self) -> usize {
        self.funded.values().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    #[test]
    fn seed_set_has_exactly_the_three_verified_addresses() {
        let set = seed_set();
        assert_eq!(set.len(), 3, "3 dia chi seed phai parse duoc het");
        assert!(set.contains(&a("0xB406021E07b31E1f7850FCcCD7076094f18d07eF")));
        assert!(set.contains(&a("0xa739Dfab40ef6585f1174fcE90EC96330669758c")));
        assert!(set.contains(&a("0x8180aD6A7c9f8f4864e9909480fbA4123FcE6c54")));
    }

    /// ĐẠT CẦN DÁN (A3) — victim `0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7`
    /// (ví trong dòng A1 của lệnh, đã verify on-chain BAOCAO41: nhận 2757.93
    /// USDT từ `0xB406` ở block `122076185` rồi swap NGAY tx kế tiếp) PHẢI
    /// được nhận diện là thuộc cụm — dù KHÔNG nằm trong danh sách seed tĩnh.
    #[test]
    fn funded_wallet_from_real_block_122076185_is_in_cluster() {
        let victim = a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7");
        let mut idx = ClusterIndex::new();
        assert!(!idx.contains(victim, 122_076_185), "truoc khi thay Transfer thi chua biet gi - khong duoc doan");
        idx.note_funded(122_076_185, victim);
        assert!(idx.contains(victim, 122_076_185), "cung block -> trong cum");
        assert!(idx.contains(victim, 122_076_186), "block ke tiep -> van trong cum (cua so 2 block)");
        assert!(!idx.contains(victim, 122_076_190), "xa hon cua so -> khong con tinh");
    }

    #[test]
    fn seed_address_is_always_in_cluster_regardless_of_block() {
        let idx = ClusterIndex::new();
        assert!(idx.contains(a("0xB406021E07b31E1f7850FCcCD7076094f18d07eF"), 1));
        assert!(idx.contains(a("0xa739Dfab40ef6585f1174fcE90EC96330669758c"), 999_999_999));
        assert!(!idx.contains(a("0x1111111111111111111111111111111111111111"), 1));
    }

    #[test]
    fn funded_window_keeps_only_two_newest_blocks() {
        let mut idx = ClusterIndex::new();
        let w1 = a("0x1111111111111111111111111111111111111111");
        let w2 = a("0x2222222222222222222222222222222222222222");
        let w3 = a("0x3333333333333333333333333333333333333333");
        idx.note_funded(100, w1);
        idx.note_funded(101, w2);
        idx.note_funded(102, w3);
        assert!(!idx.contains(w1, 102), "block 100 da bi day ra khoi cua so 2 block");
        assert!(idx.contains(w2, 102));
        assert!(idx.contains(w3, 102));
        assert_eq!(idx.funded_total(), 3);
    }

    #[test]
    fn address_from_topic_reads_last_20_bytes() {
        let mut raw = [0u8; 32];
        raw[12..].copy_from_slice(a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7").as_slice());
        assert_eq!(address_from_topic(B256::from(raw)), a("0xaaBae02D453823E0CE3C86f8A1d29d3Da0a3eaf7"));
    }

    #[test]
    fn transfer_topic0_matches_known_erc20_signature() {
        // Gia tri chuan cua ERC20 Transfer - doi chieu voi hex cong bo rong rai.
        assert_eq!(
            format!("{:#x}", transfer_topic0()),
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        );
    }
}
