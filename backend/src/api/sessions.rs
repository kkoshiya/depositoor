use alloy::eips::eip7702::{Authorization, SignedAuthorization};
use alloy::network::{TransactionBuilder, TransactionBuilder7702};
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol;
use alloy::sol_types::SolCall;
use axum::extract::{Json, Path, State};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::chains;
use crate::error::AppError;
use crate::types::{RegisterSessionRequest, RegisterSessionResponse, SessionStatus};

sol! {
    interface IDepositoorDelegate {
        function sweep(address token, address to) external;
    }

    interface IERC7821 {
        function execute(bytes32 mode, bytes calldata executionData) external payable;
    }
}

pub async fn register(
    State(state): State<Arc<super::AppState>>,
    Json(req): Json<RegisterSessionRequest>,
) -> Result<Json<RegisterSessionResponse>, AppError> {
    let cctp_domain = chains::chain_id_to_cctp_domain(req.destination_chain as u64)
        .ok_or_else(|| AppError::BadRequest(format!(
            "unsupported destination chain: {}",
            req.destination_chain
        )))?;

    let session = crate::db::insert_session(&state.pool, &req, cctp_domain).await?;

    // TTL: 30 minutes from now
    let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(30)).timestamp();

    Ok(Json(RegisterSessionResponse {
        id: session.id,
        status: SessionStatus::Pending,
        expires_at,
    }))
}

pub async fn get_session(
    State(state): State<Arc<super::AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::types::DepositSession>, AppError> {
    let session = crate::db::get_session(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {id} not found")))?;

    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
pub struct RefundRequest {
    pub refund_address: String,
    pub rpc_url: String,
}

pub async fn refund(
    State(state): State<Arc<super::AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RefundRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let session = crate::db::get_session(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {id} not found")))?;

    let detected_token: Address = session
        .detected_token
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("no detected token".into()))?
        .parse()
        .map_err(|_| AppError::BadRequest("invalid token address".into()))?;

    let burner: Address = session.burner_address.parse()
        .map_err(|_| AppError::BadRequest("invalid burner address".into()))?;
    let refund_to: Address = req.refund_address.parse()
        .map_err(|_| AppError::BadRequest("invalid refund address".into()))?;

    // Reconstruct EIP-7702 auth
    let auth_json = &session.eip7702_auth;
    let impl_addr: Address = auth_json["address"].as_str()
        .ok_or_else(|| AppError::BadRequest("missing auth address".into()))?
        .parse().map_err(|_| AppError::BadRequest("bad auth address".into()))?;
    let auth = SignedAuthorization::new_unchecked(
        Authorization {
            chain_id: U256::from(auth_json["chainId"].as_u64().unwrap_or(0)),
            address: impl_addr,
            nonce: auth_json["nonce"].as_u64().unwrap_or(0),
        },
        auth_json["yParity"].as_u64().unwrap_or(0) as u8,
        U256::from_str_radix(
            auth_json["r"].as_str().unwrap_or("0x0").trim_start_matches("0x"), 16,
        ).unwrap_or(U256::ZERO),
        U256::from_str_radix(
            auth_json["s"].as_str().unwrap_or("0x0").trim_start_matches("0x"), 16,
        ).unwrap_or(U256::ZERO),
    );

    // Build sweep call
    let sweep_call = IDepositoorDelegate::sweepCall { token: detected_token, to: refund_to };

    // ERC-7821 batch mode
    let mut mode_bytes = [0u8; 32];
    mode_bytes[0] = 0x01;
    let execute_call = IERC7821::executeCall {
        mode: FixedBytes::from(mode_bytes),
        executionData: alloy::sol_types::SolValue::abi_encode(
            &vec![(burner, U256::ZERO, Bytes::from(sweep_call.abi_encode()))]
        ).into(),
    };

    let signer: PrivateKeySigner = state.config.relayer_private_key.parse()
        .map_err(|_| AppError::Internal("bad relayer key".into()))?;
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_http(req.rpc_url.parse()
            .map_err(|_| AppError::BadRequest("invalid rpc_url".into()))?);

    let tx = TransactionRequest::default()
        .with_to(burner)
        .with_gas_limit(500_000)
        .with_authorization_list(vec![auth])
        .with_input(Bytes::from(execute_call.abi_encode()));

    let receipt = provider.send_transaction(tx).await
        .map_err(|e| AppError::Internal(format!("send tx failed: {e}")))?
        .get_receipt().await
        .map_err(|e| AppError::Internal(format!("receipt failed: {e}")))?;

    let tx_hash = format!("{:#x}", receipt.transaction_hash);

    if !receipt.status() {
        return Err(AppError::Internal(format!("refund tx reverted: {tx_hash}")));
    }

    tracing::info!(session_id = %id, tx = %tx_hash, refund_to = %req.refund_address, "refund sent");

    Ok(Json(serde_json::json!({
        "tx_hash": tx_hash,
        "refund_address": req.refund_address,
        "token": session.detected_token,
    })))
}
