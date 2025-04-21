use std::error::Error;

use api::routes::api_router;
use background_process::start_background_process;
use evm::get_latest_block_number;
use log::info;
use requests::AppState;
use serde::Deserialize;
use solana::get_latest_slot;
use storage::db::Database;
use tokio::sync::mpsc;
use types::TxMessage;

mod background_process;

#[derive(Deserialize, Debug)]
struct Config {
    db_path: String,
    evm_rpc: String,
    evm_ws: String,
    evm_pk: String,
    evm_bridge_contract: String,
    evm_block_explorer: String,
    solana_wallet: String,
    solana_rpc: String,
    solana_ws: String,
    solana_bridge_program: String,
    solana_bridge_account: String,
    solana_block_explorer: String,
    port: u16,
}

/// Main entry point for the Bridge Relayer
///
/// This function initializes all components of the bridge:
/// 1. Sets up logging
/// 2. Loads configuration from environment variables
/// 3. Creates communication channels between components
/// 4. Initializes the database
/// 5. Connects to Solana and EVM blockchains
/// 6. Starts event listeners and request processors
/// 7. Starts the API server
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    info!("Starting bridge relayer");

    dotenvy::dotenv().map_err(|e| format!("Failed to load .env file: {}", e))?;

    // Load configuration from environment variables
    let config = envy::from_env::<Config>().map_err(|e| format!("Configuration error: {}", e))?;

    // Create channels for communication between components
    let (tx_evm, rx_evm) = mpsc::channel::<TxMessage>(50);
    let (tx_sol, rx_sol) = mpsc::channel::<TxMessage>(50);

    info!("Opening database at {}", &config.db_path);
    let db =
        Database::open(config.db_path).map_err(|e| format!("Failed to open database at: {}", e))?;

    info!("Connecting to Solana at {}", config.solana_rpc);
    let solana_client = solana::solana_connection(
        &config.solana_rpc,
        &config.solana_ws,
        &config.solana_wallet,
        &config.solana_bridge_program,
        &config.solana_bridge_account,
        tx_evm.clone(),
        &config.solana_block_explorer,
    )
    .map_err(|e| {
        format!(
            "Failed to connect to Solana RPC at {}: {}",
            config.solana_rpc, e
        )
    })?;

    info!("Connecting to EVM at {}", config.evm_rpc);
    let evm_client = evm::evm_initialize(
        &config.evm_rpc,
        &config.evm_ws,
        &config.evm_pk,
        &config.evm_bridge_contract,
        tx_sol.clone(),
        &config.evm_block_explorer,
    )
    .map_err(|e| {
        format!(
            "Failed to initialize EVM client at {}: {}",
            config.evm_rpc, e
        )
    })?;

    // Test connections with timeouts
    info!("Testing connections");
    let evm_test = get_latest_block_number(&evm_client)
        .await
        .map_err(|_| "EVM connection test timed out")?;
    info!("EVM connection successful, latest block: {}", evm_test);

    let solana_test = get_latest_slot(&solana_client)
        .await
        .map_err(|_| "Solana connection test timed out")?;
    info!("Solana connection successful, latest slot: {}", solana_test);

    // Create application state to be shared across components
    let state = AppState {
        db: db.clone(),
        solana_client: solana_client.clone(),
        evm_client: evm_client.clone(),
    };

    start_background_process(state.clone(), rx_evm, rx_sol)
        .await
        .map_err(|e| format!("Background process initialize failed: {}", e))?;

    // Initialize and start the API server
    let app = api_router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;

    // Signal handling for graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    setup_signal_handlers(shutdown_tx);

    let server = axum::serve(listener, app);
    let server_handle = server.with_graceful_shutdown(async {
        let _ = shutdown_rx.await;
        info!("Shutdown signal received, shutting down gracefully");
    });

    info!("Server started successfully");
    server_handle.await?;
    info!("Server shutdown complete");

    Ok(())
}

/// Setup signal handlers for graceful shutdown
fn setup_signal_handlers(shutdown_tx: tokio::sync::oneshot::Sender<()>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        tokio::spawn(async move {
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");
            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to create SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("SIGTERM received");
                },
                _ = sigint.recv() => {
                    info!("SIGINT received");
                },
            }

            let _ = shutdown_tx.send(());
        });
    }

    #[cfg(not(unix))]
    {
        use tokio::signal::ctrl_c;

        tokio::spawn(async move {
            let _ = ctrl_c().await;
            info!("Ctrl+C received");
            let _ = shutdown_tx.send(());
        });
    }
}
