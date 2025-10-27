use {
    serde::{Deserialize, Serialize},
    solana_program::pubkey::Pubkey,
};

/// Program account states
#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MigratedAccountsState {
    /// Account is not initialized.
    Uninitialized,
    /// Initialized `MigratedAccounts` account.
    MigratedAccounts(Vec<MigratedAccount>),
}

#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct MigratedAccount {
    pub address: Pubkey,
    pub source: Option<Pubkey>,
    pub slot: u64,
}
