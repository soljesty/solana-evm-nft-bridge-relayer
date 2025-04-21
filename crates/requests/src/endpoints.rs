use std::str::FromStr;

use crate::{add_pending_request, errors::RequestError, AppState};
use alloy::primitives::Address;
use log::{error, info};
use solana_sdk::pubkey::Pubkey;
use storage::db::Database;
use types::{BRequest, Chains, InputRequest, Status};

pub async fn new_request(
    input_request: InputRequest,
    state: AppState,
) -> Result<BRequest, RequestError> {
    info!("New request received {:?}", input_request);

    let mut request = BRequest::new(input_request);

    if already_existing_request(&request.id, &state.db) {
        return Err(RequestError::AlreadyExistingRequest(request.id));
    }

    let tx_hash = match request.input.origin_network {
        Chains::EVM => {
            let detination_pubkey = Pubkey::from_str(&request.input.destination_account);
            if detination_pubkey.is_err() {
                error!("Invalid destination account {:?}", detination_pubkey.err());
                return Err(RequestError::InvalidDestinationAccount());
            }

            match evm::initialize_evm_request(
                state.evm_client,
                &request.input.contract_or_mint,
                &request.input.token_owner,
                &request.input.token_id,
                &request.id,
            )
            .await
            {
                Ok(tx) => tx,
                Err(err) => {
                    error!("Ethereum transaction has failed {:?}", err);
                    return Err(RequestError::EVMTxError());
                }
            }
        }
        Chains::SOLANA => {
            let destination_owner = Address::from_str(&request.input.destination_account);
            if destination_owner.is_err() {
                error!("Invalid destination account {:?}", destination_owner.err());
                return Err(RequestError::InvalidDestinationAccount());
            }

            match solana::initialize_request(
                &state.solana_client,
                &request.input.contract_or_mint,
                &request.input.token_owner,
                &request.id,
            )
            .await
            {
                Ok(tx) => tx.to_string(),
                Err(err) => {
                    error!("Solana transaction has failed {:?}", err);
                    return Err(RequestError::SolanaTxError());
                }
            }
        }
    };

    if request.add_tx(&tx_hash, &state.db).is_err() {
        return Err(RequestError::CreationError("".to_string()));
    }

    _ = add_pending_request(&request.id, &state.db);

    Ok(request)
}

pub fn get_request(request_id: &str, db: &Database) -> Result<Option<BRequest>, RequestError> {
    if let Ok(Some(request)) = types::request_data(request_id, db) {
        return Ok(Some(request));
    } else {
        return Err(RequestError::NoExistingRequest(request_id.to_string()));
    }
}

pub fn already_existing_request(request_id: &str, db: &Database) -> bool {
    if let Ok(Some(request)) = get_request(request_id, db) {
        if request.status != Status::Canceled && request.status != Status::Completed {
            return true;
        }
    }
    return false;
}

pub fn get_pending_requests(db: &Database) -> Option<Vec<String>> {
    let requests = types::pending_requests(db);
    requests
}

pub fn get_completed_requests(db: &Database) -> Option<Vec<String>> {
    let requests = types::completed_requests(db);
    requests
}
