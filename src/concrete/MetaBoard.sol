// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {IMetaBoardV1_2} from "../interface/unstable/IMetaBoardV1_2.sol";
import {LibMeta} from "../lib/LibMeta.sol";

contract MetaBoard is IMetaBoardV1_2 {
    /// @inheritdoc IMetaBoardV1_2
    function emitMeta(bytes32 subject, bytes calldata meta) external {
        LibMeta.checkMetaUnhashedV1(meta);
        emit MetaV1_2(msg.sender, subject, meta);
    }

    /// Exposes native hashing algorithm (keccak256) to facilitate indexing data
    /// under its hash. This avoids the need to roll a new interface to include
    /// hashes in the event logs.
    function hash(bytes calldata data) external pure returns (bytes32) {
        bytes32 dataHash;
        assembly ("memory-safe") {
            let free := mload(0x40)
            calldatacopy(free, data.offset, data.length)
            dataHash := keccak256(free, data.length)
        }
        return dataHash;
    }
}
