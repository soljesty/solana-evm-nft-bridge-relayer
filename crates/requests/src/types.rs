use evm::EVMClient;
use solana::SolanaClient;
use storage::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub solana_client: SolanaClient,
    pub evm_client: EVMClient,
}
