// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity ^0.8.19;

//forge-lint: disable-next-line(unused-import)
import {IMetaV1_2, UnexpectedMetaHash, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "../interface/unstable/IMetaV1_2.sol";

/// @title LibMeta
/// @notice Need a place to put data that can be handled offchain like ABIs that
/// IS NOT etherscan.
library LibMeta {
    /// Returns true if the metadata bytes are prefixed by the Rain meta magic
    /// number. DOES NOT attempt to validate the body of the metadata as offchain
    /// tooling will be required for this.
    /// @param meta The data that may be rain metadata.
    /// @return True if `meta` is metadata, false otherwise.
    function isRainMetaV1(bytes memory meta) internal pure returns (bool) {
        if (meta.length < 8) return false;
        uint256 mask = type(uint64).max;
        uint256 magicNumber = META_MAGIC_NUMBER_V1;
        assembly ("memory-safe") {
            magicNumber := and(mload(add(meta, 8)), mask)
        }
        return magicNumber == META_MAGIC_NUMBER_V1;
    }

    /// Reverts if the provided `meta` is NOT metadata according to
    /// `isRainMetaV1`.
    /// @param meta The metadata bytes to check.
    function checkMetaUnhashedV1(bytes memory meta) internal pure {
        if (!isRainMetaV1(meta)) {
            revert NotRainMetaV1(meta);
        }
    }

    /// Reverts if the provided `meta` is NOT metadata according to
    /// `isRainMetaV1` OR it does not match the expected hash of its data.
    /// @param meta The metadata to check.
    function checkMetaHashedV1(bytes32 expectedHash, bytes memory meta) internal pure {
        bytes32 actualHash;
        assembly ("memory-safe") {
            actualHash := keccak256(add(meta, 0x20), mload(meta))
        }
        if (expectedHash != actualHash) {
            revert UnexpectedMetaHash(expectedHash, actualHash);
        }
        checkMetaUnhashedV1(meta);
    }
}
