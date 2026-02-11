// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity ^0.8.25;

/// @title LibMetaBoardDeploy
/// A library containing the deployed address and code hash of the MetaBoard
/// contract when deployed with the rain standard zoltu deployer. This allows
/// idempotent deployments against precommitted addresses and hashes that can be
/// easily verified automatically in tests and scripts rather than relying on
/// registries or manual verification.
library LibMetaBoardDeploy {
    /// The address of the `MetaBoard` contract when deployed with the rain
    /// standard zoltu deployer.
    address constant METABOARD_DEPLOYED_ADDRESS = address(0xfb8437AeFBB8031064E274527C5fc08e30Ac6928);

    /// The code hash of the `MetaBoard` contract when deployed with the rain
    /// standard zoltu deployer. This can be used to verify that the deployed
    /// contract has the expected bytecode, which provides stronger guarantees
    /// than just checking the address.
    bytes32 constant METABOARD_DEPLOYED_CODEHASH =
        bytes32(0x60e0735a3406074fd8f85adb2813d0d7c346337ea4bcc6f2ef4eb25077a4933c);
}
