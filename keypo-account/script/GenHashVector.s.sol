// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import "forge-std/Script.sol";

/// @notice Packed UserOperation (v0.7) for EntryPoint.getUserOpHash()
struct PackedUserOperation {
    address sender;
    uint256 nonce;
    bytes initCode;
    bytes callData;
    bytes32 accountGasLimits;
    uint256 preVerificationGas;
    bytes32 gasFees;
    bytes paymasterAndData;
    bytes signature;
}

interface IEntryPoint {
    function getUserOpHash(PackedUserOperation calldata userOp) external view returns (bytes32);
}

/// @title GenHashVector
/// @notice Generates UserOp hash test vectors by calling the on-chain EntryPoint v0.7.
///         Run against a Base Sepolia fork:
///         cd keypo-account && forge script script/GenHashVector.s.sol --rpc-url https://sepolia.base.org -vvvv
contract GenHashVector is Script {
    IEntryPoint constant EP = IEntryPoint(0x0000000071727De22E5E9d8BAf0edAc6f37da032);

    function run() external view {
        // ─── Vector A: Minimal UserOp (no factory, no paymaster) ───
        {
            bytes32 accountGasLimits = packU128Pair(100000, 50000);
            bytes32 gasFees = packU128Pair(1000000000, 2000000000);

            PackedUserOperation memory opA = PackedUserOperation({
                sender: 0x1111111111111111111111111111111111111111,
                nonce: 0,
                initCode: "",
                callData: hex"abcdef",
                accountGasLimits: accountGasLimits,
                preVerificationGas: 21000,
                gasFees: gasFees,
                paymasterAndData: "",
                signature: hex"0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
            });

            bytes32 hashA = EP.getUserOpHash(opA);

            console.log("=== Vector A: Minimal ===");
            console.log("sender:", opA.sender);
            console.log("nonce:", opA.nonce);
            console.logBytes(opA.initCode);
            console.logBytes(opA.callData);
            console.logBytes32(opA.accountGasLimits);
            console.log("preVerificationGas:", opA.preVerificationGas);
            console.logBytes32(opA.gasFees);
            console.logBytes(opA.paymasterAndData);
            console.logBytes32(hashA);
        }

        // ─── Vector B: UserOp with paymaster ───
        {
            bytes32 accountGasLimits = packU128Pair(200000, 100000);
            bytes32 gasFees = packU128Pair(500000000, 3000000000);

            // paymasterAndData = paymaster (20 bytes) || paymasterVerificationGasLimit (16 bytes) || paymasterPostOpGasLimit (16 bytes) || paymasterData
            bytes memory paymasterAndData = abi.encodePacked(
                address(0x2222222222222222222222222222222222222222), // paymaster
                uint128(50000),   // paymasterVerificationGasLimit
                uint128(10000),   // paymasterPostOpGasLimit
                hex"aabbccdd"     // paymasterData
            );

            PackedUserOperation memory opB = PackedUserOperation({
                sender: 0x3333333333333333333333333333333333333333,
                nonce: 5,
                initCode: "",
                callData: hex"12345678",
                accountGasLimits: accountGasLimits,
                preVerificationGas: 50000,
                gasFees: gasFees,
                paymasterAndData: paymasterAndData,
                signature: hex"0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
            });

            bytes32 hashB = EP.getUserOpHash(opB);

            console.log("=== Vector B: With Paymaster ===");
            console.log("sender:", opB.sender);
            console.log("nonce:", opB.nonce);
            console.logBytes(opB.callData);
            console.logBytes32(opB.accountGasLimits);
            console.log("preVerificationGas:", opB.preVerificationGas);
            console.logBytes32(opB.gasFees);
            console.logBytes(opB.paymasterAndData);
            console.logBytes32(hashB);
        }

        // ─── Vector C: UserOp with factory ───
        {
            bytes32 accountGasLimits = packU128Pair(300000, 150000);
            bytes32 gasFees = packU128Pair(2000000000, 4000000000);

            // initCode = factory (20 bytes) || factoryData
            bytes memory initCode = abi.encodePacked(
                address(0x4444444444444444444444444444444444444444), // factory
                hex"deadbeef"                                        // factoryData
            );

            PackedUserOperation memory opC = PackedUserOperation({
                sender: 0x5555555555555555555555555555555555555555,
                nonce: 1,
                initCode: initCode,
                callData: hex"cafe",
                accountGasLimits: accountGasLimits,
                preVerificationGas: 30000,
                gasFees: gasFees,
                paymasterAndData: "",
                signature: hex"0101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"
            });

            bytes32 hashC = EP.getUserOpHash(opC);

            console.log("=== Vector C: With Factory ===");
            console.log("sender:", opC.sender);
            console.log("nonce:", opC.nonce);
            console.logBytes(opC.initCode);
            console.logBytes(opC.callData);
            console.logBytes32(opC.accountGasLimits);
            console.log("preVerificationGas:", opC.preVerificationGas);
            console.logBytes32(opC.gasFees);
            console.logBytes(opC.paymasterAndData);
            console.logBytes32(hashC);
        }
    }

    function packU128Pair(uint128 high, uint128 low) internal pure returns (bytes32) {
        return bytes32(uint256(uint128(high)) << 128 | uint256(uint128(low)));
    }
}
