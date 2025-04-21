use std::str::FromStr;

use eyre::Result;
use log::info;
use mpl_token_metadata::accounts::Metadata;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig, program_pack::Pack, pubkey::Pubkey, signature::Signature,
};
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use storage::db::Database;
use types::{MessageMint, Status, TxMessage};

use crate::SolanaClient;

pub fn get_metadata(client: &SolanaClient, token_mint: &str) -> Result<String> {
    let mint_pubkey = Pubkey::from_str(token_mint).expect("Invalid mint address");

    let (metadata_pda, _) = Metadata::find_pda(&mint_pubkey);

    // Fetch account data
    let metadata_account = client
        .rpc
        .get_account_data(&metadata_pda)
        .expect("Failed to get account data");

    // Deserialize Metadata
    let metadata = Metadata::from_bytes(&mut metadata_account.as_ref())
        .expect("Failed to deserialize metadata");

    Ok(metadata.uri.trim_matches('\0').to_owned())
}

pub async fn check_token_owner(db: &Database, client: &SolanaClient, request_id: &str) {
    if let Ok(Some(mut request)) = types::request_data(request_id, db) {
        info!("Checking owner");
        if request.status == Status::RequestReceived {
            let token_mint_pubkey = Pubkey::from_str(&request.input.contract_or_mint).unwrap();
            let bridge_token_account_pubkey =
                spl_associated_token_account::get_associated_token_address(
                    &client.bridge_account,
                    &token_mint_pubkey,
                );
            let data = client
                .rpc
                .get_account_data(&bridge_token_account_pubkey)
                .unwrap();
            if let Ok(token_data) = spl_token::state::Account::unpack(&data) {
                if token_data.owner == client.bridge_account && token_data.amount == 1 {
                    request.update_state(db).unwrap();

                    let metadata = get_metadata(client, &request.input.contract_or_mint).unwrap();

                    client
                        .tx_channel
                        .send(TxMessage {
                            accion: types::Function::Mint,
                            mint_data: Some(MessageMint {
                                request_id: (request_id).to_string(),
                                token_metadata: metadata,
                            }),
                            request_data: None,
                        })
                        .await
                        .unwrap();
                }
            }
        } else {
            info!("Request id already processed");
        }
    } else {
        info!("Not request id db");
    }
}

pub async fn get_transaction_data(
    client: SolanaClient,
    tx: &str,
) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
    let signature = Signature::from_str(tx).expect("Invalid signature");
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::Json),
        commitment: Some(CommitmentConfig::finalized()),
        max_supported_transaction_version: Some(0),
    };
    let get_transaction_with_config = client.rpc.get_transaction_with_config(&signature, config)?;
    return Ok(get_transaction_with_config);
}
