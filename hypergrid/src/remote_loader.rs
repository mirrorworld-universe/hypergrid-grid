use {
    crate::{config::Config, cosmos}, base64::{self, Engine}, core::fmt, dashmap::DashMap, log::*, serde_derive::{Deserialize, Serialize}, serde_json::json, sha2::{Digest, Sha256}, solana_client::rpc_client::RpcClient, solana_measure::measure::Measure, solana_sdk::{
        account::{AccountSharedData, ReadableAccount, WritableAccount}, account_utils::StateMut, bpf_loader_upgradeable::{self, UpgradeableLoaderState}, clock::Slot, commitment_config::CommitmentConfig, instruction::{AccountMeta, Instruction}, pubkey::Pubkey, signature::{Keypair, Signature, Signer}, signer::EncodableKey, transaction::Transaction
    }, std::{
        fs::File, io::Write, option_env, str::FromStr, sync::Arc, thread, time::Duration
    }, tokio, zstd
};


type AccountCacheKeyMap = DashMap<Pubkey, (AccountSharedData, Slot)>;


#[derive(Debug, Default)]
struct HypergridNode {
    pub pubkey: Pubkey,
    pub name: String,
    pub rpc: String,
    pub role: i32, // 0: unknown, 1: HSSN, 2: Sonic Grid, 3: Grid, 4: Solana L1
}

const NODE_TYPE_HSSN: i32 = 1;
const NODE_TYPE_SONIC: i32 = 2;
const NODE_TYPE_GRID: i32 = 3;
const NODE_TYPE_L1: i32 = 4;

