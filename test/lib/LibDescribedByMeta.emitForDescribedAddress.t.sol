// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibDescribedByMeta, MetadataMismatch} from "src/lib/LibDescribedByMeta.sol";
import {IDescribedByMetaV1} from "src/interface/IDescribedByMetaV1.sol";
import {IMetaBoardV1_2} from "src/interface/unstable/IMetaBoardV1_2.sol";
import {TestMetaBoard} from "test/concrete/TestMetaBoard.sol";
import {IMetaV1_2, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";
import {TestDescribedByMetaV1} from "test/lib/TestDescribedByMetaV1.sol";

contract LibDescribedByMetaEmitForDescribedAddressTest is Test {
    function externalEmitForDescribedAddress(IMetaBoardV1_2 metaboard, IDescribedByMetaV1 described, bytes memory meta)
        external
    {
        LibDescribedByMeta.emitForDescribedAddress(metaboard, described, meta);
    }

    function testEmitForDescribedAddressHappy(bytes memory metaData) external {
        IMetaBoardV1_2 metaboard = new TestMetaBoard();

        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, metaData);

        IDescribedByMetaV1 described = new TestDescribedByMetaV1(meta);

        // The metaboard is what emits, attributed to whoever called it — here
        // this test contract, as `emitForDescribedAddress` is internal and
        // inlines — and under the DESCRIBED contract's address as the subject,
        // not the caller's. Which address ends up in the subject is the whole
        // point of this function, and it is observable only across the external
        // call into the metaboard.
        vm.expectEmit(address(metaboard));
        emit IMetaV1_2.MetaV1_2(address(this), bytes32(uint256(uint160(address(described)))), meta);

        LibDescribedByMeta.emitForDescribedAddress(metaboard, described, meta);
    }

    function testEmitForDescribedAddressMismatch(bytes memory metaData, bytes memory expectedMetaData) external {
        IMetaBoardV1_2 metaboard = new TestMetaBoard();

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
