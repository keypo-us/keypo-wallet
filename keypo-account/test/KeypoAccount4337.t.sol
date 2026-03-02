// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {KeypoAccount} from "../src/KeypoAccount.sol";
import {P256Helper} from "./helpers/P256Helper.sol";
import {ERC4337Utils} from "@openzeppelin/contracts/account/utils/draft-ERC4337Utils.sol";
import {PackedUserOperation} from "@openzeppelin/contracts/interfaces/draft-IERC4337.sol";
import {Execution} from "@openzeppelin/contracts/interfaces/draft-IERC7579.sol";

/// @dev Simple ERC-20 mock for transfer tests.
contract MockERC20 {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "insufficient balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract KeypoAccount4337Test is P256Helper {
    KeypoAccount internal account;
    address internal ep;

    /// ERC-7821 batch mode: 0x01 as first byte of bytes32
    bytes32 internal constant BATCH_MODE = bytes32(uint256(1) << 248);

    /// ERC-7201 storage slot for Initializable
    bytes32 internal constant INITIALIZABLE_STORAGE =
        0xf0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00;

    function setUp() public {
        _deriveP256Keys();
        account = new KeypoAccount();
        ep = address(ERC4337Utils.ENTRYPOINT_V07);
        // Reset Initializable and set P-256 key
        vm.store(address(account), INITIALIZABLE_STORAGE, bytes32(0));
        account.initialize(qx1, qy1);
    }

    // ---------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------

    function _makeUserOp(bytes memory signature) internal view returns (PackedUserOperation memory) {
        return PackedUserOperation({
            sender: address(account),
            nonce: 0,
            initCode: "",
            callData: "",
            accountGasLimits: bytes32(0),
            preVerificationGas: 0,
            gasFees: bytes32(0),
            paymasterAndData: "",
            signature: signature
        });
    }

    function _encodeBatch(
        Execution[] memory executions
    ) internal pure returns (bytes memory) {
        return abi.encode(executions);
    }

    function _singleExecution(
        address target,
        uint256 value,
        bytes memory data
    ) internal pure returns (bytes memory) {
        Execution[] memory batch = new Execution[](1);
        batch[0] = Execution({target: target, value: value, callData: data});
        return _encodeBatch(batch);
    }

    // ---------------------------------------------------------------
    // validateUserOp tests
    // ---------------------------------------------------------------

    function test_validateUserOp_rawP256_valid() public {
        bytes32 userOpHash = keccak256("userOp1");
        bytes memory sig = _signRaw(PK1, userOpHash);
        PackedUserOperation memory op = _makeUserOp(sig);
        vm.prank(ep);
        uint256 result = account.validateUserOp(op, userOpHash, 0);
        assertEq(result, ERC4337Utils.SIG_VALIDATION_SUCCESS);
    }

    function test_validateUserOp_rawP256_invalid() public {
        bytes32 userOpHash = keccak256("userOp2");
        bytes memory sig = _invalidSignature(PK1, userOpHash);
        PackedUserOperation memory op = _makeUserOp(sig);
        vm.prank(ep);
        uint256 result = account.validateUserOp(op, userOpHash, 0);
        assertEq(result, ERC4337Utils.SIG_VALIDATION_FAILED);
    }

    function test_validateUserOp_webauthn_valid() public {
        bytes32 userOpHash = keccak256("userOp3");
        bytes memory sig = _signWebAuthn(PK1, userOpHash);
        PackedUserOperation memory op = _makeUserOp(sig);
        vm.prank(ep);
        uint256 result = account.validateUserOp(op, userOpHash, 0);
        assertEq(result, ERC4337Utils.SIG_VALIDATION_SUCCESS);
    }

    function test_validateUserOp_webauthn_invalid() public {
        bytes32 userOpHash = keccak256("userOp4");
        bytes memory sig = _invalidWebAuthnSignature(PK1, userOpHash);
        PackedUserOperation memory op = _makeUserOp(sig);
        vm.prank(ep);
        uint256 result = account.validateUserOp(op, userOpHash, 0);
        assertEq(result, ERC4337Utils.SIG_VALIDATION_FAILED);
    }

    function test_validateUserOp_notFromEntryPoint() public {
        bytes32 userOpHash = keccak256("userOp5");
        bytes memory sig = _signRaw(PK1, userOpHash);
        PackedUserOperation memory op = _makeUserOp(sig);
        vm.expectRevert();
        account.validateUserOp(op, userOpHash, 0);
    }

    // ---------------------------------------------------------------
    // ERC-7821 execute tests (via EntryPoint prank)
    // ---------------------------------------------------------------

    function test_execute_singleCall() public {
        // Execute a no-op call to self (just tests the execution path)
        bytes memory executionData = _singleExecution(address(account), 0, "");
        vm.prank(ep);
        account.execute(BATCH_MODE, executionData);
    }

    function test_execute_batchCalls() public {
        // Deploy a mock contract to receive calls
        MockERC20 token = new MockERC20();
        token.mint(address(account), 1000);

        Execution[] memory batch = new Execution[](2);
        batch[0] = Execution({
            target: address(token),
            value: 0,
            callData: abi.encodeCall(MockERC20.transfer, (address(0xBEEF), 100))
        });
        batch[1] = Execution({
            target: address(token),
            value: 0,
            callData: abi.encodeCall(MockERC20.transfer, (address(0xCAFE), 200))
        });

        vm.prank(ep);
        account.execute(BATCH_MODE, _encodeBatch(batch));

        assertEq(token.balanceOf(address(0xBEEF)), 100);
        assertEq(token.balanceOf(address(0xCAFE)), 200);
        assertEq(token.balanceOf(address(account)), 700);
    }

    function test_execute_ethTransfer() public {
        vm.deal(address(account), 1 ether);
        address payable recipient = payable(address(0xDEAD));

        bytes memory executionData = _singleExecution(recipient, 0.5 ether, "");
        vm.prank(ep);
        account.execute(BATCH_MODE, executionData);

        assertEq(recipient.balance, 0.5 ether);
        assertEq(address(account).balance, 0.5 ether);
    }

    function test_execute_erc20Transfer() public {
        MockERC20 token = new MockERC20();
        token.mint(address(account), 1000);
        address recipient = address(0xBEEF);

        bytes memory executionData = _singleExecution(
            address(token),
            0,
            abi.encodeCall(MockERC20.transfer, (recipient, 500))
        );
        vm.prank(ep);
        account.execute(BATCH_MODE, executionData);

        assertEq(token.balanceOf(recipient), 500);
        assertEq(token.balanceOf(address(account)), 500);
    }

    function test_execute_emptyBatch() public {
        Execution[] memory batch = new Execution[](0);
        vm.prank(ep);
        account.execute(BATCH_MODE, _encodeBatch(batch));
    }

    function test_execute_unauthorizedCaller() public {
        bytes memory executionData = _singleExecution(address(0), 0, "");
        vm.prank(address(0xBAD));
        vm.expectRevert();
        account.execute(BATCH_MODE, executionData);
    }
}
