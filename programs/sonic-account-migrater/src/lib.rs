#![allow(incomplete_features)]
#![cfg_attr(RUSTC_WITH_SPECIALIZATION, feature(specialization))]
#![cfg_attr(RUSTC_NEEDS_PROC_MACRO_HYGIENE, feature(proc_macro_hygiene))]

#[cfg(not(target_os = "solana"))]
pub mod processor;


pub use solana_program::sonic_account_migrater::{
    instruction,
    program::{check_id, id, ID},
};
