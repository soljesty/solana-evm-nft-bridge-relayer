use std::str::FromStr;

use anchor_client::{Client, Cluster};
use eyre::Result;
use log::info;
use solana_sdk::{pubkey::Pubkey, signature::Signature, signer::Signer, transaction::Transaction};
use storage::db::Database;
use tokio::sync::mpsc::Receiver;
use types::{Status, TxMessage};

use crate::{solana_bridge, SolanaClient};

use solana_bridge::client::args;

pub async fn initialize_request(
    client: &SolanaClient,
    mint_account: &str,
    user_account: &str,
    request_id: &str,
) -> Result<Signature> {
    let token_mint_pubkey = Pubkey::from_str(mint_account)?;
    let user_token_account_pubkey = Pubkey::from_str(user_account)?;
    let bridge_token_account_pubkey = spl_associated_token_account::get_associated_token_address(
        &client.bridge_account,
        &token_mint_pubkey,
    );

    info!("Bridge token account {}", bridge_token_account_pubkey);

    let program_client = Client::new(
        Cluster::Custom(client.rpc.url(), client.ws_url.clone()),
        client.signer.clone(),
    );

    let program = program_client.program(client.bridge_program)?;

    let instruction = program
        .request()
        .accounts(solana_bridge::client::accounts::NewRequest {
            bridge: client.bridge_account,
            mint: token_mint_pubkey,
            user_token_account: user_token_account_pubkey,
            bridge_token_account: bridge_token_account_pubkey,
            backend: client.signer.pubkey(),
            system_program: solana_program::system_program::id(),
            token_program: spl_token::ID,
            associated_token_program: spl_associated_token_account::ID,
        })
        .args(args::NewRequest {
            request_id: request_id.to_string(),
        })
        .instructions()?
        .remove(0);

    // Create a transaction and add the instruction
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&client.signer.pubkey()));

    // Sign the transaction
    let recent_blockhash = client.rpc.get_latest_blockhash()?;
    transaction.sign(&[&client.signer], recent_blockhash);

    // Send the transaction
    let signature = client.rpc.send_and_confirm_transaction(&transaction)?;

    info!("Transaction successful with signature: {}", signature);

    Ok(signature)
}

pub async fn mint_new_token(
    client: &SolanaClient,
    db: &Database,
    request_id: &str,
    token_metadata: &str,
) -> Result<Signature> {
    if let Ok(Some(mut request)) = types::request_data(request_id, db) {
        let origin_contract = &request.input.contract_or_mint;
        let detination_account = &request.input.destination_account;
        let token_id = &request.input.token_id;

        let destination_pubkey = Pubkey::from_str(&detination_account)?;
        let token_id_i64 = u64::from_str(&token_id).unwrap();
        let contract_seeds = origin_contract.split_at(origin_contract.len() / 2);

        let mint_pubkey = Pubkey::find_program_address(
            &[
                b"mint",
                contract_seeds.0.as_bytes(),
                contract_seeds.1.as_bytes(),
                &token_id_i64.to_le_bytes(),
            ],
            &client.bridge_program,
        )
        .0;

        let user_token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &destination_pubkey,
            &mint_pubkey,
        );

        info!(
            "User token account {} for mint {}",
            user_token_account_pubkey, mint_pubkey
        );

        let metadata_pubkey = Pubkey::find_program_address(
            &[
                b"metadata",
                &mpl_token_metadata::ID.to_bytes(),
                &mint_pubkey.to_bytes(),
            ],
            &mpl_token_metadata::ID,
        )
        .0;

        let mmasteredition_pubkey = Pubkey::find_program_address(
            &[
                b"metadata",
                &mpl_token_metadata::ID.to_bytes(),
                &mint_pubkey.to_bytes(),
                b"edition",
            ],
            &mpl_token_metadata::ID,
        )
        .0;

        let program_client = Client::new(
            Cluster::Custom(client.rpc.url(), client.ws_url.clone()),
            client.signer.clone(),
        );

        let program = program_client.program(client.bridge_program)?;

        let instruction = program
            .request()
            .accounts(solana_bridge::client::accounts::CreateNft {
                bridge: client.bridge_account,
                mint: mint_pubkey,
                destination_token_account: user_token_account_pubkey,
                backend: client.signer.pubkey(),
                nft_metadata: metadata_pubkey,
                master_edition_account: mmasteredition_pubkey,
                associated_token_program: spl_associated_token_account::ID,
                recipient: destination_pubkey,
                token_program: spl_token::ID,
                rent: solana_program::sysvar::rent::ID,
                metadata_program: mpl_token_metadata::ID,
                system_program: solana_program::system_program::id(),
            })
            .args(args::CreateNft {
                id: token_id_i64,
                seed_p1: contract_seeds.0.to_string(),
                seed_p2: contract_seeds.1.to_string(),
                name: "Bridged NFT".to_string(),
                symbol: "BNFT".to_string(),
                uri: token_metadata.to_string(),
                request_id: request_id.to_string(),
            })
            .instructions()?
            .remove(0);

        // Create a transaction and add the instruction
        let mut transaction =
            Transaction::new_with_payer(&[instruction], Some(&client.signer.pubkey()));

        // Sign the transaction
        let recent_blockhash = client.rpc.get_latest_blockhash()?;
        transaction.sign(&[&client.signer], recent_blockhash);

        // Send the transaction
        let signature = client.rpc.send_and_confirm_transaction(&transaction)?;

        info!("Transaction successful with signature: {}", signature);

        request.add_tx(&signature.to_string(), db)?;
        if request.status == Status::TokenReceived {
            request.update_state(db)?;
        }
        request.finalize(
            db,
            &mint_pubkey.to_string(),
            &user_token_account_pubkey.to_string(),
        )?;

        return Ok(signature);
    }
    Ok(Signature::default())
}

pub async fn process_message(
    client: SolanaClient,
    db: &Database,
    mut rx_channel: Receiver<TxMessage>,
) {
    while let Some(message) = rx_channel.recv().await {
        info!("Message received in solana tx processor {:?}", &message);
        match message.accion {
            types::Function::Mint => {
                if let Some(mint_data) = message.mint_data {
                    let tx_result = mint_new_token(
                        &client,
                        db,
                        &mint_data.request_id,
                        &mint_data.token_metadata,
                    )
                    .await;
                    info!("Transaction result {:?}", tx_result);
                }
            }
            // TODO not used yet
            types::Function::NewRequest => {
                if let Some(request_data) = message.request_data {
                    initialize_request(
                        &client,
                        &request_data.token_contract,
                        &request_data.token_id,
                        &request_data.request_id,
                    )
                    .await
                    .unwrap();
                }
            }
        }
    }
}
