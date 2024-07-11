use std::process::Command;

const COSMOS_APP: &str = "/home/ubuntu/go/bin/hypergrid-ssnd";
const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = "/home/ubuntu/.hypergrid-ssn";
const COSMOS_SIGNER: &str = "my_key";

pub fn run_load_solana_account(pub_key: &str, version:  &str, source: &str) {
    //format the command string
    let cmd_str: String = format!("{} tx hypergridssn create-solana-account {} {} {} --home {} --from {} --chain-id {} --gas 5000000 -y", 
                                COSMOS_APP, pub_key, version, source, COSMOS_HOME, COSMOS_SIGNER, COSMOS_CHAIN_ID);

    let output = Command::new("sh").arg("-c").arg(cmd_str).output().expect("sh exec error!");

    // let output_str = String::from_utf8_lossy(&output.stdout);
    println!("{:?}", String::from_utf8_lossy(&output.stdout));
}
