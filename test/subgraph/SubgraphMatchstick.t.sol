// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.2/src/Test.sol";

/// @title SubgraphMatchstickTest
/// @notice The matchstick run held to compiling the sources that are in the
/// tree, rather than reusing a binary of sources that were.
///
/// Matchstick 0.6.0 compiles each suite to `tests/.bin/<suite>.wasm` and then
/// reuses that file: `Compiler::execute` compiles only when `--recompile` is
/// passed, when the wasm is absent, or when `is_source_modified` finds the test
/// file or a transitively imported file carrying an mtime strictly NEWER than
/// the wasm. An mtime is not a fact about content. Archive extraction that
/// preserves timestamps, a coarse timestamp granularity, or any ordering that
/// lands the wasm last all leave a cached binary that matchstick executes in
/// place of the mappings actually in the tree — a suite reporting passes for
/// code nothing ran (rainlanguage/rain.metadata#230).
///
/// That is not hypothetical here. `subgraph/tests/.bin/metaboard.wasm` was
/// tracked in git, last written in 2024, and still exports
/// `generated/.../MetaBoard#hash` — an entry point deleted from the sources,
/// along with its mocks, in 2026. The binary a fresh checkout landed was a
/// build of tests that no longer exist.
///
/// Two independent things have to hold for the cached path to be unreachable,
/// and neither implies the other, so both are checked. There is nothing to
/// reuse: no compiled binary is in git, and the directory they land in is
/// ignored so a local run cannot put one back. And nothing is reused anyway:
/// the docker run passes `--recompile`, so the decision never consults an
/// mtime at all.
///
/// Solidity in the `rainix-sol` lane rather than a check inside the matchstick
/// suite, deliberately: the question is whether the matchstick lane executes
/// the current sources at all, and a check that ships inside the cached binary
/// is answered by the stale build along with everything else. It needs no
/// docker, so it runs on every push.
contract SubgraphMatchstickTest is Test {
    /// The compose file `rainix`'s `subgraph-test` task brings up. That task is
    /// `npm ci && docker compose up --abort-on-container-exit` from
    /// `subgraph/`, so this file is the whole of how matchstick is invoked in
    /// CI.
    string constant DOCKER_COMPOSE_YML = "subgraph/docker-compose.yml";

    /// JSON path of the environment variable the matchstick image expands into
    /// its own command line. The image's `CMD` is `/binary-linux-22 ${ARGS}`
    /// with `ENV ARGS=` empty, so `ARGS` is the only place a flag can be put
    /// without overriding the command outright.
    string constant MATCHSTICK_ARGS = ".services.matchstick.environment.ARGS";

    /// The flag that makes compilation unconditional. `-r` is matchstick's
    /// short spelling of `--recompile`; pinned as an exact value rather than
    /// searched for, so respelling it or appending to it is a change that has
    /// to be made here too.
    string constant RECOMPILE_FLAG = "-r";

    /// The directory matchstick writes compiled suites into, from the repo
    /// root. `.bin` under the tests folder is matchstick's own layout and
    /// `subgraph/` carries no `matchstick.yaml`, so the default tests folder
    /// `tests` applies and this is where the wasm lands.
    string constant MATCHSTICK_BIN = "subgraph/tests/.bin";

    /// A file under `subgraph/tests/` that IS tracked.
    ///
    /// Named so that the tracked-file check has a non-empty expected result. An
    /// empty `git ls-files` is what "no binary is tracked" looks like, and it
    /// is also what a `git` that failed to run looks like, so asserting against
    /// emptiness would pass in the one case this test exists to catch itself
    /// being broken by. Asking for this path in the same query makes the
    /// expected output a value the command has to produce.
    string constant TRACKED_TEST_SOURCE = "subgraph/tests/metaBoard.test.ts";

    /// No compiled matchstick binary is in git, and none can be added back.
    ///
    /// Tracked and ignored are separate claims. Deleting the wasm without the
    /// ignore rule leaves every local `subgraph-test` run rewriting an
    /// untracked file that the next `git add -A` commits again. The ignore rule
    /// without the deletion leaves the stale binary exactly where it was, since
    /// ignore rules do not apply to tracked paths.
    function testMatchstickBinariesAreNotInGit() external {
        string[] memory cmd = new string[](5);
        cmd[0] = "git";
        cmd[1] = "ls-files";
        cmd[2] = "--";
        cmd[3] = MATCHSTICK_BIN;
        cmd[4] = TRACKED_TEST_SOURCE;
        assertEq(
            string(vm.ffi(cmd)),
            TRACKED_TEST_SOURCE,
            string.concat(
                "git tracks a compiled matchstick binary under ",
                MATCHSTICK_BIN,
                ", or no longer tracks ",
                TRACKED_TEST_SOURCE
            )
        );

        cmd = new string[](4);
        cmd[0] = "git";
        cmd[1] = "check-ignore";
        cmd[2] = "--no-index";
        cmd[3] = MATCHSTICK_BIN;
        assertEq(
            string(vm.ffi(cmd)),
            MATCHSTICK_BIN,
            string.concat(MATCHSTICK_BIN, " is not gitignored; a local matchstick run leaves its binary stageable")
        );
    }

    /// The docker matchstick run recompiles unconditionally.
    ///
    /// Read as parsed YAML rather than as text for the reason
    /// `SubgraphManifest.t.sol` gives: a commented out `# ARGS: -r` satisfies a
    /// text search while the compose parser never sees it, which is exactly the
    /// way this protection would be lost.
    function testMatchstickRunForcesRecompilation() external {
        string[] memory cmd = new string[](4);
        cmd[0] = "yq";
        cmd[1] = "-o=json";
        cmd[2] = ".";
        cmd[3] = DOCKER_COMPOSE_YML;
        string memory json = string(vm.ffi(cmd));

        assertTrue(
            vm.keyExistsJson(json, MATCHSTICK_ARGS),
            string.concat(DOCKER_COMPOSE_YML, " sets no ", MATCHSTICK_ARGS, "; matchstick then falls back to mtimes")
        );
        assertEq(
            vm.parseJsonString(json, MATCHSTICK_ARGS),
            RECOMPILE_FLAG,
            string.concat(
                DOCKER_COMPOSE_YML, " does not pass ", RECOMPILE_FLAG, " to matchstick; it then falls back to mtimes"
            )
        );
    }
}
