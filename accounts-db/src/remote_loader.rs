use {
    dashmap::DashMap,
    solana_client::rpc_client::RpcClient,  
    solana_sdk::{
        account::{AccountSharedData, ReadableAccount, WritableAccount}, 
        account_utils::StateMut, 
        bpf_loader_upgradeable::{self, UpgradeableLoaderState}, 
        commitment_config::CommitmentConfig, 
        instruction::{AccountMeta, Instruction}, 
        pubkey::Pubkey, 
        signature::{Keypair, Signature, Signer}, 
        transaction::Transaction
    },
    solana_measure::measure::Measure,
    std::{
        fmt, option_env, time::Duration, //thread, //str::FromStr, 
        fs::File, io,
        path::Path,
    },
    serde_derive::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
};


fn load_config_file<T, P>(config_file: P) -> Result<T, io::Error>
where
    T: serde::de::DeserializeOwned,
    P: AsRef<Path>,
{
    let file = File::open(config_file)?;
    let config = serde_yaml::from_reader(file)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, format!("{err:?}")))?;
    Ok(config)
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Config {
    pub baselayer_rpc_url: String,
    pub keypair_base58: String,
    pub sonic_program_id: String,
}

impl Default for Config {
    fn default() -> Self {
        let keypair_base58 = "5gA6JTpFziXu7py2j63arRUq1H29p6pcPMB74LaNuzcSqULPD6s1SZUS3UMPvFEE9oXmt1kk6ez3C6piTc3bwpJ6".to_string();
        let baselayer_rpc_url = "https://api.devnet.solana.com".to_string();
        let sonic_program_id ="4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z".to_string();

        Self {
            baselayer_rpc_url,
            keypair_base58,
            sonic_program_id,
        }
    }
}

impl Config {
    /// Load a configuration from file.
    ///
    /// # Errors
    ///
    /// This function may return typical file I/O errors.
    pub fn load(config_file: &str) -> Result<Self, io::Error> {
        load_config_file(config_file)
    }
}

type AccountCacheKeyMap = DashMap<Pubkey, AccountSharedData>;

pub struct RemoteAccountLoader {
    ///RPC client used to send requests to the remote.
    rpc_client: RpcClient,
    /// Cache of accounts loaded from the remote.
    account_cache: AccountCacheKeyMap,
    /// Enable or disable the remote loader.
    enable: bool,
    config: Config,
}

impl fmt::Debug for RemoteAccountLoader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RemoteAccountLoader")
            //.field("gzip", &self.inner.gzip)
            //.field("redirect_policy", &self.inner.redirect_policy)
            //.field("referer", &self.inner.referer)
            .finish()
    }
}

impl Default for RemoteAccountLoader {
    fn default() -> Self {
        let config_path: Option<&'static str> = option_env!("SONIC_CONFIG_FILE");
        let default_config_path = {
            let mut default_config_path = dirs_next::home_dir().expect("home directory");
            default_config_path.extend([".config", "hypergrid.yml"]);
            default_config_path.to_str().unwrap().to_string()
        };
        Self::new(config_path.unwrap_or(default_config_path.as_str()))
    }
}

// const SONIC_PROGRAM_ID: &str = "4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z";

#[derive(Serialize, Deserialize)]
struct SetValueInstruction {
    pub instruction: [u8;8],
    pub value: u64,
}

#[derive(Serialize, Deserialize)]
struct SetLockerInstruction {
    pub instruction: [u8;8],
    pub locker: Pubkey,
}

fn hash_instruction_method(method: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", method));
    let result = hasher.finalize();
    let mut hash = [0u8; 8];
    hash.copy_from_slice(&result[..8]);
    hash
}

/// Remote account loader.
impl RemoteAccountLoader {
    /// Create a new remote loader.
    pub fn new(config_path: &str) -> Self {
        let mut config = Config::default();
        match Config::load(config_path) {
            Ok(setting) => {
                config = setting;

                // let key = Keypair::from_base58_string(&setting.keypair_base58); 
                // let program_id = Pubkey::from_str(&setting.sonic_program_id).unwrap();
                // println!("setting: {:?}, {:?}, {:?}", &setting.baselayer_rpc_url, key, program_id)
            },
            Err(e) => {
                println!("setting: {:?}", e);
            },
        };

        Self {
            rpc_client: RpcClient::new_with_timeout_and_commitment(&config.baselayer_rpc_url, 
            Duration::from_secs(30), CommitmentConfig::confirmed()),
            account_cache: AccountCacheKeyMap::default(),
            enable: true,
            config,
        }
    }

