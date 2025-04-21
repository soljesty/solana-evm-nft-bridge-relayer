use std::{error::Error, time::Duration};

use log::{error, info};
use requests::AppState;
use tokio::sync::mpsc;
use types::TxMessage;

pub async fn start_background_process(
    state: AppState,
    rx_evm: mpsc::Receiver<TxMessage>,
    rx_sol: mpsc::Receiver<TxMessage>,
) -> Result<(), Box<dyn Error>> {
    info!("Reding pending requests");
    if let Some(pending_request) = requests::get_pending_requests(&state.db) {
        tokio::spawn({
            let state_clone = state.clone();
            async move {
                requests::process_pending_request(pending_request, state_clone).await;
            }
        });
    }

    info!("Starting EVM event listener");
    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            match evm::catch_event(state_clone.evm_client.clone(), &state_clone.db).await {
                Ok(_) => error!("EVM event listener exited unexpectedly"),
                Err(e) => error!("EVM event listener failed: {}", e),
            }

            let backoff = Duration::from_secs(5);
            error!(
                "Restarting EVM event listener in {} seconds",
                backoff.as_secs()
            );
            tokio::time::sleep(backoff).await;
        }
    });

    info!("Starting Solana event listener");
    let state_clone = state.clone();
    tokio::spawn(async move {
        match solana::subscribe_event(&state_clone.solana_client, &state_clone.db).await {
            Ok(_) => error!("Solana event listener exited unexpectedly"),
            Err(e) => error!("Solana event listener failed: {}", e),
        }
        let backoff = Duration::from_secs(5);
        error!(
            "Restarting Solana event listener in {} seconds",
            backoff.as_secs()
        );
        tokio::time::sleep(backoff).await;
    });

    info!("Starting EVM message processor");
    let state_clone = state.clone();
    tokio::spawn(async move {
        evm::process_message(state_clone.evm_client, &state_clone.db, rx_evm).await
    });

    info!("Starting Solana message processor");
    let state_clone = state.clone();
    tokio::spawn(async move {
        solana::process_message(state_clone.solana_client, &state_clone.db, rx_sol).await
    });

    Ok(())
}
