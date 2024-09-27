use {
    crate::pubkey::Pubkey,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ProgramInstruction {
    ///Migrate remote accounts to local accounts cache
    MigrateRemoteAccounts {
        addresses: Vec<Pubkey>,
    },
    ///Deactivate remote accounts in local accounts cache
    DeactivateRemoteAccounts {
        addresses: Vec<Pubkey>,
    },
    ///Migrate remote accounts from source to local accounts cache
    MigrateSourceAccounts {
        node_id: Pubkey,
        addresses: Vec<Pubkey>,
    },
}