pub struct RemoteAccountLoader {
    ///RPC client used to send requests to the remote.
    // rpc_client: RpcClient,
    cosmos_client: cosmos::HttpClient,
    /// Cache of accounts loaded from the remote.
    account_cache: AccountCacheKeyMap,
    /// Enable or disable the remote loader.
    enable: bool,
    config: Config,
    runtime: Option<tokio::runtime::Runtime>,
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
                error!("setting: {:?}", e);
            },
        };

        Self {
            // rpc_client: RpcClient::new_with_timeout_and_commitment(&config.baselayer_rpc_url, 
            // Duration::from_secs(30), CommitmentConfig::confirmed()),
            cosmos_client: cosmos::HttpClient::new(Duration::from_secs(30)),
            account_cache: AccountCacheKeyMap::default(),
            enable: true,
            config,
            runtime: Some(
                tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4).build().unwrap()
            ),
        }
    }

    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.as_ref().expect("runtime")
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
        // println!("RemoteAccountLoader.get_account: {:?}, {}", thread::current().id(), pubkey.to_string());
        match self.account_cache.get(pubkey) {
            Some(account) => {
                // println!("RemoteAccountLoader.get_account: {} match.", pubkey.to_string());
                return Some(account.0.clone());
            },
            None => None, // self.load_account(pubkey),
        }
    }

    /// Check if the account is in the cache.
    pub fn has_account(&self, pubkey: &Pubkey) -> bool {
        if !self.enable || Self::ignored_account(pubkey) {
            return false;
        }
        // println!("RemoteAccountLoader.has_account: {:?}, {}", thread::current().id(), pubkey.to_string());
        match self.account_cache.contains_key(pubkey) {
            true => true,
            false => false, //self.load_account(pubkey).is_some(),
        }
    }

    pub fn load_accounts(remote_loader: &Arc<Self>, slot: Slot, pubkeys: Vec<Pubkey>, source: Option<Pubkey>) {
        let loader = remote_loader.clone();
        remote_loader.runtime().spawn(async move {
            // println!("AccountsCache::load_accounts_from_remote, {:?}", pubkeys);
            pubkeys.iter().for_each(|pubkey| {
                //Sonic: load from remote
                loader.load_account(slot, pubkey, source);
            });
        });
    }

    pub fn deactivate_accounts(remote_loader: &Arc<Self>, slot: Slot, pubkeys: Vec<Pubkey>) {
        let loader = remote_loader.clone();
        remote_loader.runtime().spawn(async move {
            // println!("AccountsCache::deactivate_remote_accounts, {:?}", pubkeys);
            pubkeys.iter().for_each(|pubkey| {
                //Sonic: deactivate account in cache
                loader.deactivate_account(slot, &pubkey);
            });
        });
    }

    /// Load the account from the RPC.
    pub fn load_account(&self, slot: Slot, pubkey: &Pubkey, source: Option<Pubkey>) -> Option<AccountSharedData> {
        if !self.enable || Self::ignored_account(pubkey) {
            return None;
        }

        info!("Thread {:?}: load_account: {:?} from {:?}, solt: {:?}",  thread::current().id(), pubkey, source.unwrap_or_default(), slot);
        println!("Thread {:?}: load_account: {:?} from {:?}, solt: {:?}",  thread::current().id(), pubkey, source.unwrap_or_default(), slot);

        //load the account from the local file first
        let account = self.load_account_from_local_file(slot, pubkey, source);
        if account.is_some() {
            return account;
        }

        if let Some(account_cache) = self.account_cache.get(pubkey) {
            let (account1, slot1) = account_cache.clone();
            if slot == slot1  {
                info!("******* cache: {}\n", pubkey.to_string());
                return Some(account1);
            }
        }

        let account: Option<AccountSharedData>;
        match source {
            Some(source) => {
                account = self.load_account_via_hssn(pubkey, Some(source), slot);
            },
            None => {
                account = self.load_account_via_rpc(pubkey, None, slot);
            },
        }

        match account {
            Some(account) => {
                //Sonic: insert the account to the cache
                self.account_cache.insert(pubkey.clone(), (account.clone(), slot));

                //Sonic: save the account to the local file
                self.save_account_to_local_file(slot, pubkey, source, account.clone());

                //Sonic: check if programdata account exists
                if let Some(programdata_address) = Self::has_programdata_account(account.clone()) {
                    //Sonic: load programdata account from remote
                    self.load_account(slot, &programdata_address, source);
                }
                
                Some(account)
            },
            None => None,
        }
    }

    fn load_account_from_local_file(&self, slot: Slot, pubkey: &Pubkey, source: Option<Pubkey>) -> Option<AccountSharedData> {
        let path = format!("{}/{:?}_{:?}_{:?}.json", self.config.accounts_path, pubkey, source.unwrap_or_default(), slot);
        println!("load_account_from_local_file: {}\n", path);
        let file = File::open(path);
        match file {
            Ok(file) => {
                // read file content to json
                let account_data: serde_json::Value = serde_json::from_reader(file).unwrap();
                debug!("load_account_from_local_file: account_data: {:?}", account_data);
                let account = RemoteAccountLoader::deserialize_from_json2(account_data);
                account
            },
            Err(e) => {
                error!("load_account_from_local_file: failed to open file: {:?}\n", e);
                None
            }
        }
    }

    fn save_account_to_local_file(&self, slot: Slot, pubkey: &Pubkey, source: Option<Pubkey>, account: AccountSharedData) {
        let path = format!("{}/{:?}_{:?}_{:?}.json", self.config.accounts_path, pubkey, source.unwrap_or_default(), slot);

        //make sure the directory exists
        let dir = std::path::Path::new(&path).parent().unwrap();
        if !dir.exists() {
            std::fs::create_dir_all(dir).unwrap_or_default();
        }

        println!("save_account_to_local_file: {}\n", path);
        let file = File::create(path.clone());
        match file {
            Ok(mut file) => {
                let data = {
                    if account.data().len() < 1 {
                        "".to_string()
                    } else {
                        base64::engine::general_purpose::STANDARD.encode(account.data())
                    }
                };

                let account_data = json!({
                    "lamports": account.lamports(),
                    "data": [
                        data,
                        "base58"
                    ],
                    "owner": account.owner().to_string(),
                    "executable": account.executable(),
                    "rent_epoch": account.rent_epoch(),
                });
                let result = serde_json::to_writer_pretty(&mut file, &account_data);
                match result {
                    Ok(_) => {
                        info!("save_account_to_local_file: success: {}\n", path);
                    },
                    Err(e) => {
                        error!("save_account_to_local_file: failed to write file: {:?}\n", e);
                    }
                }
            },
            Err(e) => {
                error!("save_account_to_local_file: failed to create file: {:?}\n", e);
            }
        }
    }

    /// Load the account from the RPC.
    fn load_account_via_rpc(&self, pubkey: &Pubkey, source: Option<Pubkey>, slot: Slot) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            // print!("******* skip: {}\n", pubkey.to_string());
            return None;
        }

        let rpc_url = self.get_rpc_url_by_source(source, slot);
        if rpc_url.eq("") {
            return None;
        }

        println!("Thread {:?}: load_account_via_rpc: {:?} at slot {:?} from {:?}",  thread::current().id(), pubkey, slot, rpc_url.clone());
        info!("Thread {:?}: load_account_via_rpc: {:?} at slot {:?} from {:?}",  thread::current().id(), pubkey, slot, rpc_url.clone());

        let rpc_client = RpcClient::new_with_timeout_and_commitment(rpc_url, Duration::from_secs(30), CommitmentConfig::confirmed());

        let mut time = Measure::start("load_account_from_remote");
        let result = rpc_client.get_account(pubkey);
        match result {
            Ok(account) => {
                // println!("load_account_via_rpc1: account: {:?}", account);
                let mut account = AccountSharedData::create(
                    account.lamports,
                    account.data,
                    account.owner,
                    account.executable,
                    account.rent_epoch
                );
                account.remote = true;
        
                time.stop();
                // println!("load_account_via_rpc: account: {:?}, {:?}", account, time.as_us());
                Some(account)
            },
            Err(e) => {
                error!("load_account_via_rpc: failed to load account: {:?}\n", e);
                None
            }
        }
    }

    fn get_rpc_url_by_source(&self, source: Option<Pubkey>, slot: Slot) -> String {
        if let Some(source) = source {
            let path = format!("{}/hypergrid_{:?}_{:?}.json", self.config.accounts_path, source, slot);
            println!("load hypergrid node from file: {}\n", path);
            let file = File::open(path);
            match file {
                Ok(file) => {
                    // read file content to json
                    let _data: serde_json::Value = serde_json::from_reader(file).unwrap();
                    debug!("load_account_from_local_file: account_data: {:?}", _data);
                    let node = _data.get("hypergridNode").unwrap();
                    let node_id = node["pubkey"].as_str().unwrap();
                    let node_name = node["name"].as_str().unwrap();
                    let node_url = node["rpc"].as_str().unwrap();
                    let node_role = node["role"].as_i64().unwrap();
                    // println!("node: {}, {}", node_id, node_url);

                    if node_role == 2 || node_role == 3 || node_role == 4 {
                        return node_url.to_string();
                    } else {
                        info!("load hypergrid node from file: invalid source role: {:?}, {:?}, {:?}", node_name, node_id, node_role);
                        return "".to_string();
                    }
                },
                Err(e) => {
                    info!("load hypergrid node from file: failed to open file: {:?}\n", e);
                }
            }

            let node = Self::load_hypergrid_node(self.config.clone(), source, slot);
            if let Some(node) = node {
                //Only call rpc of nodes (2: Sonic Grid, 3: Grid, 4: Solana L1)
                if node.role == NODE_TYPE_SONIC || node.role == NODE_TYPE_GRID || node.role == NODE_TYPE_L1 {
                    return node.rpc;
                } else {
                    info!("load_account_via_rpc: invalid source role: {:?}, {:?}, {:?}", node.name, node.pubkey, node.role);
                }
            }
            return "".to_string();
        } else {
            self.config.baselayer_rpc_url.clone()
        }
    }

    fn deserialize_from_json(account_data: serde_json::Value) -> Option<AccountSharedData> {
        let result = &account_data["solanaAccount"];
        if result.is_null() {
            return None;
        }

        let value = &result["value"];
        if value.is_null() {
            return None;
        }
        let value_str = value.as_str().unwrap_or("");
        let value: serde_json::Result<serde_json::Value> = serde_json::from_str(value_str);
        if let Ok(value) = value {
            let account = RemoteAccountLoader::deserialize_from_json2(value);
            info!("deserialize_from_json account: {:?}", account);
            account
        } else {
            None
        }
    }

    fn deserialize_from_json2(value: serde_json::Value) -> Option<AccountSharedData> {
        let owner = value["owner"].as_str().unwrap_or("");
        if owner.eq("") {
            return None;
        } 
        let data = value["data"][0].as_str().unwrap_or("");
        let encoding = value["data"][1].as_str().unwrap_or("");
        let lamports = value["lamports"].as_u64().unwrap_or(0);
        let rent_epoch = value["rentEpoch"].as_u64().unwrap_or(0);
        // let space = value["space"].as_u64().unwrap();
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

        info!("deserialize_from_json account: {:?}", account);
        Some(account)
    }

    fn load_hypergrid_node(config: Config, source: Pubkey, slot: Slot) -> Option<HypergridNode> {
        let url = format!("{}/hypergrid-ssn/hypergridssn/hypergrid_node/{}", config.hssn_rpc_url, source.to_string());
        info!("load_hypergrid_nodes: {}\n", url);
        let client = cosmos::HttpClient::new(Duration::from_secs(30));
        let res = client.call(url.clone());
        if let Ok(body) = res {
            //sace the response to local file
            let path = format!("{}/hypergrid_{:?}_{:?}.json", config.accounts_path, source, slot);
            let dir = std::path::Path::new(&path).parent().unwrap();
            if !dir.exists() {
                std::fs::create_dir_all(dir).unwrap_or_default();
            }

            println!("save hypergrid node to local file: {}\n", path);
            let file = File::create(path.clone());
            match file {
                Ok(mut file) => {
                    let result = file.write_all(body.as_bytes());
                    match result {
                        Ok(_) => {
                            info!("save hypergrid node to local file: success: {}\n", path);
                        },
                        Err(e) => {
                            warn!("save hypergrid node to local file: failed to write file: {:?}\n", e);
                        }
                    }
                },
                Err(e) => {
                    warn!("save hypergrid node to local file: failed to create file: {:?}\n", e);
                }
            }

            //convert the response body to json
            let value: serde_json::Result<serde_json::Value> = serde_json::from_str(&body);
            if let Ok(value) = value {
                // let value: serde_json::Value = value.unwrap();
                
                let node = value.get("hypergridNode").unwrap();
                // println!("load_hypergrid_node: success: {:?}\n", node);
                let node_id = node["pubkey"].as_str().unwrap();
                let node_name = node["name"].as_str().unwrap();
                let node_url = node["rpc"].as_str().unwrap();
                let node_role = node["role"].as_i64().unwrap();
                let node = HypergridNode {
                    pubkey: Pubkey::from_str(node_id).unwrap(),
                    name: node_name.to_string(),
                    rpc: node_url.to_string(),
                    role: node_role as i32,
                };

                return Some(node);
            }
        }
        warn!("get_hypergrid_nodes: not found: {:?}\n", url.clone());
        None
    }

    // fn load_hypergrid_nodes(&self) {
    //     let config = self.config.clone();
    //     let hypergrid_nodes = self.hypergrid_nodes.clone();
    //     thread::Builder::new()
    //         .name("load_hypergrid_nodes".to_string())
    //         .spawn(move || {
    //             Self::do_load_hypergrid_nodes(config, hypergrid_nodes);
    //         })
    //         .unwrap();
    // }

    fn load_account_via_hssn(&self, pubkey: &Pubkey, source: Option<Pubkey>, slot: Slot) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            // print!("******* skip: {}\n", pubkey.to_string());
            return None;
        }
        info!("Thread {:?}: load_account_via_hssn: {:?}",  thread::current().id(), pubkey.to_string());
        println!("Thread {:?}: load_account_via_hssn: {:?}",  thread::current().id(), pubkey.to_string());

        let url = format!("{:?}/hypergrid-ssn/hypergridssn/solana_account/{:?}/{:?}_{:?}",self.config.hssn_rpc_url, pubkey, source.unwrap_or_default(), slot);
        info!("load_account_from_hssn: {}\n", url);
        let res = self.cosmos_client.call(url);
        let mut account: Option<AccountSharedData> = None;
        match res {
            Ok(body) => {
                info!("respone: {:?}", body);
                //convert the response body to json
                let value: serde_json::Result<serde_json::Value> = serde_json::from_str(&body);
                if let Ok(value) = value {
                    // let value: serde_json::Value = value.unwrap();
                    info!("load_account_via_hssn: success: {:?}\n", value);
                    account = RemoteAccountLoader::deserialize_from_json(value);
                } 
            },
            Err(e) => {
                warn!("load_account_from_hssn: not found: {:?}, {:?}\n", pubkey, e);
            }
        }

        match account {
            Some(account) => {
                Some(account)
            },
            None => {
                info!("load_account_from_hssn: not found: {:?}\n", pubkey);
                let account = self.load_account_via_rpc(pubkey, source, slot);
                if let Some(account) = account {
                    //load the account from the source
                    let version = format!("{:?}_{:?}", source.unwrap_or_default(), slot);
                    if let Some(source) = source {
                        cosmos::run_load_solana_account(pubkey.to_string().as_str(), version.as_str(), source.to_string().as_str(), false);
                    }
                    Some(account)
                } else {
                    None
                }
            }
        }
    }

    /// Check if the account has a programdata account.
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
    pub fn deactivate_account(&self, slot: Slot, pubkey: &Pubkey) {
        if !self.enable || Self::ignored_account(pubkey) {
            return;
        }
        println!("RemoteAccountLoader.deactivate_account: {}, {}", pubkey.to_string(), slot);
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
        let account = loader.load_account(0, &pubkey, None);
        assert_eq!(account.is_none(), true);
    }

    #[test]
    fn test_remote_account_loader4() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(0, &pubkey);
        let account = loader.get_account(&pubkey);
        assert_eq!(account.is_none(), true);
    }
    
    #[test]
    fn test_remote_account_loader5() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(0, &pubkey);
        let account = loader.has_account(&pubkey);
        assert_eq!(account, false);
    }

    #[test]
    fn test_remote_account_loader6() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.load_account(0, &pubkey, None);
        assert_eq!(account.is_none(), true);
    }

}
