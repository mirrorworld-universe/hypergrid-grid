use {
    reqwest,
    std::{
        sync::Arc, 
        time::Duration,
        process::Command,
        result::Result,
    },
    log::*,
};



const COSMOS_CHAIN_ID: &str = "hypergridssn";
const COSMOS_HOME: &str = "/home/ubuntu/.hypergrid-ssn";
const COSMOS_APP: &str = " /home/ubuntu/.hypergrid-ssn/bin/hypergrid-ssnd";
const COSMOS_SIGNER: &str = "my_key";

pub fn run_load_solana_account(pub_key: &str, version:  &str, source: &str, update: bool) {
    //format the command string
    let cmd_str: String;
    if update {
        cmd_str = format!("{} tx hypergridssn update-solana-account {} {} --home {} --from {} --chain-id {} --gas 50000000 --keyring-backend test -y", 
                                COSMOS_APP, pub_key, version,  COSMOS_HOME, COSMOS_SIGNER, COSMOS_CHAIN_ID);
    } else {
        cmd_str = format!("{} tx hypergridssn create-solana-account {} {} {} --home {} --from {} --chain-id {} --gas 50000000 --keyring-backend test -y", 
                                COSMOS_APP, pub_key, version, source, COSMOS_HOME, COSMOS_SIGNER, COSMOS_CHAIN_ID);
    }

    println!("cmd_str: {}", cmd_str);
    info!("cmd_str: {}", cmd_str);
    
    let output = Command::new("sh").arg("-c").arg(cmd_str).output();
    match output {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            info!("output: {}", output_str);
            // println!("{:?}", String::from_utf8_lossy(&output.stdout));
        },
        Err(e) => {
            error!("Error: {:?}", e);
        }
    }
}



pub struct HttpClient {
    rpc_client: Arc<reqwest::Client>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for HttpClient {
    fn drop(&mut self) {
        self.runtime.take().expect("runtime").shutdown_background();
    }
}

impl HttpClient {
    pub fn new(timeout: Duration) -> Self {
        let client: reqwest::Client =reqwest::Client::builder()
                .timeout(timeout)
                .pool_idle_timeout(timeout)
                .build()
                .expect("build rpc client");
        Self {
            rpc_client: Arc::new(client),
            runtime: Some(
                tokio::runtime::Builder::new_current_thread()
                    .thread_name("solRpcClient")
                    .enable_io()
                    .enable_time()
                    .build()
                    .unwrap(),
            ),
        }
    }

    pub fn call<U: ToString>(&self, url: U) -> Result<String, String> {
        // `block_on()` panics if called within an asynchronous execution context. Whereas
        // `block_in_place()` only panics if called from a current_thread runtime, which is the
        // lesser evil.
        let res =tokio::task::block_in_place(move || self.runtime().block_on(async {
            let response = self.rpc_client.get(url.to_string()).send().await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or("".to_string());
                    if status.is_success() && body.len() > 0 {
                        Ok(body)
                    } else {
                        Err(format!("{}: {}", status, body))
                    }
                },
                Err(e) => {
                    return Err(format!("Error: {:?}", e));
                }
            }
        }));
        return res;
        
    }

    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.as_ref().expect("runtime")
    }
}