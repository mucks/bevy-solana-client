use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use gloo_net::http::Request;
use serde_json::json;
use solana_sdk::{
    account::Account, bs58, message::Message, pubkey::Pubkey, signature::Keypair,
    system_instruction, transaction::Transaction,
};

impl From<AccountValue> for Account {
    fn from(v: AccountValue) -> Self {
        let data = BASE64_STANDARD.decode(v.data.get(0).unwrap()).unwrap();

        Self {
            data,
            executable: v.executable,
            lamports: v.lamports,
            owner: Pubkey::from_str(&v.owner).unwrap(),
            rent_epoch: v.rent_epoch,
        }
    }
}

#[derive(serde::Deserialize)]
struct ProgramAccountValue {
    pub account: AccountValue,
}
#[derive(serde::Serialize)]
struct RpcRequest<T> {
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
struct RpcResponse<T> {
    jsonrpc: String,
    result: Option<T>,
    error: Option<serde_json::Value>,
    id: u64,
}

#[derive(serde::Deserialize)]
struct RpcResult<T> {
    value: T,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountValue {
    data: Vec<String>,
    executable: bool,
    lamports: u64,
    owner: String,
    rent_epoch: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetLatestBlockhash {
    blockhash: String,
    last_valid_block_height: u64,
}

pub struct RpcClient {
    url: String,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub fn local() -> Self {
        Self::new("http://127.0.0.1:8899".to_string())
    }

    pub fn devnet() -> Self {
        Self::new("https://api.devnet.solana.com".to_string())
    }

    async fn rpc_post<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let resp_str = Request::post(&self.url)
            .header("Content-Type", "application/json")
            .json(&RpcRequest::new(method, params))?
            .send()
            .await?
            .text()
            .await?;

        log::debug!("resp_str: {:?}", resp_str);
        let resp: RpcResponse<T> = serde_json::from_str(&resp_str)?;

        if let Some(e) = resp.error {
            return Err(anyhow!("rpc error: {:?}", e));
        }

        let result = resp.result.context("no result")?;

        Ok(result)
    }

    async fn rpc_post_expect_str(&self, method: &str, params: serde_json::Value) -> Result<String> {
        self.rpc_post::<String>(method, params).await
    }

    async fn rpc_post_expect_result<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        self.rpc_post::<RpcResult<T>>(method, params)
            .await
            .map(|r| r.value)
    }

    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.rpc_post_expect_result("getBalance", json![pubkey.to_string()])
            .await
    }

    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        let opt_acc_val: Option<AccountValue> = self
            .rpc_post_expect_result(
                "getAccountInfo",
                json!([pubkey.to_string(), {"encoding": "base64"}]),
            )
            .await?;

        let v = opt_acc_val.context("could not find account")?;

        Ok(v.into())
    }

    pub async fn send_transaction(&self, tx: &Transaction) -> Result<String> {
        let tx_bytes = bincode::serialize(tx)?;
        let tx: String = bs58::encode(tx_bytes).into_string();
        let resp = self
            .rpc_post_expect_str("sendTransaction", json!([tx]))
            .await
            .context("could not send transaction")?;
        log::debug!("tx hash: {}", resp);
        Ok(tx)
    }

    pub async fn sign_tx(&self, mut tx: Transaction, kp: &Keypair) -> Result<Transaction> {
        tx.sign(&[kp], self.get_latest_blockhash().await?);
        Ok(tx)
    }

    pub async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
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

    pub async fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<Account>> {
        let resp: Vec<ProgramAccountValue> = self
            .rpc_post(
                "getProgramAccounts",
                json!([program_id.to_string(), {"encoding": "base64"}]),
            )
            .await?;

        let accounts = resp.into_iter().map(|v| v.account.into()).collect();

        Ok(accounts)
    }
}

pub fn test_transfer_tx(pubkey: Pubkey) -> Result<Transaction> {
    let to_pubkey = Pubkey::from_str("8dXas6cPLK99H2Ym6Rc64uW9zBdCYUnyxXEYASDUFZcp")?;
    let lamports = 1000000;

    let instruction = system_instruction::transfer(&pubkey, &to_pubkey, lamports);

    let msg = Message::new(&[instruction], Some(&pubkey));

    let tx = Transaction::new_unsigned(msg);
    Ok(tx)
}

#[cfg(test)]
lazy_static::lazy_static! {
pub static ref TEST_KEYPAIR: Keypair = get_local_keypair().expect("could not get local keypair");
pub static ref PROGRAM_ID: Pubkey = Pubkey::from_str("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg").expect("could not parse program id");
}

#[cfg(test)]
fn get_local_keypair() -> Result<Keypair> {
    let keypair_str = include_str!("../test_keypair.json");
    let keypair_bytes: Vec<u8> = serde_json::from_str(keypair_str)?;
    Ok(Keypair::from_bytes(&keypair_bytes)?)
}

#[cfg(all(test, feature = "wasm"))]
mod wasm_tests {
    use solana_sdk::{pubkey::Pubkey, signer::Signer};

    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    async fn test_transfer_send_transaction() -> Result<()> {
        wasm_logger::init(wasm_logger::Config::default());

        let pubkey = TEST_KEYPAIR.try_pubkey()?;

        let client = RpcClient::devnet();

        let hash = client.get_latest_blockhash().await?;

        let mut tx = test_transfer_tx(pubkey)?;

        // tx.sign(&[&TEST_KEYPAIR], hash);

        // client.send_transaction(&tx).await?;

        Ok(())
    }

    #[wasm_bindgen_test]
    async fn test_get_account() -> Result<()> {
        wasm_logger::init(wasm_logger::Config::default());

        let client = RpcClient::devnet();
        let account_pub = "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg";

        let pk = Pubkey::from_str(&account_pub).unwrap();
        let account = client.get_account(&pk).await?;
        log::debug!("\n\naccount: {:?}\n\n", account);
        Ok(())
    }

    #[wasm_bindgen_test]
    async fn test_get_program_accounts() -> Result<()> {
        wasm_logger::init(wasm_logger::Config::default());

        let client = RpcClient::local();

        let accounts = client.get_program_accounts(&PROGRAM_ID).await?;
        log::debug!("\n\naccounts: {:?}\n\n", accounts);
        Ok(())
    }
}
