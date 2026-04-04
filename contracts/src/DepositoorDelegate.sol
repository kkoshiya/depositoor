// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {ERC7821} from "solady/accounts/ERC7821.sol";

contract DepositoorDelegate is ERC7821 {
    address public immutable weth;
    address public immutable keeper;

    constructor(address _weth, address _keeper) {
        weth = _weth;
        keeper = _keeper;
    }

    function _execute(
        bytes32 mode,
        bytes calldata executionData,
        Call[] calldata calls,
        bytes calldata opData
    ) internal virtual override {
        if (opData.length == 0) {
            require(msg.sender == keeper || msg.sender == address(this), "unauthorized");
            _execute(calls, bytes32(0));
            return;
        }
        revert("opData not supported");
    }

    receive() external payable override {
        (bool success,) = weth.call{value: msg.value}("");
        require(success, "WETH wrap failed");
    }
}
