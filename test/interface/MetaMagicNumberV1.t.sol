// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
//forge-lint: disable-next-line(unused-import)
import {META_MAGIC_NUMBER_V1 as META_MAGIC_NUMBER_V1_REEXPORT} from "src/interface/unstable/IMetaV1_2.sol";
import {META_MAGIC_NUMBER_V1} from "src/interface/deprecated/IMetaV1.sol";

/// @title MetaMagicNumberV1Test
/// @notice Pins the magic number LITERAL to the spec:
/// https://github.com/rainprotocol/specs/blob/main/metadata-v1.md
///
/// Every other Solidity test in this repo DERIVES its meta inputs from the
/// constant, so none of them can see the literal itself drift — a mutated
/// constant mutates their expectations with it. This is the one place the sol
/// lane spells the eight bytes as a preimage, the way
/// `SubgraphManifest.t.sol` spells the indexed event signature. The rust cli
/// (`crates/cli/src/meta/magic.rs`) and the matchstick suite
/// (`subgraph/tests/metaBoard.test.ts`) each spell the same bytes in their own
/// lanes; agreement across the three is what makes the value load bearing.
///
/// The aliased import from `IMetaV1_2.sol` pins the re-export at compile
/// time: the unstable interface MUST keep exporting the constant the
/// deprecated interface declares.
contract MetaMagicNumberV1Test is Test {
    /// The magic number as the eight wire bytes every rain meta document
    /// starts with, big endian, exactly as `LibMeta.isRainMetaV1` masks them
    /// off the front of `meta`.
    function testMetaMagicNumberV1PinnedToSpec() external pure {
        assertEq(abi.encodePacked(META_MAGIC_NUMBER_V1), hex"ff0a89c674ee7874");
    }
}
