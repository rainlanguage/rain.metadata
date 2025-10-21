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
}
