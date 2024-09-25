use {
    std::{
        fs::File, io,
        path::Path,
    },
    serde_derive::{Deserialize, Serialize},
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
    pub hssn_rpc_url: String,
    pub keypair_file: String,
    // pub sonic_program_id: String,
}

impl Default for Config {
    fn default() -> Self {
        let keypair_file = "~/.config/solana/id.json".to_string();
        let baselayer_rpc_url = "https://api.devnet.solana.com".to_string();
        // let sonic_program_id ="4WTUyXNcf6QCEj76b3aRDLPewkPGkXFZkkyf3A3vua1z".to_string();
        let hssn_rpc_url: String = "http://localhost:1317".to_string();

        Self {
            baselayer_rpc_url,
            hssn_rpc_url,
            keypair_file,
            // sonic_program_id,
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