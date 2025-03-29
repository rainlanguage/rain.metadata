// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std/Test.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {UnexpectedMetaHash, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract LibMetaCheckMetaHashedV1_2Test is Test {
    function checkMetaHashedV1External(bytes32 expectedHash, bytes memory meta) external pure {
        LibMeta.checkMetaHashedV1(expectedHash, meta);
    }

    /// When the data has a magic number, and the hash of the data matches the
    /// expected hash passed to the check, it should not revert.
    function testCheckMetaHashedV1_2Happy(bytes memory data) external pure {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        bytes32 metaHash = keccak256(meta);
        LibMeta.checkMetaHashedV1(metaHash, meta);
    }

    /// When the data has a magic number but the hash of the data does not
    /// match the expected hash passed to the check, it should revert.
    function testCheckMetaHashedV1_2GoodMagicBadHash(bytes memory data, bytes32 expectedHash) public {
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        bytes32 metaHash = keccak256(meta);
        vm.assume(metaHash != expectedHash);
        vm.expectRevert(abi.encodeWithSelector(UnexpectedMetaHash.selector, expectedHash, metaHash));
        this.checkMetaHashedV1External(expectedHash, meta);
    }

    /// When the data does not have a magic number, it should revert even if
    /// the hash of the data matches the expected hash passed to the check.
    function testCheckMetaHashedV1_2BadMagicGoodHash(bytes memory meta) public {
        bytes32 metaHash = keccak256(meta);
        vm.expectRevert(abi.encodeWithSelector(NotRainMetaV1.selector, meta));
        this.checkMetaHashedV1External(metaHash, meta);
    }

    /// When the data does not have a magic number, and the hash of the data
    /// does not match the expected hash passed to the check, it should revert.
    function testCheckMetaHashedV1_2BadMagicBadHash(bytes memory meta, bytes32 expectedHash) public {
        bytes32 metaHash = keccak256(meta);
        vm.assume(metaHash != expectedHash);

        vm.expectRevert(abi.encodeWithSelector(UnexpectedMetaHash.selector, expectedHash, metaHash));
        this.checkMetaHashedV1External(expectedHash, meta);
    }
}
