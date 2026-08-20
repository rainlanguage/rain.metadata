// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {IMetaBoardV1_2} from "../interface/unstable/IMetaBoardV1_2.sol";
import {LibIMetaBoardV1_2} from "../lib/LibIMetaBoardV1_2.sol";

/// @title MetaBoard
/// @notice The deployed concrete `IMetaBoardV1_2`: every function is a single
/// delegation into `LibIMetaBoardV1_2` and nothing else
/// (rainlanguage/rain.metadata#144). The magic number check and the `MetaV1_2`
/// event live in the library, unit tested there, and this contract adds no
/// behaviour of its own — the equivalence suite in this repo holds each entry
/// point to exactly the library's behaviour.
///
/// `msg.sender` is read inside the library and the internal functions execute
/// in this contract's call context, so the event is emitted by this contract
/// and attributed to whoever called it. See `IMetaBoardV1_2` for the spec: a
/// metaboard is an open bulletin board that anons MAY fill with garbage, so
/// tooling must treat everything indexed from it as untrusted.
contract MetaBoard is IMetaBoardV1_2 {
    /// @inheritdoc IMetaBoardV1_2
    function emitMeta(bytes32 subject, bytes calldata meta) external {
        LibIMetaBoardV1_2.emitMeta(subject, meta);
    }
}
