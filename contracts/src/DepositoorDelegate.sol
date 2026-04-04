// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {ERC7821} from "solady/accounts/ERC7821.sol";

interface IERC20 {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
    function approve(address, uint256) external returns (bool);
}

contract DepositoorDelegate is ERC7821 {
    address public immutable weth;
    address public immutable keeper;

    constructor(address _weth, address _keeper) {
        weth = _weth;
        keeper = _keeper;
    }

    /// @notice Sweep the entire balance of a token to a recipient.
    function sweep(address token, address to) external {
        require(msg.sender == keeper || msg.sender == address(this), "unauthorized");
        uint256 bal = IERC20(token).balanceOf(address(this));
        if (bal > 0) {
            require(IERC20(token).transfer(to, bal), "transfer failed");
        }
    }

    function _execute(
        bytes32,
        bytes calldata,
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
