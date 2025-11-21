// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaIsRainMetaV1_2Test is Test {
    /// All data with the magic number prefix will be considered to be rain meta
    /// and all without will not.
    function testIsRainMetaV1_2Fuzz(bytes memory data) public pure {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        // True with prefix.
        assertTrue(LibMeta.isRainMetaV1(meta));
        // False without prefix.
        assertTrue(!LibMeta.isRainMetaV1(data));
    }
}
