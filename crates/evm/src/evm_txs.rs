use alloy::{
    primitives::{Address, U256},
    providers::{Provider, WalletProvider},
    sol,
};

use eyre::Result;
use log::info;
use std::str::FromStr;
use storage::db::Database;
use tokio::sync::mpsc::Receiver;
use types::{Status, TxMessage};

use crate::{provider_rpc, EVMClient};

const MAX_FEE_PER_GAS: u128 = 3000000000;
const MAX_PRIORIRY_FEE: u128 = 3000000000;

sol! {
    #[sol(rpc)]
    interface BridgeContract {
        function newBridgeRequest(string requestId, address tokenContract, address tokenOwner, uint256 tokenId) external;
        function mintToken(string requestId, address to, uint256 tokenId, string tokenURI) external;
        function tokenAddress() external view returns (address);
    }
}

pub async fn initialize_evm_request(
    client: EVMClient,
    token_contract: &str,
    token_owner: &str,
    token_id: &str,
    request_id: &str,
) -> Result<String> {
    info!("Initialize bridge request from evm");
    let provider = provider_rpc(client.clone())?;

    // Set up the contract interaction
    let token_contract_add = Address::from_str(token_contract)?;
    let token_owner_add = Address::from_str(token_owner)?;
    let token_id_u256: U256 = token_id.parse().expect("Invalid U256 string");

    let contract = BridgeContract::new(client.bridge_contract, provider.clone());

    let signer = provider.default_signer_address();
    let nonce = provider.get_transaction_count(signer).await.unwrap();
    let mut fees = provider.estimate_eip1559_fees().await.unwrap();

    if fees.max_fee_per_gas == 1 && fees.max_priority_fee_per_gas == 1 {
        fees.max_fee_per_gas = MAX_FEE_PER_GAS;
        fees.max_priority_fee_per_gas = MAX_PRIORIRY_FEE;
    }

    // Build the transaction
    let tx = contract
        .newBridgeRequest(
            request_id.to_string(),
            token_contract_add,
            token_owner_add,
            token_id_u256,
        )
        .value(U256::from(0))
        .nonce(nonce)
        .max_fee_per_gas(fees.max_fee_per_gas)
        .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
        .gas(100000)
        .into_transaction_request();

    let _ = provider.call(tx.clone()).await?;

    let pending_tx = provider.send_transaction(tx).await?;

    info!("Transaction sent: {:?}", pending_tx);
    let receipt = pending_tx.register().await?;
    let tx_hash = receipt.tx_hash().to_string();

    Ok(tx_hash)
}

pub async fn mint_new_token(
    client: EVMClient,
    db: &Database,
    request_id: &str,
    token_metadata: &str,
) -> Result<String> {
    if let Ok(Some(mut request)) = types::request_data(request_id, db) {
        let provider = provider_rpc(client.clone())?;

        let mint_account = request.input.contract_or_mint.clone();
        let decoded = bs58::decode(mint_account).into_vec()?;

        let token_id: U256 = U256::from_be_slice(&decoded);

        let contract = BridgeContract::new(client.bridge_contract, provider.clone());

        let destination_owner = Address::from_str(&request.input.destination_account)?;
        let signer = provider.default_signer_address();
        let nonce = provider.get_transaction_count(signer).await.unwrap();
        let mut fees = provider.estimate_eip1559_fees().await.unwrap();

        let destination_contract = contract.tokenAddress().call().await?;

        if fees.max_fee_per_gas == 1 && fees.max_priority_fee_per_gas == 1 {
            fees.max_fee_per_gas = MAX_FEE_PER_GAS;
            fees.max_priority_fee_per_gas = MAX_PRIORIRY_FEE;
        }

        // Build the transaction
        let tx = contract
            .mintToken(
                request_id.to_string(),
                destination_owner,
                token_id,
                token_metadata.to_owned(),
            )
            .value(U256::from(0))
            .nonce(nonce)
            .max_fee_per_gas(fees.max_fee_per_gas)
            .max_priority_fee_per_gas(fees.max_priority_fee_per_gas)
            .gas(200000)
            .into_transaction_request();

        let _ = provider.call(tx.clone()).await?;

        // Send the transaction
        let builder = provider.send_transaction(tx).await?;

        info!("Transaction sent: {:?}", builder);
        let receipt = builder.register().await?;
        let tx_hash = receipt.tx_hash().to_string();

        request.add_tx(&tx_hash, db)?;
        if request.status == Status::TokenReceived {
            request.update_state(db)?;
        }
        request.finalize(
            db,
            &destination_contract._0.to_string(),
            &token_id.to_string(),
        )?;

        return Ok(tx_hash);
    }

    Ok(String::default())
}

pub async fn process_message(
    client: EVMClient,
    db: &Database,
    mut rx_channel: Receiver<TxMessage>,
) {
    while let Some(message) = rx_channel.recv().await {
        info!("Message received in evm tx processor {:?}", &message);
        match message.accion {
            types::Function::Mint => {
                if let Some(mint_data) = message.mint_data {
                    let tx_result = mint_new_token(
                        client.clone(),
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
                    initialize_evm_request(
                        client.clone(),
                        &request_data.token_contract,
                        &request_data.token_owner,
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
