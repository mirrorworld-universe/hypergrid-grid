use {
    serde::{Deserialize, Serialize},
    solana_frozen_abi_macro::{AbiEnumVisitor, AbiExample},
    solana_program::pubkey::Pubkey,
};

/// Program account states
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
#[allow(clippy::large_enum_variant)]
pub enum MigratedAccountsState {
    /// Account is not initialized.
    Uninitialized,
    /// Initialized `MigratedAccounts` account.
    MigratedAccounts(Vec<MigratedAccount>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
pub struct MigratedAccount {
    pub address: Pubkey,
    pub source: Option<Pubkey>, 
    pub slot: u64,
}
