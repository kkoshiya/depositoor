// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {CREATE3} from "solady/utils/CREATE3.sol";

/// @notice Minimal CREATE3 factory for deterministic cross-chain deployments.
/// @dev Deploy this contract via the deterministic deployer (0x4e59...56C)
///      so it lives at the same address on every chain.
contract Create3Factory {
    function deploy(bytes32 salt, bytes calldata creationCode)
        external
        payable
        returns (address)
    {
        // Mix msg.sender into salt so different deployers can't collide
        bytes32 guardedSalt = keccak256(abi.encodePacked(msg.sender, salt));
        return CREATE3.deployDeterministic(msg.value, creationCode, guardedSalt);
    }

    function getDeployed(address deployer, bytes32 salt) external view returns (address) {
        bytes32 guardedSalt = keccak256(abi.encodePacked(deployer, salt));
        return CREATE3.predictDeterministicAddress(guardedSalt, address(this));
    }
}
