// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Script, console} from "forge-std/Script.sol";
import {KeypoAccount} from "../src/KeypoAccount.sol";

/// @title Deploy — CREATE2 deployment of KeypoAccount
/// @notice Uses Safe Singleton Factory for deterministic cross-chain addresses.
contract Deploy is Script {
    /// @dev Safe Singleton Factory — deployed on all major chains.
    address constant SINGLETON_FACTORY = 0x4e59b44847b379578588920cA78FbF26c0B4956C;

    /// @dev Deterministic salt for v0.1.0 deployment.
    bytes32 constant SALT = keccak256("keypo-account-v0.1.0");

    function run() public {
        bytes memory creationCode = type(KeypoAccount).creationCode;
        address expected = _computeCreate2Address(SALT, keccak256(creationCode));

        console.log("Expected address:", expected);
        console.log("Salt:", vm.toString(SALT));
        console.log("Init code hash:", vm.toString(keccak256(creationCode)));

        // Check if already deployed (idempotent)
        if (expected.code.length > 0) {
            console.log("Already deployed at", expected);
            console.log("Code hash:", vm.toString(keccak256(expected.code)));
            return;
        }

        vm.startBroadcast();
        (bool success, bytes memory result) = SINGLETON_FACTORY.call(
            abi.encodePacked(SALT, creationCode)
        );
        vm.stopBroadcast();

        require(success && result.length >= 20, "CREATE2 deployment failed");
        address deployed;
        assembly {
            deployed := mload(add(result, mload(result)))
        }
        require(deployed == expected, "Address mismatch");

        console.log("Deployed at:", deployed);
        console.log("Code hash:", vm.toString(keccak256(deployed.code)));
    }

    function _computeCreate2Address(
        bytes32 salt,
        bytes32 initCodeHash
    ) internal pure returns (address) {
        return address(
            uint160(
                uint256(
                    keccak256(
                        abi.encodePacked(bytes1(0xff), SINGLETON_FACTORY, salt, initCodeHash)
                    )
                )
            )
        );
    }
}
