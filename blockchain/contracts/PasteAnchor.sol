// SPDX-License-Identifier: MIT
pragma solidity ^0.8.21;

/// @title PasteAnchor - Immutable anchoring of paste manifests
/// @notice Stores lightweight metadata about encrypted pastes on-chain so
///         recipients can verify integrity and provenance.
contract PasteAnchor {
    struct PasteRecord {
        bytes32 contentHash;      // Hash of encrypted content + metadata manifest
        uint64 expiresAt;         // Unix timestamp when the paste expires (0 = never)
        uint8 retentionClass;     // Enumerated retention class chosen client-side
        address creator;          // Wallet address that anchored the paste
        bytes32 attestationRef;   // Optional pointer to off-chain attestation (DID/EAS)
    }

    /// @notice Emitted whenever a paste manifest is anchored on-chain
    event PasteAnchored(
        bytes32 indexed pasteId,
        address indexed creator,
        bytes32 contentHash,
        uint64 expiresAt,
        uint8 retentionClass,
        bytes32 attestationRef
    );

    mapping(bytes32 => PasteRecord) private records;

    error PasteAlreadyAnchored();

    /// @notice Anchor a paste manifest by providing its deterministic identifier
    /// @param pasteId Deterministic identifier (e.g. keccak(chat_id || manifest_id))
    /// @param contentHash Hash of the paste manifest content
    /// @param expiresAt Optional expiry (0 = no expiry)
    /// @param retentionClass Enumerated retention option decided client-side
    /// @param attestationRef Pointer to optional off-chain attestation (32 bytes)
    function anchorPaste(
        bytes32 pasteId,
        bytes32 contentHash,
        uint64 expiresAt,
        uint8 retentionClass,
        bytes32 attestationRef
    ) external {
        if (records[pasteId].creator != address(0)) {
            revert PasteAlreadyAnchored();
        }

        records[pasteId] = PasteRecord({
            contentHash: contentHash,
            expiresAt: expiresAt,
            retentionClass: retentionClass,
            creator: msg.sender,
            attestationRef: attestationRef
        });

        emit PasteAnchored(pasteId, msg.sender, contentHash, expiresAt, retentionClass, attestationRef);
    }

    /// @notice Fetch the recorded metadata for a given paste identifier
    /// @param pasteId Deterministic identifier used during anchoring
    /// @return record PasteRecord struct containing metadata
    function getPaste(bytes32 pasteId) external view returns (PasteRecord memory record) {
        record = records[pasteId];
    }
}
