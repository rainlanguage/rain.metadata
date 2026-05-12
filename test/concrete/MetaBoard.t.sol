// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.1/src/Test.sol";
import {IMetaV1_2, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

contract MetaBoardTest is Test, IMetaV1_2 {
    function testEmitMeta(bytes32 subject, bytes memory data) public {
        vm.assume(!LibMeta.isRainMetaV1(data));

        MetaBoard metaBoard = new MetaBoard();

        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);
        vm.expectEmit(false, false, false, true);
        //slither-disable-next-line reentrancy-events
        emit MetaV1_2(address(this), subject, meta);
        metaBoard.emitMeta(subject, meta);

        vm.expectRevert(abi.encodeWithSelector(NotRainMetaV1.selector, data));
        metaBoard.emitMeta(subject, data);
    }
}
