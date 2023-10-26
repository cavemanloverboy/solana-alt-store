use {
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::{CommitmentConfig, CommitmentLevel},
        pubkey::Pubkey,
    },
    std::iter::zip,
};

pub type AccountKeyData = (Pubkey, Vec<u8>);

pub fn load_address_lookup_tables(
    pubkeys: &[Pubkey],
) -> Result<Vec<AccountKeyData>, Box<dyn std::error::Error>> {
    // Create a new RPC client
    let rpc_client = RpcClient::new_with_commitment(
        "https://api.mainnet-beta.solana.com".to_string(),
        CommitmentConfig {
            commitment: CommitmentLevel::Finalized,
        },
    );

    // Chunk the RPC requests into max-account requests
    let mut result = Vec::with_capacity(pubkeys.len());
    for pubkeys in pubkeys.chunks(100) {
        let accounts = rpc_client.get_multiple_accounts(pubkeys)?;
        for (pubkey, maybe_account) in zip(pubkeys, accounts.into_iter()) {
            if let Some(account) = maybe_account {
                result.push((*pubkey, account.data));
            }
        }
    }

    Ok(result)
}
