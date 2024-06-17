use {
    solana_accounts_db::remote_loader::RemoteAccountLoader, 
    solana_runtime::bank::Bank, 
    solana_sdk::{account::AccountSharedData, pubkey::Pubkey}, 
    sonic_printer::{func, show}, 
    std::collections::HashMap

};

pub(crate) fn get_account_from_overwrites_or_bank(
    pubkey: &Pubkey,
    bank: &Bank,
    overwrite_accounts: Option<&HashMap<Pubkey, AccountSharedData>>,
) -> Option<AccountSharedData> {
    overwrite_accounts
        .and_then(|accounts| accounts.get(pubkey).cloned())
        .or_else(|| bank.get_account(pubkey))
}

// Yusuf
pub(crate) fn get_account_from_remote(
    pubkey: &Pubkey,
    overwrite_accounts: Option<&HashMap<Pubkey, AccountSharedData>>,
) -> Option<AccountSharedData> {
    let remote_account_loader=  RemoteAccountLoader::new("https://rpc.hypergrid.dev");
    remote_account_loader.get_account(pubkey)
    // let remote_account_loader = RemoteAccountLoader::new();
    // remote_account_loader.get_account(pubkey)
}

