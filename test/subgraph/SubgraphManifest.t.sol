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
/// The manifest is read as YAML rather than as text. A substring search over
/// the raw file answers a different question — "does this run of characters
/// appear anywhere" — and the two answers come apart in both directions: a
/// commented-out `# - event: …` satisfies a positive search while the parser
/// never sees it, and an `address :`, a `"address":` or a flow mapping defeats
/// a negative search while the parser reads the key perfectly well. So `yq`
/// over `vm.ffi` renders the document as JSON and every assertion below names
/// the PATH of the node it is about.
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

    /// The schema the mapping stores into, and the other side of the
    /// `mapping.entities` check.
    string constant SCHEMA_GRAPHQL = "subgraph/schema.graphql";

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

    /// The placeholder `network:` the template carries.
    ///
    /// A name rather than an absence, because `network:` is required: the
    /// manifest does not parse without it. So the check that no real network
    /// has settled here cannot be "the field is missing" and cannot be "the
    /// field is not one of the deployed networks" either — that list is
    /// `networks.json` and it is in rain.metadata.deploy. It is that the field
    /// still holds the one value that is deliberately not a chain.
    string constant TEMPLATE_NETWORK = "template";

    /// JSON path of the manifest's one data source.
    ///
    /// `[0]` rather than a search because `manifestJson` pins the list to a
    /// single entry, which is what makes the index mean THE data source.
    string constant THE_DATA_SOURCE = ".dataSources[0]";

    /// Every node in the document keyed `address` or `startBlock`, by path.
    ///
    /// Document wide rather than rooted at `THE_DATA_SOURCE` because these two
    /// must be absent EVERYWHERE — a `templates:` entry has a `source:` block
    /// of its own, and a fact parked on one is the same residue as a fact on
    /// the data source. `key ==` rather than a text search is the whole point:
    /// `yq` has already resolved `address :`, `"address":` and a flow mapping
    /// to the same key by the time this runs, and has already dropped every
    /// comment.
    ///
    /// Reported as paths rather than counted so a failure names where the fact
    /// is. Pipe separated and pipe delimited at both ends for the same reason
    /// `indexedArtifactEventSignatures` is: `vm.ffi` trims its output, which
    /// would eat a space delimiter, and an undelimited empty list is
    /// indistinguishable from an empty run of output.
    string constant DEPLOYMENT_FACT_PATHS = "[.. | select(key == \"address\" or key == \"startBlock\") | path"
        " | join(\".\")] | \"|\" + join(\"|\") + \"|\"";

    /// An empty path list, as `DEPLOYMENT_FACT_PATHS` and
    /// `networkResidueExpression` both encode one: the two delimiters with
    /// nothing between them.
    string constant NO_PATHS = "||";

    /// `yq` over `vm.ffi`, the same shape `LibCopyArtifacts` already reads the
    /// forge artifact with `jq` in: the parsing is the shell tool's, and what
    /// comes back is compared in Solidity against values that are Solidity
    /// already.
    /// @param outputFormat The `-o=` flag. JSON when the whole document is
    /// wanted for the forge JSON cheatcodes; YAML when the expression's own
    /// result is a scalar, which `yq` then emits unwrapped.
    /// @param expression The `yq` expression to evaluate against the manifest.
    /// @return The evaluated output.
    function yq(string memory outputFormat, string memory expression) internal returns (string memory) {
        string[] memory cmd = new string[](4);
        cmd[0] = "yq";
        cmd[1] = outputFormat;
        cmd[2] = expression;
        cmd[3] = SUBGRAPH_YAML;
        return string(vm.ffi(cmd));
    }

    /// The manifest as JSON, with `dataSources` pinned to a single entry.
    ///
    /// The pin lives here rather than in one test because every path below
    /// indexes `dataSources[0]`, and `[0]` only means THE data source while
    /// there is exactly one. A second entry — a second network, a second
    /// address, a second handler — would otherwise sit entirely unread beside
    /// the one every assertion looks at.
    /// @return The manifest, rendered as JSON.
    function manifestJson() internal returns (string memory) {
        string memory json = yq("-o=json", ".");
        assertTrue(vm.keyExistsJson(json, THE_DATA_SOURCE), "subgraph.yaml declares no data source");
        assertFalse(
            vm.keyExistsJson(json, ".dataSources[1]"),
            "subgraph.yaml declares a second data source; every check here is written about the one"
        );
        return json;
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
    /// Read at `mapping.eventHandlers`, which is the only place a handler
    /// declaration is one: the signature appearing anywhere else in the file —
    /// in a comment, in an entity name, in a second data source — is not a
    /// declaration and must not satisfy this. The list is pinned to a single
    /// entry for the same reason `dataSources` is, so `[0]` is the handler and
    /// not merely one of them.
    ///
    /// Then two assertions, because `keccak256` is one way. The manifest
    /// declares a signature STRING; the interface exposes only the topic it
    /// hashes to. Pinning the string to the manifest and the hash to the
    /// interface leaves no way for the pair to be satisfied by an event this
    /// repo does not declare: change the manifest and the first fails, change
    /// the interface and the second does.
    function testManifestIndexesTheInterfaceEvent() external {
        string memory json = manifestJson();

        assertTrue(
            vm.keyExistsJson(json, string.concat(THE_DATA_SOURCE, ".mapping.eventHandlers[0]")),
            "subgraph.yaml declares no event handler"
        );
        assertFalse(
            vm.keyExistsJson(json, string.concat(THE_DATA_SOURCE, ".mapping.eventHandlers[1]")),
            "subgraph.yaml declares a second event handler; the manifest indexes exactly one event"
        );
        assertEq(
            vm.parseJsonString(json, string.concat(THE_DATA_SOURCE, ".mapping.eventHandlers[0].event")),
            INDEXED_EVENT_SIGNATURE,
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
    /// `source.abi` is a NAME, and the file the data source decodes with is
    /// whichever `mapping.abis` entry carries that name. Reading the path off
    /// entry zero would be checking a file the data source need not be wired
    /// to at all, so the entry is RESOLVED by name and the path read off that
    /// one. A `source.abi` naming no entry fails here rather than in the docker
    /// lane.
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
    function testManifestAbiIsAnArtifactThisRepoPublishes() external {
        string memory json = manifestJson();

        string memory wiredName = vm.parseJsonString(json, string.concat(THE_DATA_SOURCE, ".source.abi"));
        bool resolved = false;
        string memory wiredFile;
        for (uint256 i = 0; vm.keyExistsJson(json, _abiEntry(i)); i++) {
            if (
                keccak256(bytes(vm.parseJsonString(json, string.concat(_abiEntry(i), ".name"))))
                    == keccak256(bytes(wiredName))
            ) {
                assertFalse(resolved, string.concat("subgraph.yaml's mapping.abis names ", wiredName, " twice"));
                resolved = true;
                wiredFile = vm.parseJsonString(json, string.concat(_abiEntry(i), ".file"));
            }
        }
        assertTrue(
            resolved,
            string.concat("subgraph.yaml's source.abi is ", wiredName, ", which names no entry in mapping.abis")
        );
        assertEq(
            wiredFile,
            string.concat("../", LibCopyArtifacts.livePath(INDEXED_INTERFACE)),
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

    /// JSON path of one `mapping.abis` entry of the manifest's data source.
    /// @param index The entry's index in the list.
    /// @return The path.
    function _abiEntry(uint256 index) internal pure returns (string memory) {
        return string.concat(THE_DATA_SOURCE, ".mapping.abis[", vm.toString(index), "]");
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

    /// The schema's `@entity` declarations, as JSON: `types` are the entity
    /// type names, `declarations` is how many lines declare one.
    ///
    /// A LINE read, which every manifest check here is forbidden from being,
    /// because there is no schema parser to reach for: this lane deliberately
    /// has no node, and `yq` does not speak GraphQL. `declarations` is what
    /// makes that safe. It counts every non-comment line mentioning `@entity`
    /// at all, and `schemaEntityTypes` holds `types` to that count, so a schema
    /// written in a shape this cannot read — a directive on its own line, a
    /// description carrying the word — fails loudly rather than silently
    /// reporting fewer entities than the file declares. Comment lines are
    /// dropped first, since a commented-out declaration is not one, which is
    /// the same distinction reading the manifest as YAML gets for free.
    /// @return The `{declarations, types}` object as JSON.
    function schemaEntityJson() internal returns (string memory) {
        string[] memory cmd = new string[](4);
        cmd[0] = "jq";
        cmd[1] = "-Rn";
        cmd[2] = "[inputs | select(test(\"^[[:space:]]*#\") | not) | select(test(\"@entity\"))]"
            " | {declarations: length, types: [.[]"
            " | scan(\"^type[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)[[:space:]]+@entity\") | .[0]]}";
        cmd[3] = SCHEMA_GRAPHQL;
        return string(vm.ffi(cmd));
    }

    /// Every type the schema declares `@entity`.
    /// @return The entity type names.
    function schemaEntityTypes() internal returns (string[] memory) {
        string memory json = schemaEntityJson();
        string[] memory types = vm.parseJsonStringArray(json, ".types");
        assertEq(
            types.length,
            vm.parseJsonUint(json, ".declarations"),
            "schema.graphql declares @entity on a line that is not a type declaration this can read"
        );
        return types;
    }

    /// The manifest MUST declare the entities the mapping stores.
    ///
    /// `mapping.entities` is the manifest's claim about which schema entities
    /// the handler writes. Nothing downstream contradicts a stale one —
    /// graph-node does not enforce the list at runtime and `graph codegen` does
    /// not read it — which is how `Transaction`, stored by
    /// `createTransactionEntity` on every event, sat unlisted
    /// (rainlanguage/rain.metadata#151).
    ///
    /// The schema is the other side, rather than a list restated here, because
    /// it is a separate file that codegen does read, and because
    /// `dataSources` is pinned to one entry: there is exactly one mapping for
    /// its entities to belong to, so an entity the schema declares is one that
    /// mapping stores or dead schema. Both directions are asserted and neither
    /// implies the other: a schema entity the manifest omits is #151 again, and
    /// a manifest entity the schema does not declare is a name `graph build`
    /// rejects.
    function testManifestDeclaresTheEntitiesTheMappingStores() external {
        string memory json = manifestJson();
        string memory entitiesPath = string.concat(THE_DATA_SOURCE, ".mapping.entities");
        assertTrue(vm.keyExistsJson(json, entitiesPath), "subgraph.yaml declares no mapping.entities");

        string[] memory declared = vm.parseJsonStringArray(json, entitiesPath);
        string[] memory schema = schemaEntityTypes();

        for (uint256 i = 0; i < schema.length; i++) {
            assertTrue(
                _holds(declared, schema[i]),
                string.concat(
                    "subgraph.yaml's mapping.entities omits ", schema[i], ", which schema.graphql declares @entity"
                )
            );
        }
        for (uint256 i = 0; i < declared.length; i++) {
            assertTrue(
                _holds(schema, declared[i]),
                string.concat(
                    "subgraph.yaml's mapping.entities declares ",
                    declared[i],
                    ", which schema.graphql declares no @entity type for"
                )
            );
        }
        assertEq(
            declared.length,
            schema.length,
            "subgraph.yaml's mapping.entities and schema.graphql name the same entities, one of them twice"
        );
    }

    /// @param names The names to search.
    /// @param name The name to find.
    /// @return Whether the list holds the name.
    function _holds(string[] memory names, string memory name) internal pure returns (bool) {
        for (uint256 i = 0; i < names.length; i++) {
            if (keccak256(bytes(names[i])) == keccak256(bytes(name))) {
                return true;
            }
        }
        return false;
    }

    /// Every node in the document keyed `network` that is not the placeholder,
    /// by path.
    ///
    /// Document wide for the same reason `DEPLOYMENT_FACT_PATHS` is: a
    /// `templates:` entry carries a `network:` of its own, `networks.json` can
    /// name it, and `graph build --network` writes the chain into that one too.
    /// A residue parked there is the same residue.
    ///
    /// Built rather than declared so `TEMPLATE_NETWORK` is spelled once. An
    /// empty result means every `network:` in the document is the placeholder —
    /// and also means there is no `network:` at all, which is why the data
    /// source's own is read at its path as well.
    /// @return The `yq` expression.
    function networkResidueExpression() internal pure returns (string memory) {
        return string.concat(
            "[.. | select(key == \"network\" and . != \"",
            TEMPLATE_NETWORK,
            "\") | path | join(\".\")] | \"|\" + join(\"|\") + \"|\""
        );
    }

    /// The manifest MUST carry no deployment fact.
    ///
    /// `graph build --network <x>` fills `address`, `startBlock` AND `network`
    /// from `networks.json` — and writes all three back into the SOURCE
    /// manifest, not only into `build/`. So a manifest carrying any of them is
    /// either a deployment record that has drifted into the library half, or
    /// the residue of whichever network happened to sort last in the most
    /// recent build. Before #149 it was the latter, indistinguishable from the
    /// former by inspection, and nothing checked.
    ///
    /// `address` and `startBlock` are rejected as PARSED keys, anywhere in the
    /// document, and the failure names the path each was found at.
    ///
    /// `network:` is the one of the three that has to be present, so it is
    /// pinned to the placeholder rather than asserted absent. It is checked for
    /// the same reason as the other two and not as a lesser case: a real chain
    /// name sitting in a template is read as a default by everyone downstream
    /// of it, and `--network` overrides it silently, so nothing else would ever
    /// contradict it. Two assertions, because presence and absence of a residue
    /// are different claims and neither implies the other: every `network:` in
    /// the document is the placeholder, document wide so a `templates:` entry
    /// is covered; and the data source's own is read AT its path, which is what
    /// makes the document-wide list empty by placeholder rather than by there
    /// being no `network:` to find. Both name their node rather than searching
    /// the text — a `# network: template` left above a live `network: matic`
    /// satisfies a search and is exactly the write-back this is here to catch.
    ///
    /// A `graph build --network` run in this tree therefore fails HERE rather
    /// than silently committing a network's address the next time someone runs
    /// `git add -A`.
    function testManifestSourceCarriesNoDeploymentFact() external {
        assertEq(
            yq("-o=yaml", DEPLOYMENT_FACT_PATHS),
            NO_PATHS,
            "subgraph.yaml carries a deployment fact at the paths listed; networks.json is the"
            " deployment record and it is not in this repo"
        );
        assertEq(
            yq("-o=yaml", networkResidueExpression()),
            NO_PATHS,
            string.concat(
                "subgraph.yaml carries a network other than the placeholder ",
                TEMPLATE_NETWORK,
                " at the paths listed; networks.json is the deployment record and it is not in this repo"
            )
        );
        assertEq(
            vm.parseJsonString(manifestJson(), string.concat(THE_DATA_SOURCE, ".network")),
            TEMPLATE_NETWORK,
            string.concat(
                "subgraph.yaml's network is not the placeholder ",
                TEMPLATE_NETWORK,
                "; networks.json is the deployment record and it is not in this repo"
            )
        );
    }
}
