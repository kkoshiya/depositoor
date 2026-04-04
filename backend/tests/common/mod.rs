#![allow(dead_code)]

use alloy::eips::eip7702::Authorization;
use alloy::network::{TransactionBuilder, TransactionBuilder7702};
use alloy::node_bindings::Anvil;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;
use alloy::sol;
use alloy::sol_types::SolCall;

// ── Contract ABIs + bytecodes ────────────────────────────────────────────

sol! {
    #[sol(rpc, bytecode = "0x60c03461008757601f6105d538819003918201601f19168301916001600160401b0383118484101761008b57808492604094855283398101031261008757610052602061004b8361009f565b920161009f565b9060805260a05260405161052190816100b4823960805181818160530152610261015260a05181818161022001526103fe0152f35b5f80fd5b634e487b7160e01b5f52604160045260245ffd5b51906001600160a01b03821682036100875756fe6080806040526004361015610116575b50361561004c575f3560e01c63bc197c81811463f23a6e6182141763150b7a0282141761004357633c10b94e5f526004601cfd5b6020526020603cf35b5f808080347f00000000000000000000000000000000000000000000000000000000000000005af13d15610111573d67ffffffffffffffff81116100fd5760405190601f8101601f19908116603f0116820167ffffffffffffffff8111838210176100fd5760405281525f60203d92013e5b156100c557005b60405162461bcd60e51b815260206004820152601060248201526f15d15512081ddc985c0819985a5b195960821b6044820152606490fd5b634e487b7160e01b5f52604160045260245ffd5b6100be565b5f3560e01c9081633fc8cef31461024f57508063aced16611461020b578063d03c7914146101ac5763e9ae5c531461014e575f61000f565b60403660031901126101a85760243567ffffffffffffffff81116101a857366023820112156101a857806004013567ffffffffffffffff81116101a85736602482840101116101a85760246101a69201600435610290565b005b5f80fd5b346101a85760203660031901126101a857602061020160043569ffff00000000ffffffff9060b01c166901000000000078210002600160481b82146901000000000078210001831460011b1791146003021790565b1515604051908152f35b346101a8575f3660031901126101a8576040517f00000000000000000000000000000000000000000000000000000000000000006001600160a01b03168152602090f35b346101a8575f3660031901126101a8577f00000000000000000000000000000000000000000000000000000000000000006001600160a01b03168152602090f35b916102d18369ffff00000000ffffffff9060b01c166901000000000078210002600160481b82146901000000000078210001831460011b1791146003021790565b926003841461032f57505036821561032257610309925f9280359160028383019360401191141161030b575b506020813591016103b6565b565b91509150806020013501602081019035915f6102fd565b637f1812755f526004601cfd5b90919250600360b01b189082359182840192828435950192602084818860051b880101119110179060401c176103a9575f5b84810361036f575050505050565b60208160051b850101358085019060208201359085604083850101119060401c176103a95760406103a1920184610290565b600101610361565b633995943b5f526004601cfd5b92909150156103fb5760405162461bcd60e51b81526020600482015260146024820152731bdc11185d18481b9bdd081cdd5c1c1bdc9d195960621b6044820152606490fd5b337f00000000000000000000000000000000000000000000000000000000000000006001600160a01b0316148015610470575b1561043c5761030991610479565b60405162461bcd60e51b815260206004820152600c60248201526b1d5b985d5d1a1bdc9a5e995960a21b6044820152606490fd5b5030331461042e565b91905f9181156104e55760015b156104d8575b5f60205f8560051b870135870160018060a01b0381351660408201358201803591826040519687930183376020389401359080153002175af1156104d05750610486565b3d5f823e3d90fd5b916001019181830361048c575b9250505056fea2646970667358221220f39151d1b0e842cb4597ce196ae6622e9c7e110868db3a6b39db09e368848bac64736f6c634300081c0033")]
    contract DepositoorDelegate {
        constructor(address _weth, address _keeper);
        function execute(bytes32 mode, bytes calldata executionData) external payable;
        function supportsExecutionMode(bytes32 mode) external view returns (bool);
        function weth() external view returns (address);
        function keeper() external view returns (address);
    }
}

