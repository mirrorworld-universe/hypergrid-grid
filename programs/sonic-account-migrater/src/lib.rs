#[cfg(not(target_os = "solana"))]
pub mod processor;

pub use solana_program::sonic_account_migrater::{
    instruction,
    program::{check_id, id, ID},
};
