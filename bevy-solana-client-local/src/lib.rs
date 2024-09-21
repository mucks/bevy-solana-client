use anyhow::{bail, Context};
use bevy_solana_client_common::rpc_client::{RpcClient, RpcRequest, RpcResponse};

pub struct LocalRpcClient {
    pub url: String,
}

#[async_trait::async_trait(?Send)]
impl RpcClient for LocalRpcClient {
    async fn rpc_post<De: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<De> {
        let resp_str: String = reqwest::Client::new()
            .post(&self.url)
            .json(&RpcRequest::new(method, params))
            .send()
            .await?
            .text()
            .await?;

        log::debug!("resp_str: {:?}", resp_str);
        let resp: RpcResponse<De> = serde_json::from_str(&resp_str)?;

        if let Some(e) = resp.error {
            bail!("rpc error: {:?}", e);
        }

        let result = resp.result.context("no result")?;

        Ok(result)
    }
}
