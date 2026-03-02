// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Script, console} from "forge-std/Script.sol";
import {KeypoAccount} from "../src/KeypoAccount.sol";
import {IEntryPoint} from "@openzeppelin/contracts/interfaces/draft-IERC4337.sol";
import {ERC4337Utils} from "@openzeppelin/contracts/account/utils/draft-ERC4337Utils.sol";

/// @title Verify — Post-deployment verification of KeypoAccount
/// @notice Checks code existence, entryPoint version, and execution mode support.
contract Verify is Script {
    /// @dev ERC-7821 batch mode
    bytes32 constant BATCH_MODE = bytes32(uint256(1) << 248);

    function run() public view {
        address deployed = vm.envAddress("DEPLOYED_ADDRESS");

        // 1. Check code exists
        require(deployed.code.length > 0, "No code at address");
        console.log("Contract address:", deployed);
        console.log("Code size:", deployed.code.length);
        console.log("Code hash:", vm.toString(keccak256(deployed.code)));

        // 2. Verify entryPoint returns v0.7
        KeypoAccount account = KeypoAccount(payable(deployed));
        IEntryPoint ep = account.entryPoint();
        require(
            address(ep) == address(ERC4337Utils.ENTRYPOINT_V07),
            "entryPoint is not v0.7"
        );
        console.log("entryPoint:", address(ep), "(v0.7)");

        // 3. Verify batch execution mode is supported
        bool supported = account.supportsExecutionMode(BATCH_MODE);
        require(supported, "batch mode not supported");
        console.log("Batch mode (0x01) supported:", supported);

        console.log("All checks passed");
    }
}
