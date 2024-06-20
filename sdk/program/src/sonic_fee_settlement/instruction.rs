use {
    crate::{
        sonic_fee_settlement::program::id,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ProgramInstruction {
    ///Migrate remote accounts to local accounts cache
    SettleFeeBill {
        ///The remote account to be migrated
        remote_account: Pubkey,
        ///The local account to be updated
        local_account: Pubkey,
    },
    ///Deactivate remote accounts in local accounts cache
    WithdrawFeeBill {
        ///The remote account to be deactivated
        remote_account: Pubkey,
        ///The local account to be updated
        local_account: Pubkey,
    },
}
