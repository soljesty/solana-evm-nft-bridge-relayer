use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy::primitives::keccak256;

use eyre::Result;
use log::info;
use serde::{Deserialize, Serialize};
use storage::db::Database;

use crate::add_completed_request;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Status {
    RequestReceived,
    TokenReceived,
    TokenMinted,
    Completed,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Chains {
    EVM,
    SOLANA,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InputRequest {
    pub contract_or_mint: String,
    pub token_id: String,
    pub token_owner: String,
    pub origin_network: Chains,
    pub destination_account: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct OutputResult {
    pub detination_token_id_or_account: String,
    pub detination_contract_id_or_mint: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BRequest {
    pub id: String,
    pub status: Status,
    pub input: InputRequest,
    pub tx_hashes: Vec<String>,
    pub output: OutputResult,
    pub last_update: Duration,
}

impl BRequest {
    pub fn new(input: InputRequest) -> Self {
        let request_id =
            BRequest::generate_id(&input.contract_or_mint, &input.token_id, &input.token_owner);
        BRequest {
            id: request_id,
            status: Status::RequestReceived,
            input,
            tx_hashes: vec![],
            output: OutputResult::default(),
            last_update: Self::current_time(),
        }
    }

    pub fn update_state(&mut self, db: &Database) -> Result<()> {
        match self.status {
            Status::RequestReceived => self.status = Status::TokenReceived,
            Status::TokenReceived => self.status = Status::TokenMinted,
            Status::TokenMinted => self.status = Status::Completed,
            Status::Completed | Status::Canceled => {}
        }
        self.last_update = Self::current_time();

        db.write_value(&self.id, &self)?;
        info!("Request id {} status updated {:?}", self.id, self.status);
        Ok(())
    }

    pub fn cancel(&mut self, db: &Database) -> Result<()> {
        self.status = Status::Canceled;

        db.write_value(&self.id, &self)?;
        Ok(())
    }

    pub fn finalize(&mut self, db: &Database, token_contract: &str, token_id: &str) -> Result<()> {
        self.output.detination_contract_id_or_mint = token_contract.to_string();
        self.output.detination_token_id_or_account = token_id.to_string();
        self.last_update = Self::current_time();

        db.write_value(&self.id, &self)?;
        add_completed_request(&self.id, db)?;
        Ok(())
    }

    pub fn add_tx(&mut self, tx: &str, db: &Database) -> Result<()> {
        self.tx_hashes.push(tx.to_string());
        db.write_value(&self.id, &self)?;
        Ok(())
    }

    pub fn generate_id(contract: &str, token_id: &str, token_owner: &str) -> String {
        let mut data = Vec::new();
        data.extend_from_slice(contract.as_bytes());
        data.extend_from_slice(token_id.as_bytes());
        data.extend_from_slice(token_owner.as_bytes());

        keccak256(&data).to_string()
    }

    fn current_time() -> Duration {
        let now = SystemTime::now();
        now.duration_since(UNIX_EPOCH).expect("Time went backwards")
    }
}

// Api input request types
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SolanaInputRequest {
    pub token_mint: String,
    pub token_account: String,
    pub origin_network: Chains,
    pub destination_account: String,
}

impl From<SolanaInputRequest> for InputRequest {
    fn from(sol_input: SolanaInputRequest) -> Self {
        InputRequest {
            contract_or_mint: sol_input.token_mint,
            token_id: "".to_string(),
            token_owner: sol_input.token_account,
            origin_network: sol_input.origin_network,
            destination_account: sol_input.destination_account,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EVMInputRequest {
    pub token_contract: String,
    pub token_id: String,
    pub token_owner: String,
    pub origin_network: Chains,
    pub destination_account: String,
}

impl From<EVMInputRequest> for InputRequest {
    fn from(evm_input: EVMInputRequest) -> Self {
        InputRequest {
            contract_or_mint: evm_input.token_contract,
            token_id: evm_input.token_id,
            token_owner: evm_input.token_owner,
            origin_network: evm_input.origin_network,
            destination_account: evm_input.destination_account,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Function {
    Mint,
    NewRequest,
}

#[derive(Debug, Clone)]
pub struct TxMessage {
    pub accion: Function,
    pub mint_data: Option<MessageMint>,
    pub request_data: Option<MessageNewRequest>,
}

#[derive(Debug, Clone)]
pub struct MessageMint {
    pub request_id: String,
    pub token_metadata: String,
}

#[derive(Debug, Clone)]
pub struct MessageNewRequest {
    pub token_contract: String,
    pub token_owner: String,
    pub token_id: String,
    pub request_id: String,
}

#[cfg(test)]
mod test {
    use crate::{
        completed_requests, BRequest, Chains, EVMInputRequest, Function, InputRequest, MessageMint,
        MessageNewRequest, OutputResult, SolanaInputRequest, Status, TxMessage,
    };
    use storage::db::Database;
    use tempfile::tempdir;

    // Helper function to create a test database
    fn setup_test_db() -> Database {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        Database::open(path).unwrap()
    }

    // Helper function to create a test InputRequest
    fn create_test_input_request() -> InputRequest {
        InputRequest {
            contract_or_mint: "0xabc123".to_string(),
            token_id: "42".to_string(),
            token_owner: "0xowner456".to_string(),
            origin_network: Chains::EVM,
            destination_account: "0xdestination789".to_string(),
        }
    }

    #[test]
    fn test_status_enum() {
        // Test that Status enum variants exist and can be compared
        assert_ne!(Status::RequestReceived, Status::TokenReceived);
        assert_ne!(Status::TokenReceived, Status::TokenMinted);
        assert_ne!(Status::TokenMinted, Status::Completed);
        assert_ne!(Status::Completed, Status::Canceled);
    }

    #[test]
    fn test_chains_enum() {
        // Test that Chains enum variants exist and can be compared
        assert_ne!(Chains::EVM, Chains::SOLANA);
    }

    #[test]
    fn test_brequest_new() {
        let input = create_test_input_request();
        let request = BRequest::new(input.clone());

        // Check that the request was created with the correct values
        assert_eq!(request.status, Status::RequestReceived);
        assert_eq!(request.input, input);
        assert!(request.tx_hashes.is_empty());
        assert_eq!(request.output, OutputResult::default());

        // Check that the ID was generated correctly
        let expected_id =
            BRequest::generate_id(&input.contract_or_mint, &input.token_id, &input.token_owner);
        assert_eq!(request.id, expected_id);
    }

    #[test]
    fn test_brequest_generate_id() {
        // Test that generate_id produces consistent results
        let id1 = BRequest::generate_id("contract1", "token1", "owner1");
        let id2 = BRequest::generate_id("contract1", "token1", "owner1");
        let id3 = BRequest::generate_id("contract2", "token1", "owner1");

        assert_eq!(id1, id2); // Same inputs should produce same ID
        assert_ne!(id1, id3); // Different inputs should produce different IDs
    }

    #[test]
    fn test_brequest_update_state() {
        let db = setup_test_db();
        let input = create_test_input_request();
        let mut request = BRequest::new(input);

        // Initial state
        assert_eq!(request.status, Status::RequestReceived);

        // Update state and check transitions
        request.update_state(&db).unwrap();
        assert_eq!(request.status, Status::TokenReceived);

        request.update_state(&db).unwrap();
        assert_eq!(request.status, Status::TokenMinted);

        request.update_state(&db).unwrap();
        assert_eq!(request.status, Status::Completed);

        // State should not change after Completed
        request.update_state(&db).unwrap();
        assert_eq!(request.status, Status::Completed);

        // Verify the request was saved to the database
        let retrieved: BRequest = db.read(&request.id).unwrap().unwrap();
        assert_eq!(retrieved.status, Status::Completed);
    }

    #[test]
    fn test_brequest_cancel() {
        let db = setup_test_db();
        let input = create_test_input_request();
        let mut request = BRequest::new(input);

        // Initial state
        assert_eq!(request.status, Status::RequestReceived);

        // Cancel the request
        request.cancel(&db).unwrap();
        assert_eq!(request.status, Status::Canceled);

        // Verify the request was saved to the database
        let retrieved: BRequest = db.read(&request.id).unwrap().unwrap();
        assert_eq!(retrieved.status, Status::Canceled);
    }

    #[test]
    fn test_brequest_finalize() {
        let db = setup_test_db();
        let input = create_test_input_request();
        let mut request = BRequest::new(input);

        // Initial state
        assert_eq!(request.status, Status::RequestReceived);

        // Finalize the request
        let token_contract = "0xfinalcontract";
        let token_id = "999";
        request.finalize(&db, token_contract, token_id).unwrap();

        // Check that the request was updated correctly
        assert_eq!(request.status, Status::Completed);
        assert_eq!(
            request.output.detination_contract_id_or_mint,
            token_contract
        );
        assert_eq!(request.output.detination_token_id_or_account, token_id);

        // Verify the request was saved to the database
        let retrieved: BRequest = db.read(&request.id).unwrap().unwrap();
        assert_eq!(retrieved.status, Status::Completed);
        assert_eq!(
            retrieved.output.detination_contract_id_or_mint,
            token_contract
        );
        assert_eq!(retrieved.output.detination_token_id_or_account, token_id);

        // Verify the request was added to completed requests
        let completed = completed_requests(&db).unwrap();
        assert!(completed.contains(&request.id));
    }

    #[test]
    fn test_brequest_add_tx() {
        let db = setup_test_db();
        let input = create_test_input_request();
        let mut request = BRequest::new(input);

        // Initial state
        assert!(request.tx_hashes.is_empty());

        // Add a transaction
        let tx_hash = "0xtx123";
        request.add_tx(tx_hash, &db).unwrap();
        assert_eq!(request.tx_hashes.len(), 1);
        assert_eq!(request.tx_hashes[0], tx_hash);

        // Add another transaction
        let tx_hash2 = "0xtx456";
        request.add_tx(tx_hash2, &db).unwrap();
        assert_eq!(request.tx_hashes.len(), 2);
        assert_eq!(request.tx_hashes[0], tx_hash);
        assert_eq!(request.tx_hashes[1], tx_hash2);

        // Verify the request was saved to the database
        let retrieved: BRequest = db.read(&request.id).unwrap().unwrap();
        assert_eq!(retrieved.tx_hashes.len(), 2);
        assert_eq!(retrieved.tx_hashes[0], tx_hash);
        assert_eq!(retrieved.tx_hashes[1], tx_hash2);
    }

    #[test]
    fn test_solana_input_request_conversion() {
        let solana_input = SolanaInputRequest {
            token_mint: "mint123".to_string(),
            token_account: "account456".to_string(),
            origin_network: Chains::SOLANA,
            destination_account: "dest789".to_string(),
        };

        let input_request: InputRequest = solana_input.clone().into();

        assert_eq!(input_request.contract_or_mint, solana_input.token_mint);
        assert_eq!(input_request.token_id, "");
        assert_eq!(input_request.token_owner, solana_input.token_account);
        assert_eq!(input_request.origin_network, solana_input.origin_network);
        assert_eq!(
            input_request.destination_account,
            solana_input.destination_account
        );
    }

    #[test]
    fn test_evm_input_request_conversion() {
        let evm_input = EVMInputRequest {
            token_contract: "contract123".to_string(),
            token_id: "token456".to_string(),
            token_owner: "owner789".to_string(),
            origin_network: Chains::EVM,
            destination_account: "dest012".to_string(),
        };

        let input_request: InputRequest = evm_input.clone().into();

        assert_eq!(input_request.contract_or_mint, evm_input.token_contract);
        assert_eq!(input_request.token_id, evm_input.token_id);
        assert_eq!(input_request.token_owner, evm_input.token_owner);
        assert_eq!(input_request.origin_network, evm_input.origin_network);
        assert_eq!(
            input_request.destination_account,
            evm_input.destination_account
        );
    }

    #[test]
    fn test_tx_message_types() {
        // Test MessageMint
        let mint_data = MessageMint {
            request_id: "request123".to_string(),
            token_metadata: "metadata456".to_string(),
        };

        // Test MessageNewRequest
        let request_data = MessageNewRequest {
            token_contract: "contract123".to_string(),
            token_owner: "owner456".to_string(),
            token_id: "token789".to_string(),
            request_id: "request123".to_string(),
        };

        // Test TxMessage with Mint function
        let tx_message_mint = TxMessage {
            accion: Function::Mint,
            mint_data: Some(mint_data.clone()),
            request_data: None,
        };

        // Test TxMessage with NewRequest function
        let tx_message_request = TxMessage {
            accion: Function::NewRequest,
            mint_data: None,
            request_data: Some(request_data.clone()),
        };

        // Verify the data is stored correctly
        match tx_message_mint.accion {
            Function::Mint => {
                let mint_data = tx_message_mint.mint_data.unwrap();
                assert_eq!(mint_data.request_id, "request123");
                assert_eq!(mint_data.token_metadata, "metadata456");
            }
            _ => panic!("Expected Mint function"),
        }

        match tx_message_request.accion {
            Function::NewRequest => {
                let request_data = tx_message_request.request_data.unwrap();
                assert_eq!(request_data.token_contract, "contract123");
                assert_eq!(request_data.token_owner, "owner456");
                assert_eq!(request_data.token_id, "token789");
                assert_eq!(request_data.request_id, "request123");
            }
            _ => panic!("Expected NewRequest function"),
        }
    }
}
