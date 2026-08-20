// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity ^0.8.25;

/// @title LibMetaBoardDeploy
/// A library containing the deployed address and code hash of the MetaBoard
/// contract when deployed with the rain standard zoltu deployer. This allows
/// idempotent deployments against precommitted addresses and hashes that can be
/// easily verified automatically in tests and scripts rather than relying on
/// registries or manual verification.
///
/// `MetaBoard.hash` was removed per
/// https://github.com/rainlanguage/rain.metadata/issues/106 which changed the
/// contract bytecode, so the current source no longer deploys to the deployed
/// v1 address. Until the redeploy is dispatched the two sets of pins diverge:
/// the `METABOARD_DEPLOYED_*` constants and start blocks describe the live v1
/// deployment (which stays live and indexed), while the
/// `METABOARD_CANDIDATE_*` constants describe where the current source will
/// land when deployed with the rain standard zoltu deployer.
library LibMetaBoardDeploy {
    /// The address of the live v1 `MetaBoard` contract as deployed with the
    /// rain standard zoltu deployer. This deployment predates the removal of
    /// `MetaBoard.hash` and stays live; the subgraph config and start blocks
    /// below describe it.
    address constant METABOARD_DEPLOYED_ADDRESS = address(0xfb8437AeFBB8031064E274527C5fc08e30Ac6928);

    /// The code hash of the live v1 `MetaBoard` contract as deployed with the
    /// rain standard zoltu deployer. This can be used to verify that the
    /// deployed contract has the expected bytecode, which provides stronger
    /// guarantees than just checking the address.
    bytes32 constant METABOARD_DEPLOYED_CODEHASH =
        bytes32(0x60e0735a3406074fd8f85adb2813d0d7c346337ea4bcc6f2ef4eb25077a4933c);

    /// The address that the current `MetaBoard` source deploys to with the
    /// rain standard zoltu deployer. Diverges from
    /// `METABOARD_DEPLOYED_ADDRESS` because `MetaBoard.hash` was removed per
    /// https://github.com/rainlanguage/rain.metadata/issues/106; nothing is
    /// deployed here until the redeploy is dispatched.
    address constant METABOARD_CANDIDATE_ADDRESS = address(0xBA6Bb6f3BC6516337ed20e7b9DF823c923ad9F7E);

    /// The code hash of the current `MetaBoard` source when deployed with the
    /// rain standard zoltu deployer.
    bytes32 constant METABOARD_CANDIDATE_CODEHASH =
        bytes32(0x2a1e5609f6cd2598614a46786f79134485401f278461208eaf0087dc0641aaae);

    uint256 constant METABOARD_START_BLOCK_ARBITRUM = 431042729;
    uint256 constant METABOARD_START_BLOCK_BASE = 42021282;
    uint256 constant METABOARD_START_BLOCK_BASE_SEPOLIA = 38683088;
    uint256 constant METABOARD_START_BLOCK_FLARE = 55347067;
    uint256 constant METABOARD_START_BLOCK_POLYGON = 82855948;
}
