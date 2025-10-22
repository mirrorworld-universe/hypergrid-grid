#[cfg(feature = "frozen-abi")]
use solana_frozen_abi_macro::{AbiEnumVisitor, AbiExample};
use {
    super::state::SettlementAccountType,
    crate::pubkey::Pubkey,
    serde::{Deserialize, Serialize},
};

#[cfg_attr(feature = "frozen-abi", derive(AbiExample))]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct SettlementBillParam {
    pub key: Pubkey,
    pub amount: u64,
}

#[cfg_attr(feature = "frozen-abi", derive(AbiExample, AbiEnumVisitor))]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ProgramInstruction {
    InitializeAccount {
        owner: Pubkey,
        account_type: SettlementAccountType,
    },
    /// Settle fee bill
    SettleFeeBill {
        from_id: u64,
        end_id: u64,
        bills: Vec<SettlementBillParam>,
    },
    /// Withdraw fee bill
    WithdrawFeeBill { address: Pubkey, amount: u64 },
}
