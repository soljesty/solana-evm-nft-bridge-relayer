use alloy::{
    eips::BlockNumberOrTag, providers::Provider, rpc::types::Filter, sol, sol_types::SolEvent,
};
use eyre::Result;
use futures_util::stream::StreamExt;
use log::info;
use storage::db::Database;
use types::Status;

use crate::{check_token_owner, provider_ws, EVMClient};

sol! {
    #[sol(rpc)]
    event NewRequest(string requestId, address tokenContract, uint256 tokenId);
    event TokenMinted(string requestId, address tokenContract, address to, uint256 tokenId);
}

pub async fn catch_event(client: EVMClient, db: &Database) -> Result<()> {
    let provider = provider_ws(client.clone()).await?;

    let filter_request = Filter::new()
        .address(client.bridge_contract)
        .event(NewRequest::SIGNATURE)
        .from_block(BlockNumberOrTag::Latest);

    let filter_mint = Filter::new()
        .address(client.bridge_contract)
        .event(TokenMinted::SIGNATURE)
        .from_block(BlockNumberOrTag::Latest);

    let sub_request = provider.subscribe_logs(&filter_request).await.unwrap();
    let sub_mint = provider.subscribe_logs(&filter_mint).await.unwrap();

    let mut stream =
        futures_util::stream::select(sub_request.into_stream(), sub_mint.into_stream());

    info!("Listening for evm events...");
    while let Some(log) = stream.next().await {
        match log.topic0() {
            Some(&NewRequest::SIGNATURE_HASH) => {
                let NewRequest {
                    requestId,
                    tokenContract,
                    tokenId,
                } = log.log_decode()?.inner.data;
                info!("EVENT New EVM bridge request event, request id: {}, token contract {:?}, token id {:?}", &requestId, &tokenContract, &tokenId);
                check_token_owner(client.clone(), db, &requestId)
                    .await
                    .unwrap();
            }
            Some(&TokenMinted::SIGNATURE_HASH) => {
                let TokenMinted {
                    requestId,
                    tokenContract,
                    to,
                    tokenId,
                } = log.log_decode()?.inner.data;
                info!("EVENT New EVM token minted for request Id {requestId} with token contract {tokenContract} to account {to} and token id {tokenId}");
                if let Ok(Some(mut request)) = types::request_data(&requestId, db) {
                    if request.status == Status::TokenMinted {
                        if request.output.detination_contract_id_or_mint
                            == tokenContract.to_string()
                            && request.output.detination_token_id_or_account == tokenId.to_string()
                        {
                            request.update_state(db)?;
                        }
                    }
                }
            }
            _ => (),
        }
    }
    Ok(())
}
