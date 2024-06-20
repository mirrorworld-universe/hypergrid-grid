use {
    sonic_hypergrid::remote_loader::RemoteAccountLoader, 
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
    show!(file!(), line!(), func!(), overwrite_accounts);
    show!(file!(), line!(), func!(), bank.get_account(pubkey));
    overwrite_accounts
        .and_then(|accounts| accounts.get(pubkey).cloned())
        .or_else(|| bank.get_account(pubkey))
}

// // Yusuf
// pub(crate) fn get_account_from_remote(
//     pubkey: &Pubkey,
//     overwrite_accounts: Option<&HashMap<Pubkey, AccountSharedData>>,
// ) -> Option<AccountSharedData> {
//     show!(file!(), line!(), func!(), pubkey);
//     let remote_account_loader=  RemoteAccountLoader::new("https://rpc.hypergrid.dev");
//     show!(file!(), line!(), func!(), pubkey);
//     remote_account_loader.get_account(pubkey)
//     // let remote_account_loader = RemoteAccountLoader::new();
//     // show!(file!(), line!(), func!(), pubkey);
//     // remote_account_loader.get_account(pubkey)
// }

