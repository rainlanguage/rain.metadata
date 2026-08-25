// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaIsRainMetaV1_2Test is Test {
    /// All data with the magic number prefix will be considered to be rain meta
    /// and all without will not.
    function testIsRainMetaV1_2Fuzz(bytes memory data) public pure {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        // True with prefix.
        assertTrue(LibMeta.isRainMetaV1(meta));
        // False without prefix. The fuzzer CAN produce `data` that carries
        // the prefix itself — the magic number is a PUSH8 in this very
        // suite's bytecode, so it sits in the fuzz dictionary — which made
        // this half flaky red on any seed that emitted it. When that happens
        // the prefix is broken deterministically rather than the run
        // discarded: the magic starts 0xff, so a zeroed first byte is never
        // the prefix, and every other fuzzed byte keeps its value.
        if (LibMeta.isRainMetaV1(data)) {
            data[0] = 0;
        }
        assertTrue(!LibMeta.isRainMetaV1(data));
    }
}
