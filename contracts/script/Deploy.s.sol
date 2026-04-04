// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Script, console} from "forge-std/Script.sol";
import {Create3Factory} from "../src/Create3Factory.sol";
import {DepositoorDelegate} from "../src/DepositoorDelegate.sol";

/// @notice Deploys DepositoorDelegate to the same address on every chain via CREATE3.
///
/// Usage:
///   PRIVATE_KEY=0x... KEEPER=0x... SALT=0x... \
///     forge script script/Deploy.s.sol --rpc-url <rpc> --broadcast
///
/// The CREATE3 factory is deployed first (if needed) via the deterministic
/// deployment proxy (0x4e59b44847b379578588920cA78FbF26c0B4956C).
contract Deploy is Script {
    // Nick's deterministic deployment proxy — deployed on every EVM chain
    address constant DETERMINISTIC_DEPLOYER = 0x4e59b44847b379578588920cA78FbF26c0B4956C;

    // Fixed salt for deploying the factory itself (same factory address everywhere)
    bytes32 constant FACTORY_SALT = bytes32(uint256(0xde90517001));

    // Canonical WETH / wrapped native per chain
    function _weth() internal view returns (address) {
        uint256 id = block.chainid;
        if (id == 1)     return 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2; // Ethereum
        if (id == 42161) return 0x82aF49447D8a07e3bd95BD0d56f35241523fBab1; // Arbitrum
        if (id == 8453)  return 0x4200000000000000000000000000000000000006; // Base
        if (id == 10)    return 0x4200000000000000000000000000000000000006; // Optimism
        if (id == 137)   return 0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270; // Polygon (WMATIC)
        revert("unsupported chain");
    }

    function run() external {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        address keeper = vm.envAddress("KEEPER");
        bytes32 salt = vm.envBytes32("SALT");

        vm.startBroadcast(pk);

        // --- Step 1: Deploy CREATE3 factory (idempotent) ---
        address factoryAddr = _deployFactory();
        Create3Factory factory = Create3Factory(factoryAddr);

        // --- Step 2: Predict address ---
        address predicted = factory.getDeployed(vm.addr(pk), salt);
        console.log("Predicted address:", predicted);

        // --- Step 3: Deploy DepositoorDelegate via CREATE3 ---
        if (predicted.code.length > 0) {
            console.log("Already deployed on chain", block.chainid);
        } else {
            bytes memory creationCode = abi.encodePacked(
                type(DepositoorDelegate).creationCode,
                abi.encode(_weth(), keeper)
            );
            address deployed = factory.deploy(salt, creationCode);
            require(deployed == predicted, "address mismatch");
            console.log("Deployed DepositoorDelegate at:", deployed);
        }

        console.log("  chain:", block.chainid);
        console.log("  weth:", _weth());
        console.log("  keeper:", keeper);

        vm.stopBroadcast();
    }

    function _deployFactory() internal returns (address) {
        bytes memory factoryInitCode = type(Create3Factory).creationCode;
        bytes memory payload = abi.encodePacked(FACTORY_SALT, factoryInitCode);

        // Predict the factory address
        address predicted = address(
            uint160(uint256(keccak256(abi.encodePacked(
                bytes1(0xff),
                DETERMINISTIC_DEPLOYER,
                FACTORY_SALT,
                keccak256(factoryInitCode)
            ))))
        );

        if (predicted.code.length > 0) {
            console.log("Factory already deployed at:", predicted);
            return predicted;
        }

        (bool ok,) = DETERMINISTIC_DEPLOYER.call(payload);
        require(ok, "factory deployment failed");
        require(predicted.code.length > 0, "factory has no code");
        console.log("Factory deployed at:", predicted);
        return predicted;
    }
}
