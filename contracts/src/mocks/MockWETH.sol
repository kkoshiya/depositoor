// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

contract MockWETH {
    mapping(address => uint256) public balanceOf;

    receive() external payable {
        balanceOf[msg.sender] += msg.value;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "insufficient balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}
