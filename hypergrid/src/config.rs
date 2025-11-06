use {
    serde::{Deserialize, Serialize},
    solana_genesis_config::ClusterType,
    std::{fs::File, io, path::Path},
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Config {
    pub baselayer_rpc_url: String,
    pub hssn_rpc_url: String,
    pub keypair_file: String,
    // pub sonic_program_id: String,
    pub accounts_path: String,
    pub oracle_url: String,
}

// impl Default for Config {
//     fn default() -> Self {
//         let keypair_file = "~/.config/solana/id.json".to_string();
//         let baselayer_rpc_url = "https://api.testnet.solana.com".to_string();
//         // let sonic_program_id ="4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z".to_string();
//         let hssn_rpc_url: String = "https://exapi.testnet.hssn.sonic.game".to_string();
//         let accounts_path: String = "hypergrid/accounts".to_string();
//         let oracle_url: String = "https://nisaba-hssn.sonic.game".to_string();

//         Self {
//             baselayer_rpc_url,
//             hssn_rpc_url,
//             keypair_file,
//             // sonic_program_id,
//             accounts_path,
//             oracle_url,
//         }
//     }
// }

impl Config {
    pub fn new(cluster_type: ClusterType) -> Self {
        let mut baselayer_rpc_url = "https://api.testnet.solana.com".to_string();
        // let sonic_program_id ="4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z".to_string();
        let mut hssn_rpc_url: String = "https://exapi.testnet.hssn.sonic.game".to_string();

        match cluster_type {
            ClusterType::Development => {}
            ClusterType::Devnet => {
                baselayer_rpc_url = "https://api.devnet.solana.com".to_string();
                hssn_rpc_url = "https://exapi.devnet.hssn.sonic.game".to_string();
            }
            ClusterType::Testnet => {
                baselayer_rpc_url = "https://api.testnet.solana.com".to_string();
                hssn_rpc_url = "https://exapi.testnet.hssn.sonic.game".to_string();
            }
            ClusterType::MainnetBeta => {
                baselayer_rpc_url = "https://api.mainnet-beta.solana.com".to_string();
                hssn_rpc_url = "https://exapi.mainnet.hssn.sonic.game".to_string();
            }
        }

        let keypair_file = "~/.config/solana/id.json".to_string();
        let accounts_path: String = "hypergrid/accounts".to_string();
        let oracle_url: String = "https://nisaba-hssn.sonic.game".to_string();

        Self {
            baselayer_rpc_url,
            hssn_rpc_url,
            keypair_file,
            // sonic_program_id,
            accounts_path,
            oracle_url,
        }
    }
    /// Load a configuration from file.
    ///
    /// # Errors
    ///
    /// This function may return typical file I/O errors.
    pub fn load(config_file: &str) -> Result<Self, io::Error> {
        load_config_file(config_file)
    }
}
