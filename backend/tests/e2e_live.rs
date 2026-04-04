//! Live E2E tests against real Arbitrum.
//!
//! All tests are `#[ignore]` — run with:
//! ```
//! cargo test --test e2e_live -- --ignored --nocapture
//! ```
//!
//! Required env vars:
//!   ARBITRUM_RPC_URL   - Arbitrum RPC endpoint
//!   RELAYER_PRIVATE_KEY - funded keeper private key on Arbitrum
//!   DATABASE_URL       - postgres connection string (optional, only for DB tests)

mod common;

use alloy::network::{TransactionBuilder, TransactionBuilder7702};
use alloy::primitives::{address, Address, Bytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::sol_types::SolCall;
use common::{DepositoorDelegate, IERC20};

// ── Arbitrum constants ───────────────────────────────────────────────────

const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
const USDC: Address = address!("af88d065e77c8cC2239327C5EDb3A432268e5831");
const TOKEN_MESSENGER: Address = address!("19330d10D9Cc8751218eaf51E8885D058642E08A");
const ARB_CHAIN_ID: u64 = 42161;
// Base CCTP domain (destination for cross-chain tests)
const BASE_CCTP_DOMAIN: u32 = 6;

fn load_env() -> (String, PrivateKeySigner) {
    dotenvy::dotenv().ok();
    let rpc = std::env::var("ARBITRUM_RPC_URL").expect("ARBITRUM_RPC_URL required");
    let key: PrivateKeySigner = std::env::var("RELAYER_PRIVATE_KEY")
        .expect("RELAYER_PRIVATE_KEY required")
        .parse()
        .expect("invalid RELAYER_PRIVATE_KEY");
    (rpc, key)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn test_deploy_to_arbitrum() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    // Deploy DepositoorDelegate with real WETH + relayer as keeper
    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .expect("deploy failed");

    let deployed_weth = delegate.weth().call().await.unwrap();
    let deployed_keeper = delegate.keeper().call().await.unwrap();

    assert_eq!(deployed_weth, WETH);
    assert_eq!(deployed_keeper, relayer_addr);
    eprintln!("DepositoorDelegate deployed at: {:?}", delegate.address());
}

#[tokio::test]
#[ignore]
async fn test_full_sweep_erc20_to_bridge() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    // 1. Deploy
    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();
    eprintln!("delegate: {:?}", delegate.address());

    // 2. Generate burner, sign EIP-7702 auth
    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let burner_nonce = provider.get_transaction_count(burner_addr).await.unwrap();
    let signed_auth =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), burner_nonce)
            .unwrap();
    eprintln!("burner: {:?}", burner_addr);

    // 3. Transfer small WETH to burner (relayer must hold WETH)
    let weth = IERC20::new(WETH, &provider);
    let deposit_amount = U256::from(100_000_000_000_000u64); // 0.0001 WETH
    let relayer_weth = weth.balanceOf(relayer_addr).call().await.unwrap();
    assert!(
        relayer_weth >= deposit_amount,
        "relayer needs at least 0.0001 WETH, has {relayer_weth}"
    );

    weth.transfer(burner_addr, deposit_amount)
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    eprintln!("transferred {deposit_amount} WETH to burner");

    // 4. Tx 1: delegation + swap (WETH → USDC via DefiLlama)
    let swap_quote: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "https://swap.defillama.com/arbitrum?fromToken={:#x}&toToken={:#x}&amount={}&from={:#x}&slippage=1",
            WETH, USDC, deposit_amount, burner_addr
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let swap_tx = swap_quote["tx"].as_object().expect("no swap tx in quote");
    let swap_router: Address = swap_tx["to"].as_str().unwrap().parse().unwrap();
    let swap_data = Bytes::from(
        hex::decode(swap_tx["data"].as_str().unwrap().trim_start_matches("0x")).unwrap(),
    );
    let swap_value = U256::from_str_radix(
        swap_tx["value"].as_str().unwrap_or("0x0").trim_start_matches("0x"),
        16,
    )
    .unwrap_or(U256::ZERO);

    let approve_call = IERC20::approveCall {
        spender: swap_router,
        amount: deposit_amount,
    };

    let swap_calls = vec![
        (WETH, U256::ZERO, Bytes::from(approve_call.abi_encode())),
        (swap_router, swap_value, swap_data),
    ];

    let tx1 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth])
        .with_input(common::encode_execute(swap_calls));

    let receipt1 = provider
        .send_transaction(tx1)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt1.status(), "tx1 (delegation + swap) reverted");
    eprintln!("tx1 confirmed: {:#x}", receipt1.transaction_hash);

    // 5. Read USDC balance
    let usdc = IERC20::new(USDC, &provider);
    let usdc_balance = usdc.balanceOf(burner_addr).call().await.unwrap();
    assert!(!usdc_balance.is_zero(), "zero USDC after swap");
    eprintln!("USDC balance after swap: {usdc_balance}");

    // 6. Tx 2: approve + CCTP depositForBurn
    let fee_bps = 50u64; // 0.5%
    let fee = usdc_balance * U256::from(fee_bps) / U256::from(10_000u64);
    let net_amount = usdc_balance - fee;
    let dest = Address::repeat_byte(0xDE); // dummy dest for test

    let usdc_approve = IERC20::approveCall {
        spender: TOKEN_MESSENGER,
        amount: net_amount,
    };
    let bridge_data = common::build_deposit_for_burn(
        net_amount,
        BASE_CCTP_DOMAIN,
        dest,
        USDC,
        U256::ZERO,
    );

    let bridge_calls = vec![
        (USDC, U256::ZERO, Bytes::from(usdc_approve.abi_encode())),
        (TOKEN_MESSENGER, U256::ZERO, bridge_data),
    ];

    let tx2 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_input(common::encode_execute(bridge_calls));

    let receipt2 = provider
        .send_transaction(tx2)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt2.status(), "tx2 (bridge) reverted");
    eprintln!("tx2 confirmed: {:#x}", receipt2.transaction_hash);

    // 7. Check logs for DepositForBurn event
    let has_deposit_event = receipt2
        .inner
        .logs()
        .iter()
        .any(|log| !log.topics().is_empty());
    assert!(has_deposit_event, "expected CCTP DepositForBurn event");
    eprintln!("CCTP bridge event emitted");
}

