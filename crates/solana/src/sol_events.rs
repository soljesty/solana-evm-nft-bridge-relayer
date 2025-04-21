use std::str;

use anchor_lang::Discriminator;
use base64::{prelude::BASE64_STANDARD, Engine};
use borsh::BorshDeserialize;
use eyre::Result;
use futures_util::StreamExt;
use log::{error, info};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use storage::db::Database;
use types::Status;

use crate::{check_token_owner, solana_bridge, SolanaClient};

use solana_bridge::events::{NewRequestEvent, TokenMintedEvent};

pub async fn subscribe_event(client: &SolanaClient, db: &Database) -> Result<()> {
    // let mut event_commit: HashSet<String> = HashSet::new();

    let (new_request_discriminator, token_minted_discriminator) = event_discriminators();

    let pubsub_client = PubsubClient::new(&client.ws_url).await.unwrap();
    let (mut subscription, _unsubscribe) = pubsub_client
        .logs_subscribe(
            solana_client::rpc_config::RpcTransactionLogsFilter::All,
            solana_client::rpc_config::RpcTransactionLogsConfig {
                commitment: Some(CommitmentConfig::finalized()),
            },
        )
        .await
        .unwrap();

    info!("Listening for solana events...");

    while let Some(logs) = subscription.next().await {
        for log in logs.value.logs {
            if log.contains(&new_request_discriminator) {
                match event_new_request(log.as_str()) {
                    Ok(event) => {
                        info!("EVENT New Solana request received, request id {} token mint {} token account {}", &event.request_id, &event.mint, &event.user_token_account);
                        // if event_commit.get(&event.request_id).is_some() {
                        // info!("Event received for FINALIZED {:?}", event);
                        check_token_owner(db, client, &event.request_id).await;
                        // event_commit.remove(&event.request_id);
                        // } else {
                        // info!("Event received for CONFIRMED {:?}", event);
                        // Event is received in the commitment of the transaction but we want to process it when it is finalized
                        // event_commit.insert(event.request_id);
                        // }
                    }
                    Err(e) => {
                        error!("Failed to decode event: {}", e);
                    }
                }
            }
            if log.contains(&token_minted_discriminator) {
                match event_token_minted(log.as_str()) {
                    Ok(event) => {
                        info!("EVENT New Solana token minted for request Id {} with token mint {} token account {}", &event.request_id, &event.mint, &event.destination_token_account);
                        // if event_commit.get(&event.request_id).is_some() {
                        // info!("Event received for FINALIZED second time {:?}", event);
                        if let Ok(Some(mut request)) = types::request_data(&event.request_id, db) {
                            if request.status == Status::TokenMinted {
                                if request.output.detination_contract_id_or_mint
                                    == event.mint.to_string()
                                    && request.output.detination_token_id_or_account
                                        == event.destination_token_account.to_string()
                                {
                                    request.update_state(db)?;
                                }
                            }
                        }
                        // event_commit.remove(&event.request_id);
                        // } else {
                        // info!("Event received for CONFIRMED {:?}", event);
                        // Event is received in the commitment of the transaction but we want to process it when it is finalized
                        // event_commit.insert(event.request_id);
                        // }
                    }
                    Err(e) => {
                        error!("Failed to decode event: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

fn event_new_request(base64_data: &str) -> Result<NewRequestEvent> {
    let decoder_data = decode_event(base64_data)?;
    Ok(NewRequestEvent {
        mint: decoder_data.0,
        user_token_account: decoder_data.1,
        request_id: decoder_data.2,
    })
}

fn event_token_minted(base64_data: &str) -> Result<TokenMintedEvent> {
    let decoder_data = decode_event(base64_data)?;
    Ok(TokenMintedEvent {
        mint: decoder_data.0,
        destination_token_account: decoder_data.1,
        request_id: decoder_data.2,
    })
}

pub fn decode_event(base64_data: &str) -> Result<(Pubkey, Pubkey, String)> {
    let log_data: String = base64_data.replace("Program data: ", "");
    let decoded_data = BASE64_STANDARD.decode(log_data)?;
    let trim: Vec<u8> = decoded_data[8..decoded_data.len()].to_vec();

    // Mint + token account size
    let expected_size = 64;
    let (token_data, request_id_data) = trim.split_at(expected_size);

    let mint = Pubkey::try_from_slice(&token_data[0..32])?;
    let token_account = Pubkey::try_from_slice(&token_data[32..64])?;

    // The rest is request id
    let request_id = str::from_utf8(request_id_data)?.to_string();
    let id_trimmed: String = request_id[1..request_id.len()]
        .trim_matches('\0')
        .to_string();

    Ok((mint, token_account, id_trimmed))
}

fn event_discriminators() -> (String, String) {
    // Encoding adds at the end "4=" that is not needed
    let mut new_request_discriminator = BASE64_STANDARD
        .encode(NewRequestEvent::DISCRIMINATOR)
        .trim_end_matches('=')
        .to_string();
    new_request_discriminator.pop();

    let mut token_minted_discriminator = BASE64_STANDARD
        .encode(TokenMintedEvent::DISCRIMINATOR)
        .trim_end_matches('=')
        .to_string();
    token_minted_discriminator.pop();

    (new_request_discriminator, token_minted_discriminator)
}
