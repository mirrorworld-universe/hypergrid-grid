use {log::*, solana_sdk::pubkey::Pubkey, std::process::Command};

const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = ".hypergrid-ssn";
const COSMOS_APP: &str = "bin/hypergrid-ssnd";
const COSMOS_SIGNER: &str = "my_key";

pub fn run_load_solana_account(pub_key: &Pubkey, version: &str, source: &Pubkey, update: bool) {
    let home_path = dirs_next::home_dir().expect("home directory");
    let (pub_key, source) = (pub_key.to_string(), source.to_string());

    let cosmos_app_path = home_path.join(COSMOS_HOME).join(COSMOS_APP);

    let app_path = std::path::Path::new(&cosmos_app_path);
    if !app_path.exists() {
        warn!("{cosmos_app_path:?} does not exist.");
        return;
    }

    //format the command string
    let mut cmd = Command::new(cosmos_app_path);
    cmd.arg("tx")
        .arg("hypergridssn")
        .args(&if update {
            vec!["update-solana-account", &pub_key, version]
        } else {
            vec!["create-solana-account", &pub_key, version, &source]
        })
        .args([
            AsRef::<std::ffi::OsStr>::as_ref("--home"),
            home_path.join(COSMOS_HOME).as_ref(),
        ])
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
