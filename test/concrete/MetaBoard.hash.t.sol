// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.1/src/Test.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

contract MetaBoardHashTest is Test {
    function testMetaboardHash(bytes memory data) public {
        MetaBoard metaBoard = new MetaBoard();
        bytes32 h = metaBoard.hash(data);
        assertEq(h, keccak256(data));
    }
}
