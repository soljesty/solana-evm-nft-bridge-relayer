use crate::{errors::RequestError, get_pending_requests, AppState};
use alloy::primitives::{Address, U256};
use eyre::Result;
use log::{error, info};
use std::{collections::HashMap, str::FromStr, thread::sleep, time::Duration};
use storage::{
    db::Database,
    keys::{PENDING_REQUESTS, PENDING_REQUESTS_INDEX},
};
use types::{update_hashmap, update_vector, BRequest, Chains, Status};

pub fn get_pending_request_and_index(
    db: &Database,
) -> (Option<Vec<String>>, Option<HashMap<String, i128>>) {
    let pending_requests = get_pending_requests(db);
    let pending_requests_index: Option<HashMap<String, i128>> =
        db.read(PENDING_REQUESTS_INDEX).unwrap();
    info!("Reading pending requests: {:?}", &pending_requests);
    (pending_requests, pending_requests_index)
}

pub fn add_pending_request(request_id: &str, db: &Database) -> Result<()> {
    let (pending_requests, pending_requests_index): (
        Option<Vec<String>>,
        Option<HashMap<String, i128>>,
    ) = get_pending_request_and_index(&db);
    info!("Adding new request to pending: {request_id}");

    if let Some(mut pending) = pending_requests {
        let index = pending.len();
        pending.push(request_id.to_string());
        update_pending_vector(db, pending)?;

        let mut indexes = pending_requests_index.unwrap();
        indexes.insert(request_id.to_owned(), index as i128);
        update_pending_hashmap(db, indexes)?;
    } else {
        let pending = vec![request_id.to_string()];
        update_pending_vector(db, pending)?;

        let mut indexes = HashMap::new();
        indexes.insert(request_id.to_owned(), 0);
        update_pending_hashmap(db, indexes)?;
    }
    Ok(())
}

pub fn remove_pending_request(request_id: &str, db: &Database) -> Result<()> {
    let (pending_requests, pending_requests_index): (
        Option<Vec<String>>,
        Option<HashMap<String, i128>>,
    ) = get_pending_request_and_index(&db);
    info!("Removing request from pending: {request_id}");

    if let Some(mut pending) = pending_requests {
        let mut indexes = pending_requests_index.unwrap();
        let request_index = indexes.remove(request_id).unwrap();

        let last_id = pending[pending.len() - 1].clone();

        pending.swap_remove(request_index as usize);
        update_pending_vector(db, pending)?;

        if let Some(value) = indexes.get_mut(&last_id) {
            *value = request_index;
        }
        update_pending_hashmap(db, indexes)?;
    }
    Ok(())
}

fn update_pending_vector(db: &Database, requests: Vec<String>) -> Result<()> {
    _ = update_vector(db, PENDING_REQUESTS, requests)
        .map_err(|e| RequestError::CreationError(e.to_string()));
    Ok(())
}

fn update_pending_hashmap(db: &Database, indexes: HashMap<String, i128>) -> Result<()> {
    _ = update_hashmap(db, PENDING_REQUESTS_INDEX, indexes)
        .map_err(|e| RequestError::CreationError(e.to_string()));
    Ok(())
}

pub async fn process_pending_request(pending: Vec<String>, state: AppState) {
    for id in pending {
        if let Some(mut request) = state.db.read::<_, BRequest>(&id).unwrap() {
            info!("Request in pending: {:?}", request.clone());

            match request.input.origin_network {
                Chains::EVM => {
                    let processed = process_evm_pending_request(request.clone(), &state).await;
                    if processed.is_err() {
                        let error_msg = processed.err().unwrap().to_string();
                        error!(
                            "Processing pending request {}, error {:?}",
                            &request.id, &error_msg
                        );
                        if error_msg.contains("address") && error_msg.contains("already in use") {
                            info!("Canceling pending request {}", &request.id);
                            request.cancel(&state.db).unwrap_or_else(|err| {
                                error!(
                                    "Could not cancel pending request {}, error {:?}",
                                    &request.id, &err
                                );
                            });
                        }
                    }
                }
                Chains::SOLANA => {
                    let processed = process_solana_pending_request(request.clone(), &state).await;
                    if processed.is_err() {
                        error!(
                            "Processing pending request {}, error {:?}",
                            &request.id,
                            &processed.err()
                        );
                    }
                }
            }
        } else {
            error!("Error processing pending requests");
        }
        sleep(Duration::from_secs(8));
    }
}

