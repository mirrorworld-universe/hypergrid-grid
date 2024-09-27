//! The [Sonic account migrater program][np].
//!
//! [np]: 

pub mod instruction;
pub mod state;


pub mod program {
    crate::declare_id!("SonicAccountMigrater11111111111111111111111");
}

pub mod migrated_accounts {
    crate::declare_id!("SonicMigratedAccounts1111111111111111111112");
}
