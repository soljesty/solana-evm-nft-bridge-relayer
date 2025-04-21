# Bridge Relayer
Relayer for the cross-chain NFT bridge between Ethereum Virtual Machine (EVM) and Solana blockchain networks.

## Table of Contents
- [Overview](#overview)
- [Architecture](#architecture)
- [Flow](#flow)
- [Components](#components)
- [Technical Implementation](#technical-implementation)
- [Configuration](#configuration)
- [Installation Guide](#installation-guide)
- [Development](#development)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)
- [FAQ](#faq)
- [License](#license)


## Overview
Bridge-Relayer facilitates the transfer of NFTs between Solana and EVM-compatible blockchains. It is a service that listens for events on both chains and facilitates the transfer of assets between them.

## Architecture
The bridge consists of several components:

1. **API Server**: Provides HTTP endpoints to initiate bridge transfers and query their status.
2. **Request Processor**: Handles the lifecycle of bridge requests from initiation to completion.
3. **Solana Client**: Monitors Solana blockchain for bridge events and processes token transfers from Solana.
4. **EVM Client**: Monitors EVM-compatible blockchains for bridge events and processes token transfers from EVM chains.
5. **Storage**: Persistent storage for bridge requests and their statuses.

## Flow
1. User initiates a transfer request via the API
2. The bridge validates the request and locks the token on the source chain
3. The bridge monitors for events confirming the token has been locked
4. Once confirmed, the bridge mints the token on the destination chain
5. Bridge monitors for events confirming the token’s creation.
6. The request is marked as completed

### Detailed Flow
#### Solana to EVM Transfer
1. User calls `/bridge/solana-to-evm` endpoint with token mint, token account, and destination EVM address
2. Bridge creates a request and initiates a Solana transaction to lock the token
3. Bridge listens for `NewRequestEvent` from the Solana program
4. When event is detected, bridge verifies token ownership and updates request status
5. Bridge retrieves token metadata from Solana
6. Bridge mints a new token on the EVM chain with the same metadata
7. Bridge listens for `TokenMinted` event from the EVM contract
8. When event is detected, bridge updates request status to completed

#### EVM to Solana Transfer
1. User calls `/bridge/evm-to-solana` endpoint with token contract, token ID, token owner, and destination Solana address
2. Bridge creates a request and initiates an EVM transaction to lock the token
3. Bridge listens for `NewRequest` event from the EVM contract
4. When event is detected, bridge verifies token ownership and updates request status
5. Bridge retrieves token metadata from EVM
6. Bridge mints a new token on Solana with the same metadata
7. Bridge listens for `TokenMintedEvent` from the Solana program
8. When event is detected, bridge updates request status to completed

## Components

### API (`crates/api`)
Provides HTTP endpoints for interacting with the bridge:
- `/bridge/evm-to-solana`: Initiate a transfer from EVM to Solana
- `/bridge/solana-to-evm`: Initiate a transfer from Solana to EVM
- `/bridge/pending-requests`: Get a list of pending transfer requests
- `/bridge/completed-requests`: Get a list of completed transfer requests
- `/bridge/requests/{id}`: Get details about a specific request

#### API Request Format
For Solana to EVM transfers:
```json
{
  "token_mint": "Solana token mint address",
  "token_account": "User's token account address",
  "origin_network": "SOLANA",
  "destination_account": "Destination EVM address"
}
```

For EVM to Solana transfers:
```json
{
  "token_contract": "EVM token contract address",
  "token_id": "Token ID",
  "token_owner": "Token owner's EVM address",
  "origin_network": "EVM",
  "destination_account": "Destination Solana address"
}
```

### Solana Client (`crates/solana`)
Handles interactions with the Solana blockchain:
- Monitors for bridge events using Solana's WebSocket API
- Processes token transfers from Solana to EVM
- Mints tokens on Solana when transferred from EVM
- Verifies token ownership and metadata

#### Solana Events
The bridge listens for two main events from the Solana program:
1. `NewRequestEvent`: Triggered when a user initiates a transfer from Solana
2. `TokenMintedEvent`: Triggered when a token is minted on Solana

### EVM Client (`crates/evm`)
Handles interactions with EVM-compatible blockchains:
- Monitors for bridge events using EVM's WebSocket API
- Processes token transfers from EVM to Solana
- Mints tokens on EVM when transferred from Solana
- Verifies token ownership and metadata

#### EVM Events
The bridge listens for two main events from the EVM contract:
1. `NewRequest`: Triggered when a user initiates a transfer from EVM
2. `TokenMinted`: Triggered when a token is minted on EVM

### Storage (`crates/storage`)
Provides persistent storage for bridge requests and their statuses using RocksDB:
- Stores bridge requests with their current status
- Maintains lists of pending and completed requests
- Provides efficient lookup for request data

### Requests (`crates/requests`)
Manages the lifecycle of bridge requests:
- Creates new requests
- Updates request statuses
- Processes pending requests
- Handles error recovery for failed requests

#### Request States
A bridge request can be in one of the following states:
1. `RequestReceived`: Initial state when a request is created
2. `TokenReceived`: Token has been locked on the source chain
3. `TokenMinted`: Token has been minted on the destination chain
4. `Completed`: Transfer has been completed successfully
5. `Canceled`: Transfer has been canceled due to an error

### Types (`crates/types`)
Defines common data structures used throughout the bridge:
- `BRequest`: Bridge request data structure
- `InputRequest`: Input data for creating a bridge request
- `Status`: Enum representing the status of a bridge request
- `Chains`: Enum representing the supported blockchains
- `TxMessage`: Message structure for inter-component communication

## Technical Implementation

### Inter-Component Communication
The bridge uses Tokio channels for communication between components:
- `tx_evm` and `rx_evm`: Channels for sending messages to the EVM processor
- `tx_sol` and `rx_sol`: Channels for sending messages to the Solana processor

### Asynchronous Processing
The bridge uses Tokio for asynchronous processing:
- Each component runs in its own task
- Event listeners run continuously in the background
- Request processing is handled asynchronously

### Error Handling and Recovery
The bridge includes mechanisms for error handling and recovery:
- Failed requests are retried automatically
- Pending requests are processed on startup
- Requests can be canceled if they cannot be completed

## Configuration
The bridge is configured using environment variables:
- `DB_PATH`: Path to the RocksDB database
- `PORT`: API Port
- `EVM_RPC`: RPC URL for the EVM blockchain
- `EVM_WS`: WebSocket URL for the EVM blockchain
- `EVM_PK`: Private key for the EVM wallet
- `EVM_BRIDGE_CONTRACT`: Address of the bridge contract on the EVM blockchain
- `SOLANA_WALLET`: Path to the Solana wallet keypair
- `SOLANA_RPC`: RPC URL for the Solana blockchain
- `SOLANA_WS`: WebSocket URL for the Solana blockchain
- `SOLANA_BRIDGE_PROGRAM`: Address of the bridge program on Solana
- `SOLANA_BRIDGE_ACCOUNT`: Address of the bridge account on Solana


## Installation Guide

### Prerequisites
- Rust 1.80+ and Cargo
- RocksDB dependencies: `librocksdb-dev`, `libclang-dev`

### Step-by-Step Installation
1. Clone the repository:
   ```bash
   git clone https://github.com/soljesty/solana-evm-nft-bridge-relayer.git
   cd solana-evm-nft-bridge-relayer
   ```

2. Install dependencies:
   ```bash
   sudo apt-get update
   sudo apt-get install -y librocksdb-dev libclang-dev
   ```

3. Create a `.env` file based on the example:
   ```bash
   cp .env.example .env
   ```
   Edit .env with your configuration

### Logs and Debugging
- Set the `RUST_LOG` environment variable to control log levels:
  ```bash
  export RUST_LOG=info
  ```
   
### Start the Bridge
1. Start the bridge:
    ```bash
    cargo run
    ```

### Build release
1. Build the project: 
    ```bash
    cargo build --release
    ```
2. Run the bridge:
    ```bash
    ./target/release/Bridge_Relayer
    ```


## Development

### Project Structure
The project is organized as a Rust workspace with multiple crates:
- `bin/bridge_relayer`: Main executable
- `crates/api`: API server
- `crates/requests`: Request processing
- `crates/evm`: EVM client
- `crates/solana`: Solana client
- `crates/storage`: Storage layer
- `crates/types`: Common data types

### Testing
The project includes unit tests for each component (more to be added):
- Run tests with `cargo test`

## Security Considerations
- Private keys are stored in environment variables and should be kept secure
- The bridge uses secure RPC connections to the blockchains
- Token ownership is verified before processing transfers
- Error handling prevents double-spending and other security issues

This codebase is provided as-is. Users should perform their own security audits before using in production.

## Troubleshooting

### Common Issues

#### Connection Errors
- **Solana RPC Connection Failures**: Ensure your Solana RPC endpoint is correct and accessible. Try using a different RPC provider if issues persist.
- **EVM WebSocket Disconnects**: The bridge automatically attempts to reconnect when WebSocket connections drop. Check your network stability and EVM node health.

#### Transaction Failures
- **Insufficient Gas**: Ensure the EVM wallet has enough funds for gas fees.
- **Solana Transaction Errors**: Check that the Solana wallet has enough SOL for transaction fees.


## FAQ

### General Questions

**Q: Is the bridge bidirectional?**  
A: Yes, the bridge supports transfers in both directions: from Solana to EVM and from EVM to Solana.

**Q: Which EVM chains are supported?**  
A: The bridge is designed to work with any EVM-compatible blockchain, including Ethereum, Polygon, Binance Smart Chain, and others. You'll need to deploy the bridge contract on your target EVM chain.

**Q: Can I transfer any NFT?**  
A: The bridge supports transfer of NFTs that conform to the standard token interfaces on each chain (e.g., ERC-721 on EVM chains and SPL tokens on Solana).

### Technical Questions

**Q: How does the bridge handle network fees?**  
A: The bridge operator pays for the gas fees on the destination chain. Users only pay for the transaction fees on the source chain when initiating a transfer.

**Q: Is the bridge custodial?**  
A: Yes, the bridge takes custody of tokens during the transfer process. Tokens are locked in the bridge contract/program on the source chain.

**Q: How are token attributes preserved across chains?**  
A: The bridge transfers metadata along with the token, ensuring that attributes, images when minted on the destination chain.


## License
GNU License - See [LICENSE](./LICENSE) file for details.
