mod common;

use alloy::primitives::{utils::parse_ether, Address, Bytes, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::sol_types::SolCall;
use common::{DepositoorDelegate, MockERC20, MockWETH};

#[tokio::test]
async fn test_receive_wraps_eth() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Send 1 ETH to burner — receive() should wrap it to WETH
    let one_eth = parse_ether("1").unwrap();
    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .value(one_eth);
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    let balance = mock_weth.balanceOf(burner_addr).call().await.unwrap();
    assert_eq!(balance, one_eth);
}

#[tokio::test]
async fn test_execute_batch_as_keeper() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let mock_erc20 = MockERC20::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let recipient = Address::repeat_byte(0x42);

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Mint tokens to the burner
    let amount = U256::from(1000u64);
    mock_erc20.mint(burner_addr, amount).send().await.unwrap().get_receipt().await.unwrap();

    // Build batch: approve + transferFrom via keeper
    let approve_call = MockERC20::approveCall {
        spender: keeper_addr,
        amount,
    };
    let transfer_call = MockERC20::transferCall {
        to: recipient,
        amount,
    };

    let calls = vec![
        (*mock_erc20.address(), U256::ZERO, Bytes::from(approve_call.abi_encode())),
        (*mock_erc20.address(), U256::ZERO, Bytes::from(transfer_call.abi_encode())),
    ];

    // Keeper calls execute on burner
    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .input(common::encode_execute(calls).into());
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    let recipient_balance = mock_erc20.balanceOf(recipient).call().await.unwrap();
    assert_eq!(recipient_balance, amount);
}

#[tokio::test]
async fn test_execute_rejects_unauthorized() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let random_signer: PrivateKeySigner = anvil.keys()[1].clone().into();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Random signer (not keeper, not burner) tries to call execute
    let random_provider = ProviderBuilder::new()
        .wallet(random_signer)
        .connect_http(anvil.endpoint_url());

    let calls = vec![];
    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .input(common::encode_execute(calls).into());

    let result = random_provider.send_transaction(tx).await;

    // Anvil rejects the tx at simulation (revert: "unauthorized")
    assert!(result.is_err(), "unauthorized caller should be rejected");
}

#[tokio::test]
async fn test_execute_self_call() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let mock_erc20 = MockERC20::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner: PrivateKeySigner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    // Fund burner with ETH for gas BEFORE delegation (otherwise receive() wraps it)
    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .value(parse_ether("1").unwrap());
    provider.send_transaction(tx).await.unwrap().get_receipt().await.unwrap();

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Mint tokens to burner
    let amount = U256::from(500u64);
    let recipient = Address::repeat_byte(0x99);
    mock_erc20.mint(burner_addr, amount).send().await.unwrap().get_receipt().await.unwrap();

    // Burner calls execute on itself (msg.sender == address(this))
    let burner_provider = ProviderBuilder::new()
        .wallet(burner)
        .connect_http(anvil.endpoint_url());

    let transfer_call = MockERC20::transferCall {
        to: recipient,
        amount,
    };
    let calls = vec![
        (*mock_erc20.address(), U256::ZERO, Bytes::from(transfer_call.abi_encode())),
    ];

    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .input(common::encode_execute(calls).into());
    burner_provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    let recipient_balance = mock_erc20.balanceOf(recipient).call().await.unwrap();
    assert_eq!(recipient_balance, amount);
}

#[tokio::test]
async fn test_receive_zero_value() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Send 0 ETH — should not revert
    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .value(U256::ZERO);
    let result = provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await;
    assert!(result.is_ok(), "sending 0 ETH should not revert");
}

#[tokio::test]
async fn test_multiple_deposits_accumulate() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let mock_erc20 = MockERC20::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Two separate ERC20 mints to burner
    let amount1 = U256::from(100u64);
    let amount2 = U256::from(250u64);
    mock_erc20.mint(burner_addr, amount1).send().await.unwrap().get_receipt().await.unwrap();
    mock_erc20.mint(burner_addr, amount2).send().await.unwrap().get_receipt().await.unwrap();

    let balance = mock_erc20.balanceOf(burner_addr).call().await.unwrap();
    assert_eq!(balance, amount1 + amount2);
}

#[tokio::test]
async fn test_dust_amount_execute() {
    let anvil = common::spawn_anvil();
    let keeper: PrivateKeySigner = anvil.keys()[0].clone().into();
    let keeper_addr = keeper.address();
    let provider = ProviderBuilder::new()
        .wallet(keeper)
        .connect_http(anvil.endpoint_url());

    let mock_weth = MockWETH::deploy(&provider).await.unwrap();
    let mock_erc20 = MockERC20::deploy(&provider).await.unwrap();
    let delegate = DepositoorDelegate::deploy(&provider, *mock_weth.address(), keeper_addr)
        .await
        .unwrap();

    let burner = PrivateKeySigner::random();
    let burner_addr = burner.address();
    let recipient = Address::repeat_byte(0x01);

    common::set_delegation(&provider, &burner, *delegate.address())
        .await
        .unwrap();

    // Mint 1 wei of ERC20
    let dust = U256::from(1u64);
    mock_erc20.mint(burner_addr, dust).send().await.unwrap().get_receipt().await.unwrap();

    // Execute transfer of 1 wei
    let transfer_call = MockERC20::transferCall {
        to: recipient,
        amount: dust,
    };
    let calls = vec![
        (*mock_erc20.address(), U256::ZERO, Bytes::from(transfer_call.abi_encode())),
    ];

    let tx = alloy::rpc::types::TransactionRequest::default()
        .to(burner_addr)
        .input(common::encode_execute(calls).into());
    provider
        .send_transaction(tx)
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    let recipient_balance = mock_erc20.balanceOf(recipient).call().await.unwrap();
    assert_eq!(recipient_balance, dust);
}
