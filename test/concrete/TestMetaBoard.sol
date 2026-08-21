// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {IMetaBoardV1_2} from "src/interface/unstable/IMetaBoardV1_2.sol";
import {LibIMetaBoardV1_2} from "src/lib/LibIMetaBoardV1_2.sol";

/// @title TestMetaBoard
/// @notice A concrete `IMetaBoardV1_2` written the way the deploy half is meant
/// to write one: every function is a single delegation into
/// `LibIMetaBoardV1_2` and nothing else. It exists so tests here can exercise
/// the library through a real external surface — `msg.sender` attribution and
/// the `MetaV1_2` event are observable only across an external call — and it
/// doubles as the executable proof that the library surface suffices for a
/// pure-delegation concrete.
contract TestMetaBoard is IMetaBoardV1_2 {
    /// @inheritdoc IMetaBoardV1_2
    function emitMeta(bytes32 subject, bytes calldata meta) external {
        LibIMetaBoardV1_2.emitMeta(subject, meta);
    }
}
