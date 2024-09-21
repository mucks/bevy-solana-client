use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use serde_json::json;
use solana_sdk::{
    account::Account, bs58, message::Message, pubkey::Pubkey, signature::Keypair,
    system_instruction, transaction::Transaction,
};

#[derive(serde::Serialize)]
pub struct RpcRequest<T> {
    jsonrpc: String,
    method: String,
    id: u32,
    params: T,
}

impl<T> RpcRequest<T> {
    pub fn new(method: &str, params: T) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            id: 1,
            params,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct RpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<serde_json::Value>,
    pub id: u64,
}

#[derive(serde::Deserialize)]
pub struct RpcResult<T> {
    value: T,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLatestBlockhash {
    pub blockhash: String,
    pub last_valid_block_height: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcAccountInfo {
    data: [String; 2],
    executable: bool,
    lamports: u64,
    owner: String,
    rent_epoch: u64,
}

impl From<RpcAccountInfo> for Account {
    fn from(rpc_acc: RpcAccountInfo) -> Self {
        let data = BASE64_STANDARD.decode(rpc_acc.data[0].as_bytes()).unwrap();
        let owner = Pubkey::from_str(&rpc_acc.owner).unwrap();
        let lamports = rpc_acc.lamports;
        let rent_epoch = rpc_acc.rent_epoch;
        let executable = rpc_acc.executable;

        Account {
            data,
            owner,
            lamports,
            rent_epoch,
            executable,
        }
    }
}

#[async_trait::async_trait(?Send)]
pub trait RpcClient {
    async fn rpc_post<De: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<De>;

    async fn rpc_post_expect_str(&self, method: &str, params: serde_json::Value) -> Result<String> {
        self.rpc_post::<String>(method, params).await
    }

    async fn rpc_post_expect_result<De: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<De> {
        self.rpc_post::<RpcResult<De>>(method, params)
            .await
            .map(|r| r.value)
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.rpc_post_expect_result("getBalance", json![pubkey.to_string()])
            .await
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        let opt_acc_val: Option<RpcAccountInfo> = self
            .rpc_post_expect_result(
                "getAccountInfo",
                json!([pubkey.to_string(), {"encoding": "base64"}]),
            )
            .await?;

        Ok(opt_acc_val.context("could not find account")?.into())
    }

    async fn send_transaction(&self, tx: &Transaction) -> Result<String> {
        let tx_bytes = bincode::serialize(tx)?;
        let tx: String = bs58::encode(tx_bytes).into_string();
        let resp = self
            .rpc_post_expect_str("sendTransaction", json!([tx]))
            .await
            .context("could not send transaction")?;
        log::debug!("tx hash: {}", resp);
        Ok(tx)
    }

    async fn sign_tx(&self, mut tx: Transaction, kp: &Keypair) -> Result<Transaction> {
        tx.sign(&[kp], self.get_latest_blockhash().await?);
        Ok(tx)
    }

    async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        let resp: GetLatestBlockhash = self
            .rpc_post_expect_result("getLatestBlockhash", json!([{"commitment": "finalized"}]))
            .await?;

        let hash_bytes: [u8; 32] = bs58::decode(resp.blockhash)
            .into_vec()
            .context("could not decode blockhash")?
            .try_into()
            .map_err(|e| anyhow!("{:?}", e))?;

        let hash = solana_sdk::hash::Hash::new(&hash_bytes);

        Ok(hash)
    }

    async fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Account, Pubkey)>> {
        let resp: Vec<(Account, Pubkey)> = self
            .rpc_post(
                "getProgramAccounts",
                json!([program_id.to_string(), {"encoding": "base64"}]),
            )
            .await?;

        Ok(resp.into_iter().collect())
    }
}

pub const SOLANA_DEVNET_URL: &str = "https://api.devnet.solana.com";
pub const SOLANA_LOCAL_URL: &str = "http://127.0.0.1:8899";

pub fn test_transfer_tx(pubkey: Pubkey) -> Result<Transaction> {
    let to_pubkey = Pubkey::from_str("8dXas6cPLK99H2Ym6Rc64uW9zBdCYUnyxXEYASDUFZcp")?;
    let lamports = 1000000;

    let instruction = system_instruction::transfer(&pubkey, &to_pubkey, lamports);

    let msg = Message::new(&[instruction], Some(&pubkey));

    let tx = Transaction::new_unsigned(msg);
    Ok(tx)
}