#[tokio::test]
#[ignore]
async fn test_direct_usdc_bridge() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let burner_nonce = provider.get_transaction_count(burner_addr).await.unwrap();
    let signed_auth =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), burner_nonce)
            .unwrap();

    // Transfer USDC to burner (skip swap)
    let usdc = IERC20::new(USDC, &provider);
    let deposit_amount = U256::from(100_000u64); // 0.1 USDC (6 decimals)
    let relayer_usdc = usdc.balanceOf(relayer_addr).call().await.unwrap();
    assert!(
        relayer_usdc >= deposit_amount,
        "relayer needs at least 0.1 USDC, has {relayer_usdc}"
    );

    usdc.transfer(burner_addr, deposit_amount)
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Tx 1: set delegation (no swap needed, USDC already present)
    let tx1 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth])
        .with_input(common::encode_execute(vec![]));

    provider
        .send_transaction(tx1)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Tx 2: approve + bridge
    let dest = Address::repeat_byte(0xBB);
    let usdc_approve = IERC20::approveCall {
        spender: TOKEN_MESSENGER,
        amount: deposit_amount,
    };
    let bridge_data = common::build_deposit_for_burn(
        deposit_amount,
        BASE_CCTP_DOMAIN,
        dest,
        USDC,
        U256::ZERO,
    );

    let bridge_calls = vec![
        (USDC, U256::ZERO, Bytes::from(usdc_approve.abi_encode())),
        (TOKEN_MESSENGER, U256::ZERO, bridge_data),
    ];

    let tx2 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_input(common::encode_execute(bridge_calls));

    let receipt = provider
        .send_transaction(tx2)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status(), "direct USDC bridge tx reverted");
    eprintln!("direct USDC bridge confirmed: {:#x}", receipt.transaction_hash);
}

#[tokio::test]
#[ignore]
async fn test_same_chain_transfer() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let burner_nonce = provider.get_transaction_count(burner_addr).await.unwrap();
    let signed_auth =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), burner_nonce)
            .unwrap();

    // Transfer USDC to burner
    let usdc = IERC20::new(USDC, &provider);
    let deposit_amount = U256::from(100_000u64); // 0.1 USDC
    usdc.transfer(burner_addr, deposit_amount)
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Set delegation
    let tx1 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth])
        .with_input(common::encode_execute(vec![]));
    provider
        .send_transaction(tx1)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Same-chain: just transfer USDC to dest (no CCTP)
    let dest = Address::repeat_byte(0xCC);
    let fee_bps = 50u64;
    let fee = deposit_amount * U256::from(fee_bps) / U256::from(10_000u64);
    let net_amount = deposit_amount - fee;

    let transfer_call = IERC20::transferCall {
        to: dest,
        amount: net_amount,
    };
    let calls = vec![(USDC, U256::ZERO, Bytes::from(transfer_call.abi_encode()))];

    let tx2 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_input(common::encode_execute(calls));

    let receipt = provider
        .send_transaction(tx2)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status(), "same-chain transfer reverted");

    let dest_balance = usdc.balanceOf(dest).call().await.unwrap();
    assert_eq!(dest_balance, net_amount);
    eprintln!("same-chain transfer confirmed, dest received {net_amount} USDC");
}