    /// Check if the account should be ignored.
    fn ignored_account(pubkey: &Pubkey) -> bool {
        let pk = pubkey.to_string();
        if pk.contains("1111111111111111")
            // || pk.starts_with("Memo") 
            // || pk.starts_with("Token") 
            // || pk.starts_with("AToken") 
        {
            return true;
        }
        false
    }

    /// Get the account from the cache.
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if !self.enable || Self::ignored_account(pubkey) {
            return None;
        }
        match self.account_cache.get(pubkey) {
            Some(account) =>    {
                return Some(account.clone());
            },
            None => None, // self.load_account(pubkey),
        }
    }

    /// Check if the account is in the cache.
    pub fn has_account(&self, pubkey: &Pubkey) -> bool {
        if !self.enable || Self::ignored_account(pubkey) {
            return false;
        }
        
        match self.account_cache.contains_key(pubkey) {
            true => true,
            false => false, //self.load_account(pubkey).is_some(),
        }
    }

    fn deserialize_from_json(account_data: serde_json::Value) -> Option<AccountSharedData> {
        let result = &account_data["result"];
        if result.is_null() {
            return None;
        }
        
        let value = &result["value"];
        if value.is_null() {
            return None;
        }
   

        // let slot = result["context"]["slot"].as_u64().unwrap_or(0);
        let data = value["data"][0].as_str().unwrap_or("");
        let encoding = value["data"][1].as_str().unwrap_or("");
        let lamports = value["lamports"].as_u64().unwrap_or(0);
        let owner = value["owner"].as_str().unwrap_or("");
        let rent_epoch = value["rentEpoch"].as_u64().unwrap_or(0);
        let space = value["space"].as_u64().unwrap();
        let executable = value["executable"].as_bool().unwrap_or(false);
        // if owner.eq("Feature111111111111111111111111111111111111") {
        //     return None;
        // }

        let data = match encoding {
            "base58" => bs58::decode(data).into_vec().unwrap_or_default(),
            "base64" => base64::engine::general_purpose::STANDARD.decode(data).unwrap_or_default(),
            "base64+zstd" => {
                let decoded = base64::engine::general_purpose::STANDARD.decode(data).unwrap_or_default();
                let decompressed = zstd::decode_all(decoded.as_slice()).unwrap_or_default();
                decompressed
            },
            _ => Vec::new(), // Add wildcard pattern to cover all other possible values
        };
    
        
        let mut account = AccountSharedData::create(
                lamports,
                data,
                Pubkey::from_str(owner).unwrap(),
                executable,
                rent_epoch
        );
        account.remote = true;
    
        Some(account)
    }
    

    pub fn load_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if !self.enable || Self::ignored_account(pubkey) {
            return None;
        }
        match self.load_account_via_rpc(pubkey) {
            Some(account) => {
                //Sonic: check if programdata account exists
                if let Some(programdata_address) = RemoteAccountLoader::has_programdata_account(account.clone()) {
                    //Sonic: load programdata account from remote
                    self.load_account(&programdata_address);
                }
                Some(account)
            },
            None => None,
        }
    }

    /// Load the account from the RPC.
    fn load_account_via_rpc(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            return None;
        }
        
        let mut time = Measure::start("load_account_from_remote");
        let result = self.rpc_client.get_account(pubkey);
        match result {
            Ok(account) => {
                
                let mut account = AccountSharedData::create(
                    account.lamports,
                    account.data,
                    account.owner,
                    account.executable,
                    account.rent_epoch
                );
                account.remote = true;
        
                self.account_cache.insert(pubkey.clone(), account.clone());
                time.stop();
                
                Some(account)
            },
            Err(e) => {
                
                None
            }
        }
    }

    fn load_account_from_remote(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            return None;
        }
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                pubkey.to_string(),
                {
                    "encoding": "base64+zstd" //"base58"
                }
            ]
        });

        let client = self.client.clone();
        let url = self.url.clone();
        let call = thread::spawn(move || {
            let res = client.post(url)
                .header(CONTENT_TYPE, "application/json")
                .body(req.to_string())
                .send().unwrap();
            if res.status().is_success() {
                let account_json: serde_json::Value = res.json().unwrap();
                
                RemoteAccountLoader::deserialize_from_json(account_json)
            } else {
                None
            }
        });
        let result = call.join().unwrap();
        match result {
            Some(account) => {
                self.account_cache.insert(pubkey.clone(), account.clone());
                Some(account)
            },
            None => {
                None
            }
        }
        
        
    }

    pub fn has_programdata_account(program_account: AccountSharedData) -> Option<Pubkey> {
        if program_account.executable() && !bpf_loader_upgradeable::check_id(program_account.owner()) {
           return None;
        }

        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = program_account.state()
        {
            return Some(programdata_address);
        }

        return None;
    }

    /// Deactivate the account in the cache.
    pub fn deactivate_account(&self, pubkey: &Pubkey) {
        if !self.enable || Self::ignored_account(pubkey) {
            return;
        }
        // println!("RemoteAccountLoader.deactivate_account: {}", pubkey.to_string());
        match self.get_account(pubkey) {
            Some(account) => {
                self.account_cache.remove(pubkey);

                //remove the related programdata account
                match Self::has_programdata_account(account) {
                    Some(programdata_address) => {
                        self.account_cache.remove(&programdata_address);
                    },
                    None => { },
                }
            },
            None => {},
        }
        
    }

    /// Check if the account is a sonic program.
    pub fn is_sonic_program(&self, pubkey: &Pubkey) -> bool {
        if pubkey.to_string().eq(&self.config.sonic_program_id) {
            return self.has_account(pubkey);
        }
        false
    }

    /// Send a transaction to the base layer to update the status of the account.
    pub fn send_status_to_baselayer(&self, program_id: &Pubkey, account: &Pubkey, value:u64) -> Option<Signature> {
        let mut time = Measure::start("load_account_from_remote");
        let payer = Keypair::from_base58_string(&self.config.keypair_base58);
        // let program_id = Pubkey::from_str(SONIC_PROGRAM_ID).unwrap();

        let setlocker_data = SetLockerInstruction {
            instruction: hash_instruction_method("setlocker"), //[0x20, 0xda, 0x0f, 0x29, 0x6e, 0x40, 0xf2, 0x0f],
            locker: payer.pubkey(),
        };
        let setvalue_data = SetValueInstruction {
            instruction: hash_instruction_method("setvalue"), //[0x60, 0xca, 0x6c, 0x93, 0x6b, 0x11, 0x69, 0x5f],
            value,
        };

        let mut transaction = Transaction::new_with_payer(
            &[
                Instruction::new_with_bincode(
                    *program_id,
                    &setlocker_data,
                    vec![
                        // AccountMeta::new_readonly(payer.pubkey(), true),
                        AccountMeta::new(*account, false),
                        AccountMeta::new(payer.pubkey(), false),
                    ]
                ),
                Instruction::new_with_bincode(
                    *program_id,
                    &setvalue_data,
                    vec![
                        // AccountMeta::new_readonly(payer.pubkey(), true),
                        AccountMeta::new(*account, false),
                        AccountMeta::new(payer.pubkey(), false),
                    ]
                ),
            ],
            Some(&payer.pubkey()),
        );
        let blockhash = self.rpc_client.get_latest_blockhash().unwrap();
        transaction.sign(&[&payer], blockhash);
        let result = self.rpc_client.send_transaction(&transaction); //send_and_confirm_transaction(&transaction);
        time.stop();
        match result {
            Ok(signature) => {
               
                //reload the account
                self.load_account_via_rpc(account);
                Some(signature)
            },
            Err(e) => {
                
                None
            }
        }
    }

}


///unit tests for RemoteAccountLoader
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_account_loader() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.get_account(&pubkey);
        assert_eq!(account.is_none(), true);
    }
    
    #[test]
    fn test_remote_account_loader2() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.has_account(&pubkey);
        assert_eq!(account, false);
    }

    #[test]
    fn test_remote_account_loader3() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.load_account(&pubkey);
        assert_eq!(account.is_none(), true);
    }

    #[test]
    fn test_remote_account_loader4() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(&pubkey);
        let account = loader.get_account(&pubkey);
        assert_eq!(account.is_none(), true);
    }
    
    #[test]
    fn test_remote_account_loader5() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(&pubkey);
        let account = loader.has_account(&pubkey);
        assert_eq!(account, false);
    }

    #[test]
    fn test_remote_account_loader6() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.load_account(&pubkey);
        assert_eq!(account.is_none(), true);
    }

}
