// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {LibIMetaBoardV1_2} from "src/lib/LibIMetaBoardV1_2.sol";

/// @title TestLibIMetaBoardV1_2
/// @notice `LibIMetaBoardV1_2` run bare behind its own delegating surface,
/// declared independently of `MetaBoard` and deliberately NOT inheriting
/// `IMetaBoardV1_2`, so nothing but the library is shared between the two.
/// The equivalence suite holds the shipped concrete to this: same delegation,
/// therefore same runtime bytecode and same observable behaviour. If
/// `MetaBoard` ever grows logic of its own it diverges from this surface and
/// the suite fails.
contract TestLibIMetaBoardV1_2 {
    /// The library's `emitMeta` behind an external call, matching the
    /// `IMetaBoardV1_2.emitMeta` selector and calldata shape exactly.
    /// @param subject As per `IMetaBoardV1_2`.
    /// @param meta As per `IMetaBoardV1_2`.
    function emitMeta(bytes32 subject, bytes calldata meta) external {
        LibIMetaBoardV1_2.emitMeta(subject, meta);
    }
}