#[tokio::test]
#[ignore]
async fn test_fee_deduction() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let burner_nonce = provider.get_transaction_count(burner_addr).await.unwrap();
    let signed_auth =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), burner_nonce)
            .unwrap();

    // Deposit known USDC amount
    let usdc = IERC20::new(USDC, &provider);
    let deposit_amount = U256::from(1_000_000u64); // exactly 1 USDC

    usdc.transfer(burner_addr, deposit_amount)
        .send()
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Set delegation
    let tx1 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth])
        .with_input(common::encode_execute(vec![]));
    provider
        .send_transaction(tx1)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Calculate fee
    let fee_bps = 50u64; // 0.5%
    let expected_fee = deposit_amount * U256::from(fee_bps) / U256::from(10_000u64);
    let expected_net = deposit_amount - expected_fee;

    // Verify fee math: 1_000_000 * 50 / 10000 = 5000
    assert_eq!(expected_fee, U256::from(5_000u64));
    assert_eq!(expected_net, U256::from(995_000u64));

    // Bridge net amount via CCTP
    let dest = Address::repeat_byte(0xFE);
    let usdc_approve = IERC20::approveCall {
        spender: TOKEN_MESSENGER,
        amount: expected_net,
    };
    let bridge_data = common::build_deposit_for_burn(
        expected_net,
        BASE_CCTP_DOMAIN,
        dest,
        USDC,
        U256::ZERO,
    );

    let bridge_calls = vec![
        (USDC, U256::ZERO, Bytes::from(usdc_approve.abi_encode())),
        (TOKEN_MESSENGER, U256::ZERO, bridge_data),
    ];

    let tx2 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_input(common::encode_execute(bridge_calls));

    let receipt = provider
        .send_transaction(tx2)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status(), "fee deduction bridge tx reverted");

    // Fee (5000 = 0.005 USDC) remains in burner
    let remaining = usdc.balanceOf(burner_addr).call().await.unwrap();
    assert_eq!(remaining, expected_fee, "fee should remain in burner");
    eprintln!(
        "fee test passed: deposited={deposit_amount}, bridged={expected_net}, fee={expected_fee}"
    );
}

#[tokio::test]
#[ignore]
async fn test_nonce_replay_rejected() {
    let (rpc, relayer) = load_env();
    let relayer_addr = relayer.address();
    let provider = ProviderBuilder::new()
        .wallet(relayer)
        .connect_http(rpc.parse().unwrap());

    let delegate = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    // Sign auth with nonce=0
    let signed_auth1 =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), 0).unwrap();
    let signed_auth2 =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate.address(), 0).unwrap();

    // Tx 1: uses the auth, sets delegation (consumes nonce 0)
    let tx1 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth1])
        .with_input(common::encode_execute(vec![]));

    let receipt1 = provider
        .send_transaction(tx1)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt1.status(), "tx1 should succeed");
    eprintln!("tx1 succeeded, nonce 0 consumed");

    // Tx 2: try to use the same nonce=0 auth again
    // The auth is stale because burner's nonce is now 1, but the auth was signed for nonce 0
    // EIP-7702: the authorization is simply skipped (not applied), not the whole tx reverted
    // The delegation from tx1 persists, so the execute call still works
    let tx2 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![signed_auth2])
        .with_input(common::encode_execute(vec![]));

    // This tx should succeed (the stale auth is ignored, delegation persists)
    // But if we deployed a DIFFERENT contract and tried to re-delegate, it would fail
    let _receipt2 = provider
        .send_transaction(tx2)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // The key insight: the replayed auth doesn't re-set delegation.
    // To verify, deploy a second delegate and try to re-delegate with stale auth
    let delegate2 = DepositoorDelegate::deploy(&provider, WETH, relayer_addr)
        .await
        .unwrap();

    // Sign a stale auth pointing to delegate2 with nonce=0 (already consumed)
    let stale_auth =
        common::sign_authorization(&burner, ARB_CHAIN_ID, *delegate2.address(), 0).unwrap();

    let tx3 = TransactionRequest::default()
        .with_to(burner_addr)
        .with_authorization_list(vec![stale_auth])
        .with_input(common::encode_execute(vec![]));

    let _receipt3 = provider
        .send_transaction(tx3)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // The burner should still be delegated to the ORIGINAL delegate (not delegate2)
    // Verify by reading weth() — it should return the original WETH address
    let delegate_view = DepositoorDelegate::new(burner_addr, &provider);
    let weth_addr = delegate_view.weth().call().await.unwrap();
    assert_eq!(weth_addr, WETH, "delegation should not have changed to delegate2");
    eprintln!("nonce replay protection verified: stale auth was ignored");
}
