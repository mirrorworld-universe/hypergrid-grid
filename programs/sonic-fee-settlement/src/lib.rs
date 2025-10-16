#[cfg(not(target_os = "solana"))]
pub mod processor;

pub use solana_program::sonic_fee_settlement::{
    instruction,
    program::{check_id, id, ID},
};
