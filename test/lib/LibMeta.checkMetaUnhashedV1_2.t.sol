// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {UnexpectedMetaHash, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaCheckMetaUnhashedV1_2Test is Test {
    function checkMetaUnhashedV1External(bytes memory meta) external pure {
        LibMeta.checkMetaUnhashedV1(meta);
    }

    /// All data with the magic number prefix will be considered to be rain meta
    /// and all without will not. This test is the same as the above but with
    /// the revert due to the check.
    function testCheckMetaUnhashedV1_2Fuzz(bytes memory data) public {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        LibMeta.checkMetaUnhashedV1(meta);

        vm.expectRevert(abi.encodeWithSelector(NotRainMetaV1.selector, data));
        this.checkMetaUnhashedV1External(data);
    }
}
