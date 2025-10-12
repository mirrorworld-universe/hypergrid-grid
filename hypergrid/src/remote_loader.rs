use {
    crate::{config::Config, cosmos},
    base64::{self, Engine},
    log::*,
    serde::{Deserialize, Serialize},
    serde_json::json,
    solana_client::rpc_client::RpcClient,
    solana_measure::measure::Measure,
    solana_sdk::{
        account::{AccountSharedData, ReadableAccount, WritableAccount},
        account_utils::StateMut,
        bpf_loader_upgradeable::{self, UpgradeableLoaderState},
        clock::Slot,
        commitment_config::CommitmentConfig,
        genesis_config::ClusterType,
        hash::Hash,
        pubkey::Pubkey,
    },
    std::{collections::HashSet, env, fs::File, sync::RwLock, thread, time::Duration},
    thiserror::Error,
};

type AccountCacheKeyMap = dashmap::DashMap<Pubkey, (AccountSharedData, Slot)>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HypergridNode {
    pub pubkey: Pubkey,
    pub name: String,
    pub rpc: String,
    pub role: NodeType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HypergridNodeFileContent {
    hypergrid_node: HypergridNode,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
enum NodeType {
    HSSN = 1,
    Sonic = 2,
    Grid = 3,
    L1 = 4,
}

#[derive(Debug, Error)]
enum NodeTypeErr {
    #[error("Node is unknown (node role is 0)")]
    UnknownNode,
    #[error("Invalid node type {0}")]
    InvalidNode(i64),
}

impl TryFrom<i64> for NodeType {
    type Error = NodeTypeErr;
    fn try_from(n: i64) -> Result<Self, Self::Error> {
        match n {
            0 => Err(Self::Error::UnknownNode),
            1 => Ok(Self::HSSN),
            2 => Ok(Self::Sonic),
            3 => Ok(Self::Grid),
            4 => Ok(Self::L1),
            n => Err(Self::Error::InvalidNode(n)),
        }
    }
}

impl From<NodeType> for i64 {
    fn from(nt: NodeType) -> Self {
        nt as Self
    }
}

#[derive(Debug)]
pub struct RemoteAccountLoader {
    ///RPC client used to send requests to the remote.
    // rpc_client: RpcClient,
    http_client: reqwest::blocking::Client,
    /// Cache of accounts loaded from the remote.
    account_cache: AccountCacheKeyMap,
    genesis_hash: RwLock<Hash>,
    config: Config,
}

impl Default for RemoteAccountLoader {
    fn default() -> Self {
        let cluster_type =
            env::var("SOLANA_RUN_SH_CLUSTER_TYPE").unwrap_or(ClusterType::STRINGS[0].to_string());
        let cluster_type = cluster_type.parse().unwrap_or(ClusterType::Development);
        let default_config_path = {
            //get current directory
            let mut default_config_path = std::env::current_dir().expect("current directory");
            // let mut default_config_path = dirs_next::home_dir().expect("home directory");
            default_config_path.extend(["hypergrid", "config.yml"]);
            default_config_path.to_str().unwrap().to_string()
        };
        let config_path = env::var("SONIC_CONFIG_FILE").unwrap_or(default_config_path);
        Self::new(&config_path, cluster_type, Hash::default())
    }
}

/// Remote account loader.
impl RemoteAccountLoader {
    /// Create a new remote loader.
    pub fn new(config_path: &str, cluster_type: ClusterType, genesis_hash: Hash) -> Self {
        let mut config = Config::new(cluster_type);
        match Config::load(config_path) {
            Ok(setting) => {
                config = setting;

                // let key = Keypair::from_base58_string(&setting.keypair_base58);
                // let program_id = Pubkey::from_str(&setting.sonic_program_id).unwrap();
                // println!("setting: {:?}, {:?}, {:?}", &setting.baselayer_rpc_url, key, program_id)
            }
            Err(e) => {
                error!("setting: {:?}", e);
            }
        };

        Self {
            // rpc_client: RpcClient::new_with_timeout_and_commitment(&config.baselayer_rpc_url,
            // Duration::from_secs(30), CommitmentConfig::confirmed()),
            http_client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            account_cache: AccountCacheKeyMap::default(),
            genesis_hash: RwLock::new(genesis_hash),
            config,
        }
    }

    pub fn set_genesis_hash(&self, hash: Hash) {
        *self.genesis_hash.write().unwrap() = hash;
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
        if Self::ignored_account(pubkey) {
            return None;
        }

        // println!("RemoteAccountLoader.get_account: {:?}, {}", thread::current().id(), pubkey.to_string());
        self.account_cache.get(pubkey).map(|ent| ent.0.clone())
    }

    /// Check if the account is in the cache.
    pub fn has_account(&self, pubkey: &Pubkey) -> bool {
        !Self::ignored_account(pubkey) && self.account_cache.contains_key(pubkey)
    }

    pub fn get_account_list(&self) -> HashSet<Pubkey> {
        self.account_cache.iter().map(|ent| *ent.key()).collect()
    }

    pub fn get_historical_accounts(&self) -> HashSet<(Pubkey, Slot)> {
        std::fs::read_dir(&self.config.accounts_path)
            .unwrap()
            .map(|ent| ent.unwrap().path())
            .filter_map(|path| {
                let file_name = path.file_name()?.to_string_lossy();
                if path.extension()? != "json" || file_name.starts_with("hypergrid_") {
                    return None;
                }
                Some(file_name.to_string())
            })
            .filter_map(|file_name| {
                let file_name = file_name.strip_suffix(".json")?;
                let [pubkey, _source, _genesis_hash, slot] =
                    *file_name.split('_').collect::<Vec<_>>()
                else {
                    return None;
                };
                Some((pubkey.parse().unwrap(), slot.parse().unwrap()))
            })
            .collect()
    }

    pub fn load_accounts(&self, slot: Slot, pubkeys: Vec<Pubkey>, source: Option<Pubkey>) {
        info!(
            "Sonic AccountsCache::load_accounts_from_remote, {:?}",
            pubkeys
        );
        pubkeys.iter().for_each(|pubkey| {
            //Sonic: load from remote
            self.load_account(slot, pubkey, source);
        });
    }

    pub fn deactivate_accounts(&self, slot: Slot, pubkeys: Vec<Pubkey>) {
        info!(
            "Sonic AccountsCache::deactivate_remote_accounts, {:?}",
            pubkeys
        );
        pubkeys.iter().for_each(|pubkey| {
            self.deactivate_account(slot, pubkey);
        });
    }

    /// Load the account from the RPC.
    pub fn load_account(
        &self,
        slot: Slot,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
    ) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            return None;
        }

        info!(
            "Sonic Thread {:?}: load_account: {:?} from {:?}, solt: {:?}",
            thread::current().id(),
            pubkey,
            source.unwrap_or_default(),
            slot
        );

        // println!("Thread {:?}: load_account: {:?} from {:?}, solt: {:?}",  thread::current().id(), pubkey, source.unwrap_or_default(), slot);

        //load the account from the local file first
        let account = self.load_account_from_local_file(slot, pubkey, source);
        if let Some(account) = account {
            //Sonic: insert the account to the cache
            self.account_cache.insert(*pubkey, (account.clone(), slot));

            //Sonic: check if programdata account exists
            if let Some(programdata_address) = Self::has_programdata_account(&account) {
                //Sonic: load programdata account from remote
                self.load_account(slot, &programdata_address, source);
            }
            return Some(account);
        }

        if let Some(entry) = self.account_cache.get(pubkey) {
            let (cached_account, cached_slot) = &*entry;
            if slot == *cached_slot {
                info!("Sonic cache: {pubkey}\n");
                return Some(cached_account.clone());
            }
        }

        let account = if let Some(source) = source {
            self.load_account_via_hssn(pubkey, Some(source), slot)
        } else {
            self.load_account_via_oracle(pubkey, None, slot)
        }?;

        //Sonic: insert the account to the cache
        self.account_cache.insert(*pubkey, (account.clone(), slot));

        //Sonic: save the account to the local file
        self.save_account_to_local_file(slot, pubkey, source, account.clone());

        //Sonic: check if programdata account exists
        if let Some(programdata_address) = Self::has_programdata_account(&account) {
            //Sonic: load programdata account from remote
            self.load_account(slot, &programdata_address, source);
        }

        Some(account)
    }

    fn load_account_from_local_file(
        &self,
        slot: Slot,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
    ) -> Option<AccountSharedData> {
        let path = format!(
            "{}/{:?}_{:?}_{}_{:?}.json",
            self.config.accounts_path,
            pubkey,
            source.unwrap_or_default(),
            self.genesis_hash.read().unwrap(),
            slot
        );
        info!("Sonic load_account_from_local_file: {}\n", path);
        let file = File::open(path);
        match file {
            Ok(file) => {
                // read file content to json
                let mut account = serde_json::from_reader::<_, AccountSharedData>(file).unwrap();
                account.remote = true;
                debug!("Sonic load_account_from_local_file: account: {account:?}");

                Some(account)
            }
            Err(e) => {
                error!(
                    "Sonic load_account_from_local_file: failed to open file: {:?}\n",
                    e
                );
                None
            }
        }
    }

    fn save_account_to_local_file(
        &self,
        slot: Slot,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
        account: AccountSharedData,
    ) {
        let path = format!(
            "{}/{:?}_{:?}_{}_{:?}.json",
            self.config.accounts_path,
            pubkey,
            source.unwrap_or_default(),
            self.genesis_hash.read().unwrap(),
            slot
        );
        //make sure the directory exists
        let dir = std::path::Path::new(&path).parent().unwrap();
        if !dir.exists() {
            std::fs::create_dir_all(dir).unwrap_or_default();
        }

        info!("Sonic save_account_to_local_file: {}\n", path);
        let file = File::create(path.clone());
        match file {
            Ok(mut file) => {
                let data = {
                    if account.data().is_empty() {
                        "".to_string()
                    } else {
                        base64::engine::general_purpose::STANDARD.encode(account.data())
                    }
                };

                let account_data = json!({
                    "lamports": account.lamports(),
                    "data": [
                        data,
                        "base64"
                    ],
                    "owner": account.owner().to_string(),
                    "executable": account.executable(),
                    "rentEpoch": account.rent_epoch(),
                });
                let result = serde_json::to_writer_pretty(&mut file, &account_data);
                match result {
                    Ok(_) => {
                        info!("Sonic save_account_to_local_file: success: {}\n", path);
                    }
                    Err(e) => {
                        error!(
                            "Sonic save_account_to_local_file: failed to write file: {:?}\n",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    "Sonic save_account_to_local_file: failed to create file: {:?}\n",
                    e
                );
            }
        }
    }

    /// Load the account from the RPC.
    fn load_account_via_oracle(
        &self,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
        slot: Slot,
    ) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            // print!("******* skip: {}\n", pubkey.to_string());
            return None;
        }

        let rpc_url = self.get_rpc_url_by_source(source, slot);
        if rpc_url.is_empty() {
            return None;
        }

        // println!("Thread {:?}: load_account_via_oracle: {:?} at {} slot {:?} from {:?}",  thread::current().id(), pubkey, genesis_hash, slot, rpc_url.clone());
        info!(
            "Sonic Thread {:?}: load_account_via_oracle: {:?} at {} slot {:?} from {:?}",
            thread::current().id(),
            pubkey,
            self.genesis_hash.read().unwrap(),
            slot,
            rpc_url,
        );

        let url = format!("{}/solana/GetAccountInfo", self.config.oracle_url);
        let data = json!({
            "rpc": rpc_url,
            "address": pubkey.to_string(),
            "version": format!("{:?}-{}-{}", source.unwrap_or_default(), self.genesis_hash.read().unwrap(), slot),
        });
        info!("Sonic load_account_from_oracle: {}\n", url);
        let resp = match self.http_client.post(&url).json(&data).send() {
            Ok(resp) => resp,
            Err(err) => {
                warn!(
                    "Sonic load_account_from_oracle: not found in url {url}: {pubkey}, {err:?}\n"
                );
                return None;
            }
        };
        let mut account = match resp.json::<AccountSharedData>() {
            Ok(a) => a,
            Err(err) => {
                warn!("Invalid account from oracle {url}: {err}");
                return None;
            }
        };
        // XXX: it's set by default to false by serde
        account.remote = true;
        info!("Sonic load_account_via_hssn: success: {account:?}\n");
        Some(account)
    }

    /// Load the account from the RPC.
    #[allow(dead_code)]
    fn load_account_via_rpc(
        &self,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
        slot: Slot,
    ) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            // print!("******* skip: {}\n", pubkey.to_string());
            return None;
        }

        let rpc_url = self.get_rpc_url_by_source(source, slot);
        if rpc_url.eq("") {
            return None;
        }

        // println!("Thread {:?}: load_account_via_rpc: {:?} at {} slot {:?} from {:?}",  thread::current().id(), pubkey, genesis_hash, slot, rpc_url.clone());
        info!(
            "Sonic Thread {:?}: load_account_via_rpc: {:?} at {} slot {:?} from {:?}",
            thread::current().id(),
            pubkey,
            self.genesis_hash.read().unwrap(),
            slot,
            rpc_url,
        );

        let rpc_client = RpcClient::new_with_timeout_and_commitment(
            rpc_url,
            Duration::from_secs(30),
            CommitmentConfig::confirmed(),
        );

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
                    account.rent_epoch,
                );
                account.remote = true;

                time.stop();
                // println!("load_account_via_rpc: account: {:?}, {:?}", account, time.as_us());
                Some(account)
            }
            Err(e) => {
                error!(
                    "Sonic load_account_via_rpc: failed to load account: {:?}\n",
                    e
                );
                None
            }
        }
    }

    fn get_rpc_url_by_source(&self, source: Option<Pubkey>, slot: Slot) -> String {
        let Some(source) = source else {
            return self.config.baselayer_rpc_url.clone();
        };
        let path = format!(
            "{}/hypergrid_{:?}_{}_{:?}.json",
            self.config.accounts_path,
            source,
            self.genesis_hash.read().unwrap(),
            slot
        );
        info!("Sonic load hypergrid node from file: {}\n", path);
        let file = File::open(path);
        match file {
            Ok(file) => {
                // read file content to json
                let HypergridNodeFileContent { hypergrid_node } =
                    serde_json::from_reader(file).unwrap();
                debug!("Sonic load hypergrid node from file: {hypergrid_node:?}");

                return hypergrid_node.rpc.to_string();
            }
            Err(e) => {
                info!(
                    "Sonic load hypergrid node from file: failed to open file: {:?}\n",
                    e
                );
            }
        }

        self.load_hypergrid_node(source, slot)
            .map(|node| node.rpc)
            .unwrap_or_default()
    }

    fn load_hypergrid_node(&self, source: Pubkey, slot: Slot) -> Option<HypergridNode> {
        // let url = format!("{}/hypergrid-ssn/hypergridssn/hypergrid_node/{}", config.hssn_rpc_url, source.to_string());
        let url = format!("{}/hssn/HypergridNode", self.config.oracle_url);
        let data = json!({
            "rpc": self.config.hssn_rpc_url,
            "address": source.to_string(),
            "version": format!("{}-{}", self.genesis_hash.read().unwrap(), slot),
        });
        info!("Sonic load_hypergrid_nodes: {}, {:?}\n", url, data);
        // println!("load_hypergrid_nodes: {}, {:?}\n", url, data);
        let resp = match self.http_client.post(&url).json(&data).send() {
            Ok(body) => body,
            Err(err) => {
                info!("Sonic get_hypergrid_nodes: not found for {url}. Err: {err:?}\n");
                return None;
            }
        };

        //convert the response body to json
        let node = match resp.json() {
            Ok(HypergridNodeFileContent { hypergrid_node }) => {
                info!("Sonic load_hypergrid_node: success: {hypergrid_node:?}\n");
                hypergrid_node
            }
            Err(err) => {
                error!("Invalid hypergrid nodes from url {url}. Err: {err:?}");
                return None;
            }
        };

        //sace the response to local file
        let path = format!(
            "{}/hypergrid_{:?}_{}_{:?}.json",
            self.config.accounts_path,
            source,
            self.genesis_hash.read().unwrap(),
            slot
        );
        let dir = std::path::Path::new(&path).parent().unwrap();
        if !dir.exists() {
            std::fs::create_dir_all(dir).unwrap_or_default();
        }

        info!("Sonic save hypergrid node to local file: {}\n", path);
        if let Err(err) = std::fs::write(&path, serde_json::to_vec(&node).expect("Always valid")) {
            warn!("Failed to save to hypergrid nodes to file {path}: {err}");
        }

        Some(node)
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

    fn load_account_via_hssn(
        &self,
        pubkey: &Pubkey,
        source: Option<Pubkey>,
        slot: Slot,
    ) -> Option<AccountSharedData> {
        if Self::ignored_account(pubkey) {
            // print!("******* skip: {}\n", pubkey.to_string());
            return None;
        }
        info!(
            "Sonic Thread {:?}: load_account_via_hssn: {:?}",
            thread::current().id(),
            pubkey
        );
        // println!("Thread {:?}: load_account_via_hssn: {:?}",  thread::current().id(), pubkey);

        let url = format!(
            "{:?}/hypergrid-ssn/hypergridssn/solana_account/{:?}/{:?}-{}-{:?}",
            self.config.hssn_rpc_url,
            pubkey,
            source.unwrap_or_default(),
            self.genesis_hash.read().unwrap(),
            slot
        );
        info!("Sonic load_account_from_hssn: {}\n", url);
        match self.http_client.get(url).send() {
            Ok(resp) => {
                info!("Sonic respone: {resp:?}");
                //convert the response body to json
                if let Ok(mut value) = resp.json::<AccountSharedData>() {
                    info!("Sonic load_account_via_hssn: success: {:?}\n", value);
                    value.remote = true;
                    return Some(value);
                }
            }
            Err(e) => {
                warn!(
                    "Sonic load_account_from_hssn: not found: {:?}, {:?}\n",
                    pubkey, e
                );
            }
        }

        info!("Sonic load_account_from_hssn: not found: {:?}\n", pubkey);
        let account = self.load_account_via_oracle(pubkey, source, slot)?;

        if let Some(source) = source {
            // load the account from the source
            let version = format!("{source}_{}_{slot}", self.genesis_hash.read().unwrap());

            cosmos::run_load_solana_account(pubkey, &version, &source, false);
        }

        Some(account)
    }

    /// Check if the account has a programdata account.
    pub fn has_programdata_account(program_account: &AccountSharedData) -> Option<Pubkey> {
        if program_account.executable()
            && !bpf_loader_upgradeable::check_id(program_account.owner())
        {
            return None;
        }

        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = program_account.state()
        {
            return Some(programdata_address);
        }

        None
    }

    /// Deactivate the account in the cache.
    pub fn deactivate_account(&self, slot: Slot, pubkey: &Pubkey) {
        if Self::ignored_account(pubkey) {
            return;
        }
        info!(
            "Sonic RemoteAccountLoader.deactivate_account: {}, {}",
            pubkey, slot
        );

        let Some(account) = self.get_account(pubkey) else {
            return;
        };
        self.account_cache.remove(pubkey);
        //remove the related programdata account
        if let Some(programdata_address) = Self::has_programdata_account(&account) {
            self.account_cache.remove(&programdata_address);
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
        assert!(account.is_none());
    }

    #[test]
    fn test_remote_account_loader2() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.has_account(&pubkey);
        assert!(!account);
    }

    #[test]
    fn test_remote_account_loader3() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.load_account("", 0, &pubkey, None);
        assert!(account.is_none());
    }

    #[test]
    fn test_remote_account_loader4() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(0, &pubkey);
        let account = loader.get_account(&pubkey);
        assert!(account.is_none());
    }

    #[test]
    fn test_remote_account_loader5() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        loader.deactivate_account(0, &pubkey);
        let account = loader.has_account(&pubkey);
        assert!(!account);
    }

    #[test]
    fn test_remote_account_loader6() {
        let loader = RemoteAccountLoader::default();
        let pubkey = Pubkey::from_str("4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z").unwrap();
        let account = loader.load_account("", 0, &pubkey, None);
        assert!(account.is_none());
    }
}
