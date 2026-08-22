// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaCheckMetaUnhashedV1_2Test is Test {
    function checkMetaUnhashedV1External(bytes memory meta) external pure {
        LibMeta.checkMetaUnhashedV1(meta);
    }

    /// All data with the magic number prefix will be considered to be rain meta
    /// and all without will not. This test is the same as the above but with
    /// the revert due to the check.
    ///
    /// The original form asserted that raw `data` ALWAYS reverts, which is
    /// wrong whenever the fuzzed `data` itself begins with the magic number
    /// (counterexample found by the fuzzer: `data = 0xff0a89c674ee7874`), so
    /// the expectation is split on whether `data` carries the prefix itself.
    function testCheckMetaUnhashedV1_2Fuzz(bytes memory data) public {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        LibMeta.checkMetaUnhashedV1(meta);

        bool dataHasMagicPrefix = false;
        if (data.length >= 8) {
            uint256 prefix;
            assembly ("memory-safe") {
                prefix := shr(192, mload(add(data, 0x20)))
            }
            dataHasMagicPrefix = prefix == uint256(META_MAGIC_NUMBER_V1);
        }

        if (dataHasMagicPrefix) {
            // Data carrying the prefix is rain meta already: no revert.
            this.checkMetaUnhashedV1External(data);
        } else {
            vm.expectRevert(abi.encodeWithSelector(NotRainMetaV1.selector, data));
            this.checkMetaUnhashedV1External(data);
        }
    }
}