sol! {
    #[sol(rpc, bytecode = "0x608080604052346015576101a8908161001a8239f35b5f80fdfe6080806040526004361015610034575b50361561001a575f80fd5b335f525f60205260405f20610030348254610165565b9055005b5f3560e01c90816370a082311461011c575063a9059cbb14610056575f61000f565b346101185760403660031901126101185761006f61014f565b60243590335f525f6020528160405f2054106100dc57335f525f60205260405f208054908382039182116100c8575560018060a01b03165f525f6020526100bb60405f20918254610165565b9055602060405160018152f35b634e487b7160e01b5f52601160045260245ffd5b60405162461bcd60e51b8152602060048201526014602482015273696e73756666696369656e742062616c616e636560601b6044820152606490fd5b5f80fd5b34610118576020366003190112610118576020906001600160a01b0361014061014f565b165f525f825260405f20548152f35b600435906001600160a01b038216820361011857565b919082018092116100c85756fea2646970667358221220c176acae225dcaca857884eac52b101f73190e378a93b37f4afa6b0f341186fc64736f6c634300081c0033")]
    contract MockWETH {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

sol! {
    #[sol(rpc, bytecode = "0x60808060405234601557610399908161001a8239f35b5f80fdfe60806040526004361015610011575f80fd5b5f3560e01c8063095ea7b31461027f57806323b872dd1461019957806340c10f191461015c57806370a0823114610125578063a9059cbb146100af5763dd62ed3e1461005b575f80fd5b346100ab5760403660031901126100ab576100746102c6565b61007c6102dc565b6001600160a01b039182165f908152600160209081526040808320949093168252928352819020549051908152f35b5f80fd5b346100ab5760403660031901126100ab576100c86102c6565b60243590335f525f6020526100e38260405f205410156102f2565b335f525f60205260405f206100f9838254610335565b905560018060a01b03165f525f60205261011860405f20918254610356565b9055602060405160018152f35b346100ab5760203660031901126100ab576001600160a01b036101466102c6565b165f525f602052602060405f2054604051908152f35b346100ab5760403660031901126100ab576001600160a01b0361017d6102c6565b165f525f60205260405f206101956024358254610356565b9055005b346100ab5760603660031901126100ab576101b26102c6565b6101ba6102dc565b6001600160a01b039091165f818152600160209081526040808320338452909152902054604435929190831161024157805f525f6020526102018360405f205410156102f2565b805f52600160205260405f2060018060a01b0333165f5260205260405f2061022a848254610335565b90555f525f60205260405f206100f9838254610335565b60405162461bcd60e51b8152602060048201526016602482015275696e73756666696369656e7420616c6c6f77616e636560501b6044820152606490fd5b346100ab5760403660031901126100ab576102986102c6565b335f52600160205260405f209060018060a01b03165f5260205260405f206024359055602060405160018152f35b600435906001600160a01b03821682036100ab57565b602435906001600160a01b03821682036100ab57565b156102f957565b60405162461bcd60e51b8152602060048201526014602482015273696e73756666696369656e742062616c616e636560601b6044820152606490fd5b9190820391821161034257565b634e487b7160e01b5f52601160045260245ffd5b919082018092116103425756fea26469706673582212200aaf06d2ddd59db5c86381ad8af63fb778554e81407f53bbeba600c5f74573c564736f6c634300081c0033")]
    contract MockERC20 {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function mint(address to, uint256 amount) external;
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
    }
}

// Standard ERC20 interface (for live token interactions)
sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

// Used for encoding execute calldata (mirrors backend's IERC7821)
sol! {
    interface IERC7821 {
        function execute(bytes32 mode, bytes calldata executionData) external payable;
    }
}

// CCTP V2 TokenMessenger
sol! {
    #[sol(rpc)]
    interface ITokenMessengerV2 {
        function depositForBurn(
            uint256 amount,
            uint32 destinationDomain,
            bytes32 mintRecipient,
            address burnToken,
            bytes32 destinationCaller,
            uint256 maxFee,
            uint32 minFinalityThreshold
        ) external returns (bytes32 nonce);
    }
}

/// Build CCTP V2 depositForBurn calldata
pub fn build_deposit_for_burn(
    amount: U256,
    destination_domain: u32,
    mint_recipient: Address,
    usdc_address: Address,
    max_fee: U256,
) -> Bytes {
    let mut recipient_bytes = [0u8; 32];
    recipient_bytes[12..].copy_from_slice(mint_recipient.as_slice());

    let call = ITokenMessengerV2::depositForBurnCall {
        amount,
        destinationDomain: destination_domain,
        mintRecipient: FixedBytes::from(recipient_bytes),
        burnToken: usdc_address,
        destinationCaller: FixedBytes::ZERO,
        maxFee: max_fee,
        minFinalityThreshold: 2000, // standard finality
    };

    Bytes::from(call.abi_encode())
}

/// Sign an EIP-7702 authorization (returns the SignedAuthorization without submitting)
pub fn sign_authorization(
    burner: &PrivateKeySigner,
    chain_id: u64,
    implementation: Address,
    nonce: u64,
) -> eyre::Result<alloy::eips::eip7702::SignedAuthorization> {
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: implementation,
        nonce,
    };
    let sig = burner.sign_hash_sync(&auth.signature_hash())?;
    Ok(alloy::eips::eip7702::SignedAuthorization::new_unchecked(
        auth,
        sig.v() as u8,
        sig.r(),
        sig.s(),
    ))
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Spawn Anvil with Prague hardfork (required for EIP-7702)
pub fn spawn_anvil() -> alloy::node_bindings::AnvilInstance {
    Anvil::new()
        .arg("--hardfork")
        .arg("prague")
        .spawn()
}

/// ERC-7821 batch mode constant (mode ID 1)
pub fn batch_mode() -> FixedBytes<32> {
    let mut bytes = [0u8; 32];
    bytes[0] = 0x01;
    FixedBytes::from(bytes)
}

/// Encode ERC-7821 batch execute calldata (mirrors backend's encode_execute)
pub fn encode_execute(calls: Vec<(Address, U256, Bytes)>) -> Bytes {
    let execution_data = alloy::sol_types::SolValue::abi_encode(&calls);
    let execute_call = IERC7821::executeCall {
        mode: batch_mode(),
        executionData: execution_data.into(),
    };
    Bytes::from(execute_call.abi_encode())
}

/// Sign EIP-7702 authorization and submit the delegation tx.
/// The provider must have a wallet (keeper) that pays gas.
pub async fn set_delegation(
    provider: &(impl Provider + Send + Sync),
    burner: &PrivateKeySigner,
    implementation: Address,
) -> eyre::Result<()> {
    let chain_id = provider.get_chain_id().await?;
    let nonce = provider.get_transaction_count(burner.address()).await?;

    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: implementation,
        nonce,
    };

    let sig = burner.sign_hash_sync(&auth.signature_hash())?;
    let signed_auth = alloy::eips::eip7702::SignedAuthorization::new_unchecked(
        auth,
        sig.v() as u8,
        sig.r(),
        sig.s(),
    );

    let tx = TransactionRequest::default()
        .with_to(burner.address())
        .with_authorization_list(vec![signed_auth]);

    provider.send_transaction(tx).await?.get_receipt().await?;
    Ok(())
}
