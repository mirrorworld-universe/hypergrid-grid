use {
    crate::{
        sonic_account_migrater::program::id,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
    },
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ProgramInstruction {
    ///Migrate remote accounts to local accounts cache
    MigrateRemoteAccounts,
    ///Deactivate remote accounts in local accounts cache
    DeactivateRemoteAccounts,
}

/// Constructs an instruction which migrate remote accounts to local accounts cache.
pub fn migrate_remote_accounts(
    payer_address: Pubkey,
    addresses: Vec<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(payer_address, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    for address in addresses {
        accounts.push(AccountMeta::new_readonly(address, false));
    }

    Instruction::new_with_bincode(
        id(),
        &ProgramInstruction::MigrateRemoteAccounts, // { addresses },
        accounts,
    )
}

/// Constructs an instruction that deactivates remote accounts in local accounts cache.
pub fn deactivate_remote_accounts(
    payer_address: Pubkey,
    addresses: Vec<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new_readonly(payer_address, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    for address in addresses {
        accounts.push(AccountMeta::new_readonly(address, false));
    }
    Instruction::new_with_bincode(
        id(),
        &ProgramInstruction::DeactivateRemoteAccounts,
        accounts,
    )
}