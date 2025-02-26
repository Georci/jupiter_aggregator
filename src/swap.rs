use std::sync::Arc;
use anyhow::Ok;
use crate::*;
use solana_sdk::{pubkey::Pubkey, signer::keypair, transaction::VersionedTransaction};
use spl_token::*;
use bs58;
use {
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        hash::Hash,
        pubkey,
        signature::{read_keypair_file, Keypair, Signer},
    },
    spl_token::{amount_to_ui_amount, ui_amount_to_amount},
};
use spl_associated_token_account::{
    get_associated_token_address,
};
use crate::utils::get_mint_info;


pub async fn swap(
    token_in: Pubkey,
    token_out: Pubkey,
    amount_in: u64,
    slippage_bps: u64,
) -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let keypair_str = env::var("PRIVATE_KEY").unwrap();
    let keypair = Arc::from(Keypair::from_base58_string(&keypair_str));

    let rpc_client = Arc::from(RpcClient::new_with_commitment(
        env::var("SOLANA_MAINNET_RPC_URL").unwrap(),
        CommitmentConfig::confirmed(),
    ));

    let mut token_in_decimal = 0;
    let mut token_out_decimal = 0;

    if token_in == native_mint::ID {
        token_in_decimal = 9 as u8;
        token_out_decimal = get_mint_info(rpc_client.clone(), keypair.clone(), &token_out)
        .await?.decimals;
    } else if token_out == native_mint::ID{
        token_out_decimal = 9 as u8;
        token_in_decimal = get_mint_info(rpc_client.clone(), keypair.clone(), &token_in)
            .await?.decimals;
    }else {
        token_in_decimal = get_mint_info(rpc_client.clone(), keypair.clone(), &token_in)
            .await?.decimals;
        token_out_decimal = get_mint_info(rpc_client.clone(), keypair.clone(), &token_out)
            .await?.decimals;
    }

    println!("token_in_decimal: {}", token_in_decimal);
    println!("token_out_decimal: {}", token_out_decimal);

    let token_out_ata_address =
        get_associated_token_address(&keypair.clone().pubkey(), &token_out);
    println!(
        "Pre-swap {} balance: {}",
        token_in,
        get_token_balance(rpc_client.clone(), keypair.clone(), token_in, token_in_decimal).await?
    );
    println!(
        "Pre-swap {} balance: {}",
        token_out,
        get_token_balance(rpc_client.clone(), keypair.clone(), token_out, token_out_decimal).await?
    );

    let only_direct_routes = false;
    let quotes = quote(
        token_in,
        token_out,
        amount_in,
        QuoteConfig {
            only_direct_routes,
            slippage_bps: Some(slippage_bps),
            ..QuoteConfig::default()
        },
    )
    .await?;

    let route = quotes.route_plan[0]
        .swap_info
        .label
        .clone()
        .unwrap_or_else(|| "Unknown DEX".to_string());
    println!(
        "Quote: {} :{} for {} :{} via {} (worst case with slippage: {}). Impact: {:.2}%",
        token_in,
        amount_to_ui_amount(quotes.in_amount, token_in_decimal),
        token_out,
        amount_to_ui_amount(quotes.out_amount, token_out_decimal),
        route,
        amount_to_ui_amount(quotes.other_amount_threshold, token_out_decimal),
        quotes.price_impact_pct * 100.
    );

    let request: SwapRequest = SwapRequest::new(keypair.pubkey(), quotes.clone());

    let Swap {
        mut swap_transaction,
        last_valid_block_height: _,
    } = crate::swap(request).await?;

    let recent_blockhash_for_swap: Hash = rpc_client.get_latest_blockhash().await?;
    swap_transaction
        .message
        .set_recent_blockhash(recent_blockhash_for_swap); // Updating to latest blockhash to not error out

    let swap_transaction = VersionedTransaction::try_new(swap_transaction.message, &[&keypair])?;
    println!(
        "Simulating swap transaction: {}",
        swap_transaction.signatures[0]
    );
    let response = rpc_client.simulate_transaction(&swap_transaction).await?;
    println!("Sending transaction: {}", swap_transaction.signatures[0]);

    let _ = rpc_client
        .send_and_confirm_transaction_with_spinner(&swap_transaction)
        .await?;

    println!(
        "Post-swap {} balance: {}",
        token_in,
        get_token_balance(rpc_client.clone(), keypair.clone(), token_in, token_in_decimal).await?
    );
    println!(
        "Post-swap {} balance: {}",
        token_out,
        get_token_balance(rpc_client.clone(), keypair.clone(), token_out, token_out_decimal).await?
    );

    Ok(())
}

pub async fn get_token_balance(rpc_client: Arc<RpcClient>, keypair: Arc<Keypair>, token: Pubkey, token_decimal: u8) -> anyhow::Result<f64> {
    if token == spl_token::native_mint::ID{
        Ok(amount_to_ui_amount(rpc_client.get_balance(&keypair.pubkey()).await?, token_decimal))
    } else{
        let token_out_ata_address =
            get_associated_token_address(&keypair.clone().pubkey(), &token);

        Ok(amount_to_ui_amount(
            rpc_client
                .get_token_account_balance(&token_out_ata_address)
                .await?
                .amount
                .parse::<u64>()?,
            token_decimal
        ))
    }
}


#[cfg(test)]
mod tests{
    use super::*;
    #[tokio::test]
    pub async fn test_swap() {
        let token_in = pubkey!("6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN");
        // let token_in = pubkey!("So11111111111111111111111111111111111111112");
        let token_out = pubkey!("So11111111111111111111111111111111111111112");
        // let token_out = pubkey!("6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN");
        let amount_in: u64 = 60_000;
        let slippage_bps: u64 = 500;

        let res = swap(token_in, token_out, amount_in, slippage_bps).await;
    }
}
