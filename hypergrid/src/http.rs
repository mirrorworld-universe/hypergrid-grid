use {
    log::*,
    serde_json::Value,
    std::{result::Result, sync::Arc, time::Duration},
};

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
        let client: reqwest::Client = reqwest::Client::builder()
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

    pub fn get<U: ToString>(&self, url: U) -> Result<String, String> {
        // `block_on()` panics if called within an asynchronous execution context. Whereas
        // `block_in_place()` only panics if called from a current_thread runtime, which is the
        // lesser evil.
        let res = tokio::task::block_in_place(move || {
            self.runtime().block_on(async {
                let response = self.rpc_client.get(url.to_string()).send().await;
                match response {
                    Ok(response) => {
                        let status = response.status();
                        let body = response.text().await.unwrap_or("".to_string());
                        if status.is_success() && !body.is_empty() {
                            Ok(body)
                        } else {
                            error!("Error: {:?}, {:?}", status, body);
                            Err(format!("{:?}: {:?}", status, body))
                        }
                    }
                    Err(e) => {
                        error!("Error: {:?}", e);
                        Err(format!("Error: {:?}", e))
                    }
                }
            })
        });
        res
    }

    pub fn post<U: ToString>(&self, url: U, data: &Value) -> Result<String, String> {
        // `block_on()` panics if called within an asynchronous execution context. Whereas
        // `block_in_place()` only panics if called from a current_thread runtime, which is the
        // lesser evil.
        let res = tokio::task::block_in_place(move || {
            self.runtime().block_on(async {
                let response = self
                    .rpc_client
                    .post(url.to_string())
                    .json(data)
                    .send()
                    .await;
                match response {
                    Ok(response) => {
                        let status = response.status();
                        let body = response.text().await.unwrap_or("".to_string());
                        if status.is_success() && !body.is_empty() {
                            Ok(body)
                        } else {
                            error!("Error: {:?}, {:?}", status, body);
                            Err(format!("{:?}: {:?}", status, body))
                        }
                    }
                    Err(e) => {
                        error!("Error: {:?}", e);
                        Err(format!("Error: {:?}", e))
                    }
                }
            })
        });
        res
    }

    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.as_ref().expect("runtime")
    }
}
