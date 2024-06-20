use std::process::Command;
use std::error;

const COSMOS_APP: &str = "/home/ubuntu/go/bin/hypergrid-ssnd";
const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = "/home/ubuntu/.hypergrid-ssn";
const COSMOS_SIGNER: &str = "alice";

fn run_load_solana_account(pub_key: &str, version:  &str) {
    //format the command string
    let cmd_str: String = format!("{} tx hypergridssn create-solana-account {} {} --home {} --from {} --chain-id {} --gas 5000000 -y", COSMOS_APP, pub_key, version, COSMOS_HOME, COSMOS_SIGNER, COSMOS_CHAIN_ID);

    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").arg("/c").arg(cmd_str).output().expect("cmd exec error!")
    } else {
        Command::new("sh").arg("-c").arg(cmd_str).output().expect("sh exec error!")
    };

    // let output_str = String::from_utf8_lossy(&output.stdout);
    println!("{:?}", String::from_utf8_lossy(&output.stdout));
}

fn main() -> Result<(), Box<dyn error::Error>> {
    run_load_solana_account("Csz6Y33L28jpQ58rNosdtPrsHUgxQMhX9HyLfahMc8b9", "0");

    Ok(())
}