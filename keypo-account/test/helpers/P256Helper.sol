// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {P256} from "@openzeppelin/contracts/utils/cryptography/P256.sol";
import {WebAuthn} from "@openzeppelin/contracts/utils/cryptography/WebAuthn.sol";
import {Base64} from "@openzeppelin/contracts/utils/Base64.sol";

/// @dev Test helper that provides P-256 keypairs and signature utilities.
///      Public keys are derived from private keys using vm.signP256 + P256.recovery.
abstract contract P256Helper is Test {
    // Test keypair 1
    uint256 internal constant PK1 = 0xdead00000000000000000000000000000000000000000000000000000000beef;
    bytes32 internal qx1;
    bytes32 internal qy1;

    // Test keypair 2 (for wrong-key tests)
    uint256 internal constant PK2 = 0xcafe00000000000000000000000000000000000000000000000000000000babe;
    bytes32 internal qx2;
    bytes32 internal qy2;

    function _deriveP256Keys() internal {
        (qx1, qy1) = _derivePublicKey(PK1);
        (qx2, qy2) = _derivePublicKey(PK2);
    }

    function _derivePublicKey(uint256 privateKey) internal returns (bytes32 qx, bytes32 qy) {
        bytes32 digest = keccak256("keygen");
        (bytes32 r, bytes32 s) = vm.signP256(privateKey, digest);
        // Try both recovery IDs (y parity 0 and 1)
        for (uint8 v = 0; v < 2; v++) {
            (bytes32 cx, bytes32 cy) = P256.recovery(digest, v, r, s);
            if (cx == bytes32(0) && cy == bytes32(0)) continue;
            // Verify with a second signature to confirm correct key
            bytes32 digest2 = keccak256("verify");
            (bytes32 r2, bytes32 s2) = vm.signP256(privateKey, digest2);
            if (P256.verify(digest2, r2, s2, cx, cy)) {
                return (cx, cy);
            }
        }
        revert("P256Helper: failed to derive public key");
    }

    /// @dev Creates a raw 64-byte P-256 signature (r || s).
    function _signRaw(uint256 privateKey, bytes32 hash) internal returns (bytes memory) {
        (bytes32 r, bytes32 s) = vm.signP256(privateKey, hash);
        return abi.encodePacked(r, s);
    }

    /// @dev Creates a high-S signature (s > N/2) for malleability testing.
    function _signRawHighS(uint256 privateKey, bytes32 hash) internal returns (bytes memory) {
        (bytes32 r, bytes32 s) = vm.signP256(privateKey, hash);
        // Negate s: s' = N - s. If s was low-S, s' will be high-S.
        bytes32 highS = bytes32(P256.N - uint256(s));
        return abi.encodePacked(r, highS);
    }

    /// @dev Creates an invalid (corrupted) raw signature.
    function _invalidSignature(uint256 privateKey, bytes32 hash) internal returns (bytes memory) {
        (bytes32 r, bytes32 s) = vm.signP256(privateKey, hash);
        // Corrupt r by flipping the last byte
        bytes32 badR = bytes32(uint256(r) ^ 0xff);
        return abi.encodePacked(badR, s);
    }

    /// @dev Builds a WebAuthn assertion signature for the given hash.
    ///      The challenge is abi.encodePacked(hash) (32 bytes), base64url-encoded
    ///      in clientDataJSON. Flags = UP only (0x01), no UV.
    function _signWebAuthn(
        uint256 privateKey,
        bytes32 hash
    ) internal returns (bytes memory) {
        return _buildWebAuthnSig(privateKey, hash, false);
    }

    /// @dev Builds an invalid WebAuthn signature (corrupted r).
    function _invalidWebAuthnSignature(
        uint256 privateKey,
        bytes32 hash
    ) internal returns (bytes memory) {
        return _buildWebAuthnSig(privateKey, hash, true);
    }

    function _buildWebAuthnSig(
        uint256 privateKey,
        bytes32 hash,
        bool corrupt
    ) internal returns (bytes memory) {
        bytes memory challenge = abi.encodePacked(hash);
        string memory b64Challenge = Base64.encodeURL(challenge);

        // Build clientDataJSON
        string memory clientDataJSON = string.concat(
            '{"type":"webauthn.get","challenge":"',
            b64Challenge,
            '","origin":"https://keypo.test","crossOrigin":false}'
        );

        // authenticatorData: rpIdHash (32 bytes) + flags (1 byte) + counter (4 bytes)
        bytes32 rpIdHash = sha256("keypo.test");
        bytes1 flags = 0x01; // UP=1, UV=0
        bytes memory authenticatorData = abi.encodePacked(rpIdHash, flags, uint32(1));

        // Compute the signing input per WebAuthn spec
        bytes32 clientDataHash = sha256(bytes(clientDataJSON));
        bytes32 signingInput = sha256(abi.encodePacked(authenticatorData, clientDataHash));

        // Sign the input
        (bytes32 r, bytes32 s) = vm.signP256(privateKey, signingInput);
        if (corrupt) {
            r = bytes32(uint256(r) ^ 0xff);
        }

        // Build WebAuthnAuth struct — ABI encoded
        // typeIndex: position of "type": in clientDataJSON = 1
        // challengeIndex: position of "challenge": in clientDataJSON
        uint256 typeIndex = 1; // {"type" starts at index 1
        uint256 challengeIndex = 23; // ,"challenge" starts at index 23 (the opening quote)

        // Encode as a flat tuple (not a struct) to match tryDecodeAuth expectations.
        // abi.encode(struct) adds an outer offset pointer; flat encoding does not.
        return abi.encode(r, s, challengeIndex, typeIndex, authenticatorData, clientDataJSON);
    }
}