async fn process_evm_pending_request(mut request: BRequest, state: &AppState) -> Result<()> {
    match request.status {
        Status::RequestReceived => {
            evm::check_token_owner(state.evm_client.clone(), &state.db, &request.id).await?;
            Ok(())
        }
        Status::TokenReceived => {
            continue_from_metadata(state, &request).await?;
            Ok(())
        }
        Status::TokenMinted => {
            let last_tx = &request.tx_hashes[request.tx_hashes.len() - 1];
            if solana::get_transaction_data(state.solana_client.clone(), &last_tx)
                .await
                .is_err()
            {
                continue_from_metadata(state, &request).await?;
            } else {
                // If the destination token has metadata it, the process was completed
                if let Ok(_) = solana::get_metadata(
                    &state.solana_client.clone(),
                    &request.output.detination_contract_id_or_mint,
                ) {
                    request.update_state(&state.db)?;
                } else {
                    // If not exist send the transaction to mint the token again
                    continue_from_metadata(state, &request).await?;
                }
            }
            Ok(())
        }
        Status::Completed => Ok(remove_pending_request(&request.id, &state.db)?),
        Status::Canceled => Ok(remove_pending_request(&request.id, &state.db)?),
    }
}

async fn process_solana_pending_request(mut request: BRequest, state: &AppState) -> Result<()> {
    match request.status {
        Status::RequestReceived => {
            solana::check_token_owner(&state.db, &state.solana_client, &request.id).await;
            Ok(())
        }
        Status::TokenReceived => {
            continue_from_metadata(state, &request).await?;
            Ok(())
        }
        Status::TokenMinted => {
            let last_tx = &request.tx_hashes[request.tx_hashes.len() - 1];
            if evm::get_transaction_data(state.evm_client.clone(), &last_tx)
                .await
                .unwrap()
                .is_none()
            {
                continue_from_metadata(state, &request).await?;
            } else {
                let data = evm::get_transaction_data(state.evm_client.clone(), &last_tx)
                    .await
                    .unwrap();
                info!("Transaction data exist {:?}", data);
                let token_contract =
                    Address::from_str(&request.output.detination_contract_id_or_mint).unwrap();
                let token_id: U256 = request
                    .output
                    .detination_token_id_or_account
                    .parse()
                    .expect("Invalid U256 string");

                // If the destination token has metadata it, the process was completed
                if evm::get_token_metadata(state.evm_client.clone(), token_contract, token_id)
                    .await
                    .is_ok()
                {
                    request.update_state(&state.db)?;
                } else {
                    // If not exist send the transaction to mint the token again
                    continue_from_metadata(state, &request).await?;
                }
            }
            Ok(())
        }
        Status::Completed => Ok(remove_pending_request(&request.id, &state.db)?),
        Status::Canceled => Ok(remove_pending_request(&request.id, &state.db)?),
    }
}

async fn continue_from_metadata(state: &AppState, request: &BRequest) -> Result<()> {
    match request.input.origin_network {
        Chains::EVM => {
            let token_contract = Address::from_str(&request.input.contract_or_mint).unwrap();
            let token_id: U256 = request.input.token_id.parse().expect("Invalid U256 string");
            if let Ok(metadata) =
                evm::get_token_metadata(state.evm_client.clone(), token_contract, token_id).await
            {
                solana::mint_new_token(&state.solana_client, &state.db, &request.id, &metadata)
                    .await?;
            }
            Ok(())
        }
        Chains::SOLANA => {
            if let Ok(metadata) =
                solana::get_metadata(&state.solana_client, &request.input.contract_or_mint)
            {
                evm::mint_new_token(state.evm_client.clone(), &state.db, &request.id, &metadata)
                    .await?;
            }
            Ok(())
        }
    }
}
