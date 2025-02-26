use std::{str::FromStr, sync::Arc};

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey, signature::Keypair};
use spl_token::state::{Account, Mint};
use spl_token_client::{
    client::{ProgramClient, ProgramRpcClient, ProgramRpcClientSendTransaction},
    token::{TokenError, TokenResult},
};
use spl_token::ID;


pub async fn get_mint_info(
    client: Arc<RpcClient>,
    _keypair: Arc<Keypair>,
    address: &Pubkey,
) -> TokenResult<Mint> {
    let program_client = Arc::new(ProgramRpcClient::new(
        client.clone(),
        ProgramRpcClientSendTransaction,
    ));
    let account = program_client
        .get_account(*address)
        .await
        .map_err(TokenError::Client)?
        .ok_or(TokenError::AccountNotFound)?;

    if account.owner != ID {
        return Err(TokenError::AccountInvalidOwner);
    }

    let mint_result = Mint::unpack(&account.data).map_err(Into::into);
    let decimals: Option<u8> = None;
    if let (Ok(mint), Some(decimals)) = (&mint_result, decimals) {
        if decimals != mint.decimals {
            return Err(TokenError::InvalidDecimals);
        }
    }

    mint_result
}