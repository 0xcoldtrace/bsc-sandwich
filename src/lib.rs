//! Lib crate — cho phép các binary khác (`src/bin/rpc_probe.rs`) dùng lại
//! đúng logic parse/redact/failover RPC (`transport.rs`) thay vì chép lại.
//! `src/main.rs` (binary chính, `bsc_sandwich`) dùng lại các module này qua
//! `use bsc_sandwich::...` thay vì tự khai `mod ...` như trước cụm
//! `rpc-probe`. Không đổi nội dung bất kỳ module nào — chỉ đổi nơi khai báo.

pub mod calldata;
pub mod competitor;
pub mod config;
pub mod decoder;
pub mod executor;
pub mod logger;
pub mod mem;
pub mod pairbook;
pub mod pipeline;
pub mod pool;
pub mod relay;
pub mod shadow;
pub mod sim_evm;
pub mod sim_v2;
pub mod sim_v3;
pub mod state;
pub mod tax;
pub mod transport;
pub mod venues;
pub mod victims;
pub mod web;
