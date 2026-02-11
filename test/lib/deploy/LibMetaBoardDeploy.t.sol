// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std/Test.sol";
import {LibRainDeploy} from "rain.deploy/lib/LibRainDeploy.sol";
import {LibMetaBoardDeploy} from "src/lib/deploy/LibMetaBoardDeploy.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

contract LibMetaBoardDeployTest is Test {
    function testDeployAddress() external {
        vm.createSelectFork(vm.envString("CI_FORK_ETH_RPC_URL"));

        address deployedAddress = LibRainDeploy.deployZoltu(type(MetaBoard).creationCode);

        assertEq(deployedAddress, LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS);
        assertTrue(address(deployedAddress).code.length > 0, "Deployed address has no code");

        assertEq(address(deployedAddress).codehash, LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH);
    }

    function testExpectedCodeHash() external {
        MetaBoard metaBoard = new MetaBoard();

        assertEq(address(metaBoard).codehash, LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH);
    }
}
