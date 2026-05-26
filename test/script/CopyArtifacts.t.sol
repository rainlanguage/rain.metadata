// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 thedavidmeister
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.1/src/Test.sol";
import {LibCopyArtifacts} from "script/lib/LibCopyArtifacts.sol";

contract CopyArtifactsTest is Test {
    function _assertCommittedMatches(string memory contractName) internal {
        bytes memory liveAbi = LibCopyArtifacts.extractStable(vm, contractName);
        bytes memory committed = bytes(vm.readFile(LibCopyArtifacts.committedPath(contractName)));
        assertEq(
            keccak256(liveAbi),
            keccak256(committed),
            string.concat(
                contractName, ": run `forge script script/CopyArtifacts.sol` to update the committed artifact"
            )
        );
    }

    function testArtifactsCommitted() external {
        string[] memory names = LibCopyArtifacts.contracts();
        for (uint256 i = 0; i < names.length; i++) {
            _assertCommittedMatches(names[i]);
        }
    }
}
