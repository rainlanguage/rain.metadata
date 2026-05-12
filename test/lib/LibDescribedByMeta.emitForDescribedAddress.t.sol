// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.1/src/Test.sol";
import {LibDescribedByMeta, MetadataMismatch} from "src/lib/LibDescribedByMeta.sol";
import {IDescribedByMetaV1} from "src/interface/IDescribedByMetaV1.sol";
import {IMetaBoardV1_2} from "src/interface/unstable/IMetaBoardV1_2.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";
import {META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

contract TestDescribedByMetaV1 is IDescribedByMetaV1 {
    bytes32 public immutable EXPECTED;

    constructor(bytes memory meta) {
        EXPECTED = keccak256(meta);
    }

    function describedByMetaV1() external view override returns (bytes32) {
        return EXPECTED;
    }
}

contract LibDescribedByMetaEmitForDescribedAddressTest is Test {
    function externalEmitForDescribedAddress(IMetaBoardV1_2 metaboard, IDescribedByMetaV1 described, bytes memory meta)
        external
    {
        LibDescribedByMeta.emitForDescribedAddress(metaboard, described, meta);
    }

    function testEmitForDescribedAddressHappy(bytes memory metaData) external {
        IMetaBoardV1_2 metaboard = new MetaBoard();

        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, metaData);

        IDescribedByMetaV1 described = new TestDescribedByMetaV1(meta);

        LibDescribedByMeta.emitForDescribedAddress(metaboard, described, meta);
    }

    function testEmitForDescribedAddressMismatch(bytes memory metaData, bytes memory expectedMetaData) external {
        IMetaBoardV1_2 metaboard = new MetaBoard();

        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, metaData);
        bytes memory expectedMeta = abi.encodePacked(META_MAGIC_NUMBER_V1, expectedMetaData);

        IDescribedByMetaV1 described = new TestDescribedByMetaV1(expectedMeta);

        vm.assume(keccak256(meta) != keccak256(expectedMeta));
        vm.expectRevert(
            abi.encodeWithSelector(MetadataMismatch.selector, described, keccak256(expectedMeta), keccak256(meta))
        );

        this.externalEmitForDescribedAddress(metaboard, described, meta);
    }
}
