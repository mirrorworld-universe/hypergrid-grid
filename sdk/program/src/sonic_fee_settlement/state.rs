use {
    serde::{Deserialize, Serialize},
    solana_program::pubkey::Pubkey,
};

/// Program account states
#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum SettlementState {
    /// Account is not initialized.
    Uninitialized,
    /// Initialized `Settlement` account.
    FeeBillSettled(Vec<SettlementAccount>),
}

#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct SettlementAccount {
    pub owner: Pubkey,
    pub account_type: SettlementAccountType,
    pub amount: u64,
    pub withdrawable: u64,
    pub withdrawed: u64,
}

#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum SettlementAccountType {
    BurnAccount,
    HSSNAccount,
    SonicGridAccount,
    GridAccount,
}
