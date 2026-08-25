// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 thedavidmeister
pragma solidity =0.8.25;

import {Test, Vm} from "forge-std-1.16.2/src/Test.sol";
import {LibCopyArtifacts} from "script/lib/LibCopyArtifacts.sol";

contract CopyArtifactsTest is Test {
    /// The directory the committed artifacts live in, spelled as its own
    /// literal rather than derived from `committedPath`, so the assertions
    /// about what the directory holds are independent of the code that
    /// decides where to write.
    string constant COMMITTED_ABI_DIR = "crates/bindings/abi";

    /// The artifacts `crates/bindings/src/lib.rs` consumes via `alloy::sol!`,
    /// spelled as literals. That file is the CONSUMER of everything
    /// `LibCopyArtifacts` maintains, and it is rust, so the sol lane cannot
    /// read it as an oracle; the names are restated here instead, the same way
    /// `SubgraphManifest.t.sol` restates the indexed event signature. Changing
    /// the consumed set means changing lib.rs, `contracts()` and this list
    /// together.
    function consumedArtifacts() internal pure returns (string[2] memory) {
        return ["IMetaBoardV1_2", "IDescribedByMetaV1"];
    }

    /// `jq` over `vm.ffi`, the same shape `LibCopyArtifacts.extractStable`
    /// reads the live artifact in — but with its own filters, so nothing
    /// asserted here can drift in lockstep with the filter under test.
    /// `-c` keeps the output one line and, unlike `-r`, keeps string results
    /// quoted so a `0x…` value cannot be hex-decoded by `vm.ffi` on the way
    /// back.
    /// @param filter The jq filter to evaluate.
    /// @param file The JSON file to evaluate it against.
    /// @return The evaluated output.
    function jq(string memory filter, string memory file) internal returns (string memory) {
        string[] memory cmd = new string[](4);
        cmd[0] = "jq";
        cmd[1] = "-c";
        cmd[2] = filter;
        cmd[3] = file;
        return string(vm.ffi(cmd));
    }

    /// `contracts()` and the committed directory MUST name each other exactly.
    ///
    /// The `copy-artifacts` CI job proves every committed artifact is
    /// FRESH, which cannot see a name DROPPED from the list: the committed
    /// copy stays behind as an orphan the copy script no longer maintains,
    /// silently going stale under the rust crate that still compiles against
    /// it. So the set is held from both ends: every consumed name is in
    /// `contracts()` exactly once, every listed name has its committed file,
    /// and the directory holds nothing else.
    function testCommittedAbiDirMatchesContracts() external {
        string[] memory names = LibCopyArtifacts.contracts();
        string[2] memory consumed = consumedArtifacts();

        assertEq(names.length, consumed.length, "contracts() does not name every consumed artifact exactly once");
        for (uint256 i = 0; i < consumed.length; i++) {
            uint256 occurrences = 0;
            for (uint256 j = 0; j < names.length; j++) {
                if (keccak256(bytes(names[j])) == keccak256(bytes(consumed[i]))) {
                    occurrences++;
                }
            }
            assertEq(
                occurrences,
                1,
                string.concat(consumed[i], ": consumed by crates/bindings/src/lib.rs, named by contracts() once")
            );
        }

        for (uint256 i = 0; i < names.length; i++) {
            assertTrue(
                vm.exists(LibCopyArtifacts.committedPath(names[i])),
                string.concat(names[i], ": no committed artifact at committedPath")
            );
        }

        Vm.DirEntry[] memory entries = vm.readDir(COMMITTED_ABI_DIR);
        assertEq(
            entries.length,
            names.length,
            string.concat(COMMITTED_ABI_DIR, " holds a file `contracts()` does not name; orphans are never regenerated")
        );
    }

    /// The committed artifact MUST be exactly the stable subset of the live
    /// one: top level keys `abi`, `bytecode`, `deployedBytecode` and nothing
    /// else, the two bytecode objects reduced to `object` alone, and every
    /// kept value equal to the live artifact's.
    ///
    /// The `copy-artifacts` CI job regenerates and compares the committed copy against
    /// `extractStable`, so a filter that drifts — keeping the whole
    /// non-deterministic bytecode object, dropping the `abi` key
    /// `alloy::sol!` reads — regenerates and re-reads its own drift and stays
    /// green. Here both sides are read with this file's own jq spellings and
    /// the SHAPE is pinned to literals, so the filter has nothing to agree
    /// with but the intent.
    function testCommittedArtifactIsTheStableSubset() external {
        string[] memory names = LibCopyArtifacts.contracts();
        for (uint256 i = 0; i < names.length; i++) {
            string memory committed = LibCopyArtifacts.committedPath(names[i]);
            string memory live = LibCopyArtifacts.livePath(names[i]);

            assertEq(
                jq("keys", committed),
                "[\"abi\",\"bytecode\",\"deployedBytecode\"]",
                string.concat(committed, ": top level keys are not exactly the stable subset")
            );
            assertEq(
                jq(".bytecode | keys", committed),
                "[\"object\"]",
                string.concat(committed, ": bytecode carries more than `object`, which is not deterministic")
            );
            assertEq(
                jq(".deployedBytecode | keys", committed),
                "[\"object\"]",
                string.concat(committed, ": deployedBytecode carries more than `object`, which is not deterministic")
            );
            assertEq(
                jq(".abi", committed),
                jq(".abi", live),
                string.concat(committed, ": abi differs from the live artifact's")
            );
            assertEq(
                jq(".bytecode.object", committed),
                jq(".bytecode.object", live),
                string.concat(committed, ": bytecode.object differs from the live artifact's")
            );
            assertEq(
                jq(".deployedBytecode.object", committed),
                jq(".deployedBytecode.object", live),
                string.concat(committed, ": deployedBytecode.object differs from the live artifact's")
            );
        }
    }
}
