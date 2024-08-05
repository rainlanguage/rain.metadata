// SPDX-License-Identifier: CAL
pragma solidity ^0.8.19;

import {IDescribedByMetaV1} from "../interface/IDescribedByMetaV1.sol";
import {IMetaBoardV1_2} from "../interface/unstable/IMetaBoardV1_2.sol";

error MetadataMismatch(IDescribedByMetaV1 described, bytes32 expected, bytes32 actual);

/// @title LibDescribedByMeta
library LibDescribedByMeta {
    function emitForDescribedAddress(IMetaBoardV1_2 metaboard, IDescribedByMetaV1 described, bytes memory meta)
        internal
    {
        bytes32 expected = described.describedByMetaV1();
        bytes32 actual = keccak256(meta);
        if (actual != expected) {
            revert MetadataMismatch(described, expected, actual);
        }
        metaboard.emitMeta(bytes32(uint256(uint160(address(described)))), meta);
    }
}
