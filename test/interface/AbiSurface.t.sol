// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {LibCopyArtifacts} from "script/lib/LibCopyArtifacts.sol";

/// @title AbiSurfaceTest
/// @notice The published interfaces' ABI surface, read from the BUILT
/// artifacts the way the consumers read it — `alloy::sol!` compiles the
/// committed copy of these artifacts into the rust bindings, `graph codegen`
/// decodes event layouts out of the live one — and pinned to literals.
///
/// Renaming or re-typing a member breaks in-tree callers at compile time, but
/// the artifact carries surface those callers cannot hold still: whether an
/// event input is INDEXED (which moves it from the data section into a topic
/// and re-layouts every log for every decoder) and a function's state
/// mutability (which decides whether tooling may `eth_call` it). Both are
/// invisible to the selector and to Solidity call sites, so they are pinned
/// here against the artifact itself.
contract AbiSurfaceTest is Test {
    /// `jq` over `vm.ffi`, the same shape `LibCopyArtifacts` and
    /// `SubgraphManifest.t.sol` already read the artifacts in. `-c` keeps the
    /// result one line; results here all open with `[`, so `vm.ffi` cannot
    /// mistake them for hex.
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

    /// Every function an artifact declares, reduced to the surface a consumer
    /// binds against: name, mutability, input types, output types.
    /// @param contractName The artifact to read, by `LibCopyArtifacts.livePath`.
    /// @return The function list as one line of JSON.
    function functionSurface(string memory contractName) internal returns (string memory) {
        return jq(
            "[.abi[] | select(.type==\"function\")]"
            " | map({name, stateMutability, inputs: [.inputs[].type], outputs: [.outputs[].type]})",
            LibCopyArtifacts.livePath(contractName)
        );
    }

    /// `MetaV1_2` keeps all three inputs UNINDEXED, deliberately — the
    /// interface marks the slither `unindexed-event-address` warning as
    /// intended. The subgraph handler reads `sender`, `subject` and `meta`
    /// out of the data section as the artifact lays it out; indexing one
    /// would silently re-layout every log under a decoder built from the old
    /// artifact.
    function testMetaV1_2DeclaresNothingIndexed() external {
        assertEq(
            jq(
                "[.abi[] | select(.type==\"event\" and .name==\"MetaV1_2\") | .inputs[] | .indexed]",
                LibCopyArtifacts.livePath("IMetaBoardV1_2")
            ),
            "[false,false,false]",
            "MetaV1_2 must carry sender, subject and meta unindexed, in the data section"
        );
    }

    /// The whole of `IMetaBoardV1_2`'s function surface:
    /// `emitMeta(bytes32,bytes)`, nonpayable, returning nothing.
    function testIMetaBoardV1_2FunctionSurface() external {
        assertEq(
            functionSurface("IMetaBoardV1_2"),
            "[{\"name\":\"emitMeta\",\"stateMutability\":\"nonpayable\",\"inputs\":[\"bytes32\",\"bytes\"],"
            "\"outputs\":[]}]",
            "IMetaBoardV1_2's function surface is exactly emitMeta(bytes32,bytes) nonpayable"
        );
    }

    /// The whole of `IDescribedByMetaV1`'s function surface:
    /// `describedByMetaV1()`, VIEW, returning the described hash. `view` is
    /// surface: it is what lets tooling `eth_call` a described contract for
    /// its hash, and `IDescribedByMetaV1` requires the hash never to change,
    /// so nothing about answering may write.
    function testIDescribedByMetaV1FunctionSurface() external {
        assertEq(
            functionSurface("IDescribedByMetaV1"),
            "[{\"name\":\"describedByMetaV1\",\"stateMutability\":\"view\",\"inputs\":[]," "\"outputs\":[\"bytes32\"]}]",
            "IDescribedByMetaV1's function surface is exactly describedByMetaV1() view returns (bytes32)"
        );
    }
}
