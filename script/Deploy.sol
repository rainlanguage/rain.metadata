// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Script} from "forge-std-1.16.2/src/Script.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";
import {LibRainDeploy} from "rain-deploy-0.1.7/src/lib/LibRainDeploy.sol";
import {LibMetaBoardDeploy} from "src/lib/deploy/LibMetaBoardDeploy.sol";

/// @title Deploy
/// @notice Deploys `MetaBoard` to every network `LibRainDeploy` supports, via
/// the Zoltu factory, against the precommitted address and code hash in
/// `LibMetaBoardDeploy`. The deploy is idempotent: networks already holding the
/// expected code at the expected address are skipped. Dispatched manually by
/// the `Manual sol artifacts` workflow.
contract Deploy is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYMENT_KEY");

        LibRainDeploy.deployAndBroadcast(
            vm,
            LibRainDeploy.supportedNetworks(),
            deployerPrivateKey,
            type(MetaBoard).creationCode,
            "src/concrete/MetaBoard.sol:MetaBoard",
            LibMetaBoardDeploy.METABOARD_CANDIDATE_ADDRESS,
            LibMetaBoardDeploy.METABOARD_CANDIDATE_CODEHASH,
            new address[](0)
        );
    }
}
