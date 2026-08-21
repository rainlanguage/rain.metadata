// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";
import {IMetaV1_2} from "src/interface/unstable/IMetaV1_2.sol";
import {LibCopyArtifacts} from "script/lib/LibCopyArtifacts.sol";

/// @title SubgraphManifestTest
/// @notice `subgraph/subgraph.yaml` held against the interface it indexes and
/// against the artifact record this repo already keeps.
///
/// rainlanguage/rain.metadata#149 splits the subgraph on whether a file carries
/// a DEPLOYMENT FACT. `networks.json` — addresses and start blocks per network
/// — went to rain.metadata.deploy. The manifest stayed, with its `source:`
/// block reduced to `abi: MetaBoard`, because everything left in it is a claim
/// about SOURCE in this tree: which interface's ABI is decoded, which event
/// signature is decoded out of it, which mapping handles it.
///
/// Those claims had nothing checking them here before, and the ABI is the one
/// coupling `graph codegen` only half catches: a renamed event breaks the
/// build, but a RE-TYPED parameter still codegens, still compiles, and produces
/// a handler decoding the wrong layout against a live chain.
///
/// This is Solidity in the `rainix-sol` lane deliberately. The things the
/// manifest is compared against — `IMetaV1_2.MetaV1_2.selector` and
/// `LibCopyArtifacts.contracts()` — ARE Solidity, so a check written elsewhere
/// would have to re-spell them and could then disagree with them. It needs no
/// docker and no node, so it runs on every push rather than only in the
/// matchstick lane.
contract SubgraphManifestTest is Test {
    /// The subgraph manifest, as a template: no deployment fact in it.
    string constant SUBGRAPH_YAML = "subgraph/subgraph.yaml";

    /// The interface whose ABI the manifest decodes with. Named rather than
    /// read out of the manifest, so that the manifest is compared to an
    /// expectation instead of to itself.
    string constant INDEXED_INTERFACE = "IMetaBoardV1_2";

    /// The event the subgraph indexes, as a signature.
    ///
    /// Spelled here because `keccak256` is one way: proving the manifest
    /// declares THIS interface's event needs the preimage written down
    /// somewhere, and here is the one place a mutation to either side is
    /// caught.
    string constant INDEXED_EVENT_SIGNATURE = "MetaV1_2(address,bytes32,bytes)";

    /// The manifest, read once per test.
    /// @return The file contents.
    function manifest() internal view returns (string memory) {
        return vm.readFile(SUBGRAPH_YAML);
    }

    /// Every event signature the indexed artifact's ABI declares, pipe
    /// separated and pipe delimited at both ends.
    ///
    /// Derived from the BUILT artifact rather than from the interface source,
    /// because the artifact is what `graph codegen` actually reads — a
    /// signature this cannot see is a signature codegen cannot see either.
    /// `jq` over `vm.ffi` is how `LibCopyArtifacts` already reads that file.
    ///
    /// Delimited rather than bare so a membership test cannot be satisfied by a
    /// longer signature that merely ends with the one being looked for. `|`
    /// rather than a space because `vm.ffi` trims the output it returns, which
    /// would eat a space delimiter at both ends and break the first and last
    /// signature in the list.
    /// @return The delimited signature list.
    function indexedArtifactEventSignatures() internal returns (string memory) {
        string[] memory cmd = new string[](4);
        cmd[0] = "jq";
        cmd[1] = "-r";
        cmd[2] = string.concat(
            "[.abi[] | select(.type==\"event\") | .name + \"(\" + ([.inputs[].type] | join(\",\")) + \")\"]",
            " | \"|\" + join(\"|\") + \"|\""
        );
        cmd[3] = LibCopyArtifacts.livePath(INDEXED_INTERFACE);
        return string(vm.ffi(cmd));
    }

    /// The manifest MUST index the interface's event.
    ///
    /// Two assertions, because `keccak256` is one way. The manifest declares a
    /// signature STRING; the interface exposes only the topic it hashes to.
    /// Pinning the string to the manifest and the hash to the interface leaves
    /// no way for the pair to be satisfied by an event this repo does not
    /// declare: change the manifest and the first fails, change the interface
    /// and the second does.
    function testManifestIndexesTheInterfaceEvent() external view {
        assertTrue(
            vm.contains(manifest(), string.concat("event: ", INDEXED_EVENT_SIGNATURE)),
            "subgraph.yaml does not index the expected event signature"
        );
        assertEq(
            keccak256(bytes(INDEXED_EVENT_SIGNATURE)),
            IMetaV1_2.MetaV1_2.selector,
            "the indexed signature is not the interface's MetaV1_2 topic"
        );
    }

    /// The manifest's ABI MUST be an artifact this repo publishes.
    ///
    /// The path is DERIVED from `LibCopyArtifacts.livePath`, which is the same
    /// function `script/CopyArtifacts.sol` uses to find what it copies for the
    /// rust bindings, so the manifest follows the forge artifact layout instead
    /// of restating it. The `../` prefix is the manifest's own directory: the
    /// manifest resolves relative paths from `subgraph/`, the artifact tree is
    /// at the repo root.
    ///
    /// And the interface MUST be one `contracts()` names. That list is the
    /// repo's declaration of which artifacts are consumed outside the Solidity
    /// build at all — dropping the interface from it while the manifest still
    /// reads the file would leave the subgraph depending on an artifact nothing
    /// says is load bearing.
    function testManifestAbiIsAnArtifactThisRepoPublishes() external view {
        assertTrue(
            vm.contains(manifest(), string.concat("file: ../", LibCopyArtifacts.livePath(INDEXED_INTERFACE))),
            string.concat("subgraph.yaml does not read its ABI from ../", LibCopyArtifacts.livePath(INDEXED_INTERFACE))
        );

        string[] memory published = LibCopyArtifacts.contracts();
        bool isPublished = false;
        for (uint256 i = 0; i < published.length; i++) {
            if (keccak256(bytes(published[i])) == keccak256(bytes(INDEXED_INTERFACE))) {
                isPublished = true;
                break;
            }
        }
        assertTrue(isPublished, string.concat(INDEXED_INTERFACE, " is not one of LibCopyArtifacts.contracts()"));
    }

    /// The indexed artifact MUST actually declare the indexed event.
    ///
    /// This is the half `graph codegen` does not catch. Codegen resolves the
    /// event by NAME, so renaming it breaks the build loudly, but re-typing a
    /// parameter generates a handler that compiles and then decodes the wrong
    /// layout out of every log it is handed. Comparing full signatures — name
    /// AND parameter types — is what makes the re-type visible.
    function testIndexedArtifactDeclaresTheIndexedEvent() external {
        assertTrue(
            vm.contains(indexedArtifactEventSignatures(), string.concat("|", INDEXED_EVENT_SIGNATURE, "|")),
            string.concat(
                LibCopyArtifacts.livePath(INDEXED_INTERFACE),
                " declares no ",
                INDEXED_EVENT_SIGNATURE,
                " for graph codegen to read"
            )
        );
    }

    /// The manifest MUST carry no deployment fact.
    ///
    /// `graph build --network <x>` fills `address` and `startBlock` from
    /// `networks.json` — and writes them back into the SOURCE manifest, not
    /// only into `build/`. So a manifest with either field in it is either a
    /// deployment record that has drifted into the library half, or the residue
    /// of whichever network happened to sort last in the most recent build.
    /// Before #149 it was the latter, indistinguishable from the former by
    /// inspection, and nothing checked.
    ///
    /// A `graph build --network` run in this tree therefore fails HERE rather
    /// than silently committing a network's address the next time someone runs
    /// `git add -A`.
    function testManifestSourceCarriesNoDeploymentFact() external view {
        string memory yaml = manifest();
        assertFalse(
            vm.contains(yaml, "address:"),
            "subgraph.yaml names an address; networks.json is the deployment record and it is not in this repo"
        );
        assertFalse(
            vm.contains(yaml, "startBlock:"),
            "subgraph.yaml names a startBlock; networks.json is the deployment record and it is not in this repo"
        );
    }
}
