// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Script, console} from "forge-std/Script.sol";
import {CREATE3} from "solady/utils/CREATE3.sol";

/// @notice Mines a vanity salt for CREATE3 deployment.
/// @dev Run: forge script script/MineSalt.s.sol --sig "mine(string,uint256)" "dep051" 1000000
contract MineSalt is Script {
    // Must match Deploy.s.sol
    address constant DETERMINISTIC_DEPLOYER = 0x4e59b44847b379578588920cA78FbF26c0B4956C;
    bytes32 constant FACTORY_SALT = bytes32(uint256(0xde90517001));

    function _factoryAddress() internal pure returns (address) {
        bytes32 initCodeHash = keccak256(type(Create3FactoryStub).creationCode);
        return address(uint160(uint256(keccak256(abi.encodePacked(
            bytes1(0xff),
            DETERMINISTIC_DEPLOYER,
            FACTORY_SALT,
            initCodeHash
        )))));
    }

    function mine(string calldata prefix, uint256 maxAttempts) external view {
        uint256 pk = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(pk);
        address factory = _factoryAddress();

        bytes memory prefixBytes = bytes(prefix);
        uint256 prefixLen = prefixBytes.length;

        console.log("Factory address:", factory);
        console.log("Deployer:", deployer);
        console.log("Prefix:", prefix);
        console.log("Max attempts:", maxAttempts);
        console.log("---");

        uint256 found = 0;
        for (uint256 i = 0; i < maxAttempts; i++) {
            bytes32 salt = bytes32(i);
            bytes32 guardedSalt = keccak256(abi.encodePacked(deployer, salt));
            address predicted = CREATE3.predictDeterministicAddress(guardedSalt, factory);

            // Check if address starts with prefix (after 0x)
            if (_matchesPrefix(predicted, prefixBytes, prefixLen)) {
                found++;
                console.log("Found! salt:", vm.toString(salt));
                console.log("  address:", vm.toString(predicted));
                if (found >= 5) break; // Stop after 5 matches
            }
        }

        if (found == 0) {
            console.log("No matches found. Try increasing maxAttempts.");
        }
    }

    function mine(string calldata prefix) external view {
        // Default: 2M attempts — usually enough for a 3-char hex prefix
        this.mine(prefix, 2_000_000);
    }

    function _matchesPrefix(address addr, bytes memory prefix, uint256 len)
        internal
        pure
        returns (bool)
    {
        bytes memory addrHex = _toHexString(addr);
        for (uint256 i = 0; i < len; i++) {
            // Compare lowercase
            uint8 a = uint8(addrHex[i]);
            uint8 b = uint8(prefix[i]);
            if (a >= 65 && a <= 90) a += 32; // lowercase
            if (b >= 65 && b <= 90) b += 32;
            if (a != b) return false;
        }
        return true;
    }

    function _toHexString(address addr) internal pure returns (bytes memory) {
        bytes memory hex16 = "0123456789abcdef";
        bytes20 data = bytes20(addr);
        bytes memory str = new bytes(40);
        for (uint256 i = 0; i < 20; i++) {
            str[i * 2] = hex16[uint8(data[i]) >> 4];
            str[i * 2 + 1] = hex16[uint8(data[i]) & 0x0f];
        }
        return str;
    }
}

/// @dev Stub so we can compute the factory's initcode hash without importing it.
///      Must match Create3Factory exactly.
import {CREATE3 as C3} from "solady/utils/CREATE3.sol";
contract Create3FactoryStub {
    function deploy(bytes32 salt, bytes calldata creationCode)
        external payable returns (address)
    {
        bytes32 guardedSalt = keccak256(abi.encodePacked(msg.sender, salt));
        return C3.deployDeterministic(msg.value, creationCode, guardedSalt);
    }

    function getDeployed(address deployer, bytes32 salt) external view returns (address) {
        bytes32 guardedSalt = keccak256(abi.encodePacked(deployer, salt));
        return C3.predictDeterministicAddress(guardedSalt, address(this));
    }
}
