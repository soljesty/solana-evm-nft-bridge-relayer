use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    rpc::types::Transaction,
    sol,
};

use eyre::Result;
use log::info;
use std::str::FromStr;
use storage::db::Database;
use types::{MessageMint, TxMessage};

use crate::{provider_rpc, EVMClient};

sol! {
    #[sol(rpc)]
    interface ERC721Token {
        function ownerOf(uint256 tokenId) external view returns (address);
        function tokenURI(uint256 tokenId) public view virtual override returns (string);
    }
}

pub async fn check_token_owner(client: EVMClient, db: &Database, request_id: &str) -> Result<()> {
    let provider = provider_rpc(client.clone())?;
    if let Ok(Some(mut request)) = types::request_data(&request_id, db) {
        let token_contract = Address::from_str(&request.input.contract_or_mint)?;
        let token_id: U256 = request.input.token_id.parse().expect("Invalid U256 string");

        let contract = ERC721Token::new(token_contract, provider);
        let token_owner = contract.ownerOf(token_id).call().await?._0;

        if token_owner != client.bridge_contract {
            let _ = request.cancel(db);
        }
        request.update_state(db)?;

        let token_metadata = get_token_metadata(client.clone(), token_contract, token_id)
            .await
            .unwrap();

        client
            .tx_channel
            .send(TxMessage {
                accion: types::Function::Mint,
                mint_data: Some(MessageMint {
                    request_id: request_id.to_string(),
                    token_metadata: token_metadata,
                }),
                request_data: None,
            })
            .await
            .unwrap();
    }

    Ok(())
}

pub async fn get_token_metadata(
    client: EVMClient,
    token_contract: Address,
    token_id: U256,
) -> Result<String> {
    let provider = provider_rpc(client.clone())?;

    let contract = ERC721Token::new(token_contract, provider);
    let token_metadata = contract.tokenURI(token_id).call().await?._0;

    info!(
        "Read token contract from evm {}, with token Id {} and metadata {}",
        token_contract, token_id, token_metadata
    );

    Ok(token_metadata)
}

pub async fn get_transaction_data(client: EVMClient, tx: &str) -> Result<Option<Transaction>> {
    let provider = provider_rpc(client.clone())?;
    let tx_hash = tx.parse()?;

    let data = provider.get_transaction_by_hash(tx_hash).await?;
    return Ok(data);
}
