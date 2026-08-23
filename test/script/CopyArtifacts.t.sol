// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 thedavidmeister
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibCopyArtifacts} from "script/lib/LibCopyArtifacts.sol";
import {CopyArtifacts} from "script/CopyArtifacts.sol";

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

    /// All committed-artifact behaviour lives in this single test function:
    /// forge runs test functions in parallel, and every step below mutates or
    /// reads the same on-disk committed files, so splitting the steps into
    /// separate tests races them against each other.
    function testArtifactsCommitted() external {
        string[] memory names = LibCopyArtifacts.contracts();
        for (uint256 i = 0; i < names.length; i++) {
            _assertCommittedMatches(names[i]);
        }

        // Corrupting every committed artifact and running the script restores
        // each committed file byte-exactly to its pre-corruption on-disk
        // state.
        bytes32[] memory preCorruption = new bytes32[](names.length);
        for (uint256 i = 0; i < names.length; i++) {
            preCorruption[i] = keccak256(bytes(vm.readFile(LibCopyArtifacts.committedPath(names[i]))));
            vm.writeFile(LibCopyArtifacts.committedPath(names[i]), "corrupt");
        }
        new CopyArtifacts().run();
        for (uint256 i = 0; i < names.length; i++) {
            assertEq(
                keccak256(bytes(vm.readFile(LibCopyArtifacts.committedPath(names[i])))),
                preCorruption[i],
                string.concat(names[i], ": committed artifact not restored byte-exactly")
            );
        }

        // Deleting a committed artifact and running the script recreates it
        // byte-exactly; a missing destination is written without an attempted
        // removal.
        string memory dst = LibCopyArtifacts.committedPath(names[0]);
        bytes32 preDeletion = keccak256(bytes(vm.readFile(dst)));
        //forge-lint: disable-next-line(unsafe-cheatcode)
        vm.removeFile(dst);
        assertFalse(vm.exists(dst));
        new CopyArtifacts().run();
        assertTrue(vm.exists(dst), string.concat(names[0], ": committed artifact not recreated"));
        assertEq(
            keccak256(bytes(vm.readFile(dst))),
            preDeletion,
            string.concat(names[0], ": recreated artifact does not match pre-deletion state")
        );
    }
}
