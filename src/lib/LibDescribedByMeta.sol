// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity ^0.8.19;

import {IDescribedByMetaV1} from "../interface/IDescribedByMetaV1.sol";
import {IMetaBoardV1_2} from "../interface/unstable/IMetaBoardV1_2.sol";

/// @dev Thrown when metadata hash does not match expected value when attempting
/// to be emitted by the metaboard.
/// @param described The contract that describes itself with metadata.
/// @param expected The expected metadata hash.
/// @param actual The actual metadata hash.
error MetadataMismatch(IDescribedByMetaV1 described, bytes32 expected, bytes32 actual);

/// @title LibDescribedByMeta
/// Tools for working with IDescribedByMetaV1 contracts and metadata.
library LibDescribedByMeta {
    /// Emits metadata for a contract that implements IDescribedByMetaV1,
    /// verifying that the hash of the metadata matches the expected hash.
    /// The caller can be any or many contracts, as long as the metadata is
    /// emitted at least once it can be indexed offchain under the hash of the
    /// data and retrieved later.
    /// @param metaboard The metaboard to emit the metadata on.
    /// @param described The contract that describes itself with metadata.
    /// @param meta The metadata to emit.
    function emitForDescribedAddress(IMetaBoardV1_2 metaboard, IDescribedByMetaV1 described, bytes memory meta)
        internal
    {
        bytes32 expected = described.describedByMetaV1();
        bytes32 actual;
        assembly ("memory-safe") {
            actual := keccak256(add(meta, 0x20), mload(meta))
        }
        if (actual != expected) {
            revert MetadataMismatch(described, expected, actual);
        }
        metaboard.emitMeta(bytes32(uint256(uint160(address(described)))), meta);
    }
}
