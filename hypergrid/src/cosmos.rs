use {log::*, std::process::Command};

const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = ".hypergrid-ssn";
const COSMOS_APP: &str = "bin/hypergrid-ssnd";
const COSMOS_SIGNER: &str = "my_key";

pub fn run_load_solana_account(pub_key: &str, version: &str, source: &str, update: bool) {
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
        // println!("{} does not exist.", cosmos_app_path);
        return;
    }

    //format the command string
    let mut cmd = Command::new(cosmos_app_path);
    cmd.arg("tx")
        .arg("hypergridssn")
        .args(&if update {
            vec!["update-solana-account", pub_key, version]
        } else {
            vec!["create-solana-account", pub_key, version, source]
        })
        .args(["--home", &cosmos_home_path])
        .args(["--from", COSMOS_SIGNER])
        .args(["--chain-id", COSMOS_CHAIN_ID])
        .args(["--gas", "50000000"])
        .args(["--keyring-backend", "test"])
        .arg("-y");

    info!("cmd_str: {cmd:?}");

    match cmd.output() {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            info!("output: {:?}", output_str);
            // println!("{:?}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            // TODO: print stderr as well
            error!("Error: {:?}", e);
        }
    }
}
