// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Script} from "forge-std/Script.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

/// @title Deploy
/// @notice A script that deploys all contracts. This is intended to be run on
/// every commit by CI to a testnet such as mumbai.
contract Deploy is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYMENT_KEY");

        LibRainDeploy.deployAndBroadcastToSupportedNetworks(
            vm,
            LibRainDeploy.supportedNetworks(),
            deployerPrivateKey,
            type(MetaBoard).creationCode,
            "",
            LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS,
            LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH,
            new address[](0)
        );

        vm.startBroadcast(deployerPrivateKey);

        new MetaBoard();

        vm.stopBroadcast();
    }
}
