use anchor_lang::declare_program;
use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
};
use std::{str::FromStr, sync::Arc};
use tokio::sync::mpsc::Sender;
use types::TxMessage;

declare_program!(solana_bridge);

#[derive(Clone)]
pub struct SolanaClient {
    pub rpc: Arc<RpcClient>,
    pub ws_url: String,
    pub signer: Arc<Keypair>,
    pub bridge_program: Pubkey,
    pub bridge_account: Pubkey,
    pub tx_channel: Sender<TxMessage>,
    pub block_explorer: String,
}

pub fn solana_connection(
    rpc_url: &str,
    ws_url: &str,
    keypair_path: &str,
    bridge_program: &str,
    bridge_account: &str,
    tx_channel: Sender<TxMessage>,
    block_explorer: &str,
) -> Result<SolanaClient> {
    let client: RpcClient =
        RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

    let payer = read_keypair_file(keypair_path)
        .map_err(|e| format!("Solana keypair file not found, {}", e))
        .unwrap();
    let bridge_program_pubkey = Pubkey::from_str(bridge_program)?;
    let bridge_account_pubkey = Pubkey::from_str(bridge_account)?;

    let solana_client = SolanaClient {
        rpc: Arc::new(client),
        ws_url: ws_url.to_string(),
        signer: Arc::new(payer),
        bridge_program: bridge_program_pubkey,
        bridge_account: bridge_account_pubkey,
        tx_channel: tx_channel,
        block_explorer: block_explorer.to_string(),
    };

    Ok(solana_client)
}

pub async fn get_latest_slot(client: &SolanaClient) -> Result<u64> {
    let latest_slot = client.rpc.get_slot()?;
    Ok(latest_slot)
}
