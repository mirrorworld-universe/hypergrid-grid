use {
    std::process::Command,
    log::*,
};

const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = ".hypergrid-ssn";
const COSMOS_APP: &str = "bin/hypergrid-ssnd";
const COSMOS_SIGNER: &str = "my_key";

pub fn run_load_solana_account(pub_key: &str, version:  &str, source: &str, update: bool) {
    let home_path = dirs_next::home_dir().expect("home directory");
    
    let cosmos_home_path = {
        let mut _path = home_path.clone();
        _path.extend([COSMOS_HOME]);
        _path.to_str().unwrap().to_string()
    };

    let cosmos_app_path = {
        let mut _path = home_path.clone();
        _path.extend([COSMOS_HOME, COSMOS_APP]);
        _path.to_str().unwrap().to_string()
    };

    let app_path = std::path::Path::new(&cosmos_app_path);
    if !app_path.exists() {
        warn!("{} does not exist.", cosmos_app_path);
        println!("{} does not exist.", cosmos_app_path);
        return;
    }
    
    //format the command string
    let cmd_str: String;
    if update {
        cmd_str = format!("{} tx hypergridssn update-solana-account {} {} --home {} --from {} --chain-id {} --gas 50000000 --keyring-backend test -y", 
        cosmos_app_path, pub_key, version, cosmos_home_path, COSMOS_SIGNER, COSMOS_CHAIN_ID);
    } else {
        cmd_str = format!("{} tx hypergridssn create-solana-account {} {} {} --home {} --from {} --chain-id {} --gas 50000000 --keyring-backend test -y", 
        cosmos_app_path, pub_key, version, source, cosmos_home_path, COSMOS_SIGNER, COSMOS_CHAIN_ID);
    }

    println!("cmd_str: {}", cmd_str);
    info!("cmd_str: {}", cmd_str);
    
    let output = Command::new("sh").arg("-c").arg(cmd_str).output();
    match output {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            info!("output: {:?}", output_str);
            // println!("{:?}", String::from_utf8_lossy(&output.stdout));
        },
        Err(e) => {
            error!("Error: {:?}", e);
        }
    }
}
