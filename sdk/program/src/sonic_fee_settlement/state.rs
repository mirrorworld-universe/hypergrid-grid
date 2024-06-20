use {
    serde::{Deserialize, Serialize},
    solana_frozen_abi_macro::{AbiEnumVisitor, AbiExample},
    solana_program::{
        address_lookup_table::error::AddressLookupError,
        clock::Slot,
        instruction::InstructionError,
        pubkey::Pubkey,
        slot_hashes::{SlotHashes, MAX_ENTRIES},
    },
    std::borrow::Cow,
};

/// Program account states
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, AbiExample, AbiEnumVisitor)]
#[allow(clippy::large_enum_variant)]
pub enum SettlementState {
    /// Account is not initialized.
    Uninitialized,
    /// Initialized `LookupTable` account.
    FeeBillSettled(ProfileBill),
}


struct ProfileBill {

}