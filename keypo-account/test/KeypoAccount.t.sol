// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {KeypoAccount} from "../src/KeypoAccount.sol";
import {P256Helper} from "./helpers/P256Helper.sol";
import {P256} from "@openzeppelin/contracts/utils/cryptography/P256.sol";
import {ERC4337Utils} from "@openzeppelin/contracts/account/utils/draft-ERC4337Utils.sol";

/// @dev Harness that exposes internal functions for direct testing.
contract KeypoAccountHarness is KeypoAccount {
    function exposed_rawSignatureValidation(
        bytes32 hash,
        bytes calldata signature
    ) external view returns (bool) {
        return _rawSignatureValidation(hash, signature);
    }

    function exposed_erc7821AuthorizedExecutor(
        address caller,
        bytes32 mode,
        bytes calldata executionData
    ) external view returns (bool) {
        return _erc7821AuthorizedExecutor(caller, mode, executionData);
    }
}

contract KeypoAccountTest is P256Helper {
    KeypoAccountHarness internal account;

    /// @dev ERC-7201 storage slot for Initializable state.
    bytes32 internal constant INITIALIZABLE_STORAGE =
        0xf0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00;

    function setUp() public {
        _deriveP256Keys();
        account = new KeypoAccountHarness();
        // Reset Initializable storage so initialize() can be called.
        // The constructor calls _disableInitializers(), setting _initialized = type(uint64).max.
        vm.store(address(account), INITIALIZABLE_STORAGE, bytes32(0));
        account.initialize(qx1, qy1);
    }

    // ---------------------------------------------------------------
    // Initialization tests
    // ---------------------------------------------------------------

    function test_initialize_setsPublicKey() public view {
        (bytes32 qx, bytes32 qy) = account.signer();
        assertEq(qx, qx1);
        assertEq(qy, qy1);
    }

    function test_initialize_revertsOnSecondCall() public {
        vm.expectRevert();
        account.initialize(qx2, qy2);
    }

    function test_implementationCannotBeReinitialized() public {
        // Fresh deploy without vm.store reset — constructor called _disableInitializers()
        KeypoAccountHarness fresh = new KeypoAccountHarness();
        vm.expectRevert();
        fresh.initialize(qx1, qy1);
    }

    function test_uninitializedAccount_rejectsSignature() public {
        // Deploy fresh account (constructor sets signer to generator point GX, GY).
        // A valid signature from PK1 should NOT validate against GX, GY.
        KeypoAccountHarness uninit = new KeypoAccountHarness();
        bytes32 hash = keccak256("test");
        bytes memory sig = _signRaw(PK1, hash);
        assertFalse(uninit.exposed_rawSignatureValidation(hash, sig));
    }

    // ---------------------------------------------------------------
    // Raw P-256 signature validation (64 bytes)
    // ---------------------------------------------------------------

    function test_rawSigValidation_rawP256_valid() public {
        bytes32 hash = keccak256("valid raw sig");
        bytes memory sig = _signRaw(PK1, hash);
        assertTrue(account.exposed_rawSignatureValidation(hash, sig));
    }

    function test_rawSigValidation_rawP256_invalid() public {
        bytes32 hash = keccak256("invalid raw sig");
        bytes memory sig = _invalidSignature(PK1, hash);
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    function test_rawSigValidation_rawP256_highS() public {
        bytes32 hash = keccak256("high-s raw sig");
        bytes memory sig = _signRawHighS(PK1, hash);
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    function test_rawSigValidation_rawP256_wrongKey() public {
        bytes32 hash = keccak256("wrong key raw sig");
        bytes memory sig = _signRaw(PK2, hash);
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    // ---------------------------------------------------------------
    // WebAuthn signature validation (>64 bytes)
    // ---------------------------------------------------------------

    function test_rawSigValidation_webauthn_valid() public {
        bytes32 hash = keccak256("valid webauthn sig");
        bytes memory sig = _signWebAuthn(PK1, hash);
        assertTrue(account.exposed_rawSignatureValidation(hash, sig));
    }

    function test_rawSigValidation_webauthn_invalid() public {
        bytes32 hash = keccak256("invalid webauthn sig");
        bytes memory sig = _invalidWebAuthnSignature(PK1, hash);
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    function test_rawSigValidation_webauthn_wrongKey() public {
        bytes32 hash = keccak256("wrong key webauthn");
        bytes memory sig = _signWebAuthn(PK2, hash);
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    // ---------------------------------------------------------------
    // Short signature rejection (<64 bytes)
    // ---------------------------------------------------------------

    function test_rawSigValidation_tooShort() public view {
        bytes32 hash = keccak256("short sig");
        bytes memory sig = abi.encodePacked(bytes32(0), bytes31(0)); // 63 bytes
        assertFalse(account.exposed_rawSignatureValidation(hash, sig));
    }

    // ---------------------------------------------------------------
    // ERC-7821 authorized executor
    // ---------------------------------------------------------------

    function test_erc7821AuthorizedExecutor_self() public view {
        bytes32 mode = bytes32(uint256(1) << 248); // batch mode 0x01
        assertEq(
            account.exposed_erc7821AuthorizedExecutor(address(account), mode, ""),
            true
        );
    }

    function test_erc7821AuthorizedExecutor_entryPoint() public view {
        bytes32 mode = bytes32(uint256(1) << 248);
        address ep = address(ERC4337Utils.ENTRYPOINT_V07);
        assertEq(
            account.exposed_erc7821AuthorizedExecutor(ep, mode, ""),
            true
        );
    }

    function test_erc7821AuthorizedExecutor_other() public view {
        bytes32 mode = bytes32(uint256(1) << 248);
        address random = address(0xdead);
        assertEq(
            account.exposed_erc7821AuthorizedExecutor(random, mode, ""),
            false
        );
    }
}
