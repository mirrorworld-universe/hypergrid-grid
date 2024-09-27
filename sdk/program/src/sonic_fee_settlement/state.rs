use {
    serde::{Deserialize, Serialize},
    solana_frozen_abi_macro::{AbiEnumVisitor, AbiExample},
    solana_program::pubkey::Pubkey,
};

/// Program account states
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
#[allow(clippy::large_enum_variant)]
pub enum SettlementState {
    /// Account is not initialized.
    Uninitialized,
    /// Initialized `Settlement` account.
    FeeBillSettled(Vec<SettlementAccount>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
pub struct SettlementAccount {
    pub owner: Pubkey,
    pub account_type: SettlementAccountType, 
    pub amount: u64,
    pub withdrawable: u64,
    pub withdrawed: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
pub enum SettlementAccountType {
    BurnAccount,
    HSSNAccount,
    SonicGridAccount,
    GridAccount,
}