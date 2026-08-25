// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaIsRainMetaV1_2Test is Test {
    /// All data with the magic number prefix will be considered to be rain meta
    /// and all without will not.
    ///
    /// The original form of this test asserted that `data` alone is NEVER
    /// rain meta, which is false whenever the fuzzed `data` itself begins
    /// with the magic number — by spec such data IS rain meta. The fuzzer
    /// found the counterexample `data = 0xff0a89c674ee7874` once the magic
    /// literal entered the fuzz dictionary, so the expectation is now split
    /// on whether `data` carries the prefix itself.
    function testIsRainMetaV1_2Fuzz(bytes memory data) public pure {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        // True with prefix.
        assertTrue(LibMeta.isRainMetaV1(meta));

        // Whether the raw data is itself rain meta depends only on whether it
        // begins with the 8 magic bytes.
        bool dataHasMagicPrefix = false;
        if (data.length >= 8) {
            uint256 prefix;
            assembly ("memory-safe") {
                prefix := shr(192, mload(add(data, 0x20)))
            }
            dataHasMagicPrefix = prefix == uint256(META_MAGIC_NUMBER_V1);
        }
        if (dataHasMagicPrefix) {
            assertTrue(LibMeta.isRainMetaV1(data));
        } else {
            assertTrue(!LibMeta.isRainMetaV1(data));
        }
    }

    /// A 7-byte input is strictly below the 8-byte magic prefix, so it can
    /// never be rain meta, even when the byte in memory JUST past its claimed
    /// length would complete the magic number. Discriminates the strict
    /// `< 8` length guard from a lowered guard, which would read past the
    /// array end and see the full magic.
    function testIsRainMetaV1_2LengthSevenMagicByteBeyond() external pure {
        bytes memory meta;
        assembly ("memory-safe") {
            meta := mload(0x40)
            mstore(0x40, add(meta, 0x40))
            mstore(meta, 7)
            mstore(add(meta, 0x20), 0xff0a89c674ee7874000000000000000000000000000000000000000000000000)
        }
        assertTrue(!LibMeta.isRainMetaV1(meta));
    }

    /// Pin the magic number to the literal published in the metadata-v1 spec
    /// (0xff0a89c674ee7874) instead of deriving the prefix from the constant
    /// the implementation reads, so a mutated constant cannot co-vary with
    /// the test input. The Rust crates pin the same literal in their own
    /// lane; this is the Solidity-side pin.
    function testIsRainMetaV1_2MagicNumberLiteral() external pure {
        assertEq(uint256(META_MAGIC_NUMBER_V1), uint256(0xff0a89c674ee7874));
        assertTrue(LibMeta.isRainMetaV1(hex"ff0a89c674ee7874"));
        assertTrue(!LibMeta.isRainMetaV1(hex"ff0a89c674ee7875"));
        assertTrue(!LibMeta.isRainMetaV1(hex"ef0a89c674ee7874"));
    }
}
