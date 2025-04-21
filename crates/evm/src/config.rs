use alloy::{
    network::EthereumWallet,
    primitives::Address,
    providers::{Provider, ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
use eyre::Result;
use std::{str::FromStr, sync::Arc};
use tokio::sync::mpsc::Sender;
use types::TxMessage;

use crate::provider_type::{MyProviderRPC, MyProviderWS};

#[derive(Clone)]
pub struct EVMClient {
    pub rpc: String,
    pub ws: String,
    pub signer: Arc<EthereumWallet>,
    pub bridge_contract: Address,
    pub tx_channel: Sender<TxMessage>,
    pub block_explorer: String,
}

pub fn evm_initialize(
    rpc_url: &str,
    ws_url: &str,
    account_key: &str,
    bridge_contract: &str,
    tx_channel: Sender<TxMessage>,
    block_explorer: &str,
) -> Result<EVMClient> {
    let signer: PrivateKeySigner = account_key.parse().expect("should parse private key");
    let wallet = EthereumWallet::from(signer.clone());

    let bridge_contract_address = Address::from_str(bridge_contract)?;

    let evm_client = EVMClient {
        rpc: rpc_url.to_string(),
        ws: ws_url.to_string(),
        signer: Arc::new(wallet),
        bridge_contract: bridge_contract_address,
        tx_channel: tx_channel,
        block_explorer: block_explorer.to_string(),
    };

    Ok(evm_client)
}

pub async fn get_latest_block_number(client: &EVMClient) -> Result<u64> {
    let provider = provider_rpc(client.to_owned())?;

    let latest_block = provider.get_block_number().await?;
    Ok(latest_block)
}

pub fn provider_rpc(client: EVMClient) -> Result<MyProviderRPC> {
    let rpc_url = client.rpc.parse()?;

    // Create a provider with the HTTP transport using the `reqwest` crate.
    let provider: MyProviderRPC = ProviderBuilder::new()
        .wallet(client.signer)
        .on_http(rpc_url);

    Ok(provider)
}

pub async fn provider_ws(client: EVMClient) -> Result<MyProviderWS> {
    let rpc_url = client.ws;
    let ws = WsConnect::new(rpc_url);
    let provider: MyProviderWS = ProviderBuilder::new().on_ws(ws).await?;

    Ok(provider)
}
