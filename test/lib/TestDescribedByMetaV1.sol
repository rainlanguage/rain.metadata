// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {IDescribedByMetaV1} from "src/interface/IDescribedByMetaV1.sol";

/// @dev Test implementation of `IDescribedByMetaV1` that reports the keccak256
/// hash of the metadata bytes provided to its constructor.
contract TestDescribedByMetaV1 is IDescribedByMetaV1 {
    bytes32 public immutable EXPECTED;

    constructor(bytes memory meta) {
        EXPECTED = keccak256(meta);
    }

    function describedByMetaV1() external view override returns (bytes32) {
        return EXPECTED;
    }
}
