// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test, Vm} from "forge-std-1.16.2/src/Test.sol";
import {IMetaV1_2} from "src/interface/unstable/IMetaV1_2.sol";
import {TestMetaBoard} from "test/concrete/TestMetaBoard.sol";

/// @title EmitCalldataFixtureTest
/// @notice The rust half of this repo builds `emitMeta` calldata and the
/// solidity half decides whether it is acceptable, and until now nothing tested
/// that seam: each half was checked against its own idea of the bytes. That is
/// how `generate_emit_meta_calldata` came to build a bare cbor map, which
/// `LibMeta.checkMetaUnhashedV1` reverts `NotRainMetaV1` on, while both test
/// suites stayed green.
///
/// `crates/cli/tests/emit_calldata_fixture.rs` writes the calldata rust
/// actually produces to `test/fixtures/emit-calldata.json` and fails if the
/// committed copy is stale. This sends that calldata to a real metaboard, so
/// what accepts or rejects it is the contract rather than an assertion
/// restating what the encoder did.
contract EmitCalldataFixtureTest is Test {
    TestMetaBoard internal metaBoard;

    function setUp() external {
        metaBoard = new TestMetaBoard();
    }

    /// Every entry in the fixture is calldata a metaboard accepts, and emits
    /// verbatim as a single `MetaV1_2`. A case that reverts fails here rather
    /// than in production.
    /// @param key The fixture key naming the rust producer under test.
    function _checkFixtureCase(string memory key) internal {
        string memory json = vm.readFile("test/fixtures/emit-calldata.json");
        bytes memory callData = vm.parseJsonBytes(json, string.concat(".", key));

        vm.recordLogs();
        (bool success,) = address(metaBoard).call(callData);
        assertTrue(success, string.concat("metaboard rejected calldata for ", key));

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 1, string.concat("log count for ", key));
        assertEq(logs[0].topics[0], IMetaV1_2.MetaV1_2.selector, string.concat("topic for ", key));

        // The emitted meta is the calldata's own meta argument, carried
        // verbatim. Decoding the log against the calldata rather than against a
        // literal keeps this test about the seam and not about the encoding.
        (bytes32 subjectArg, bytes memory metaArg) = abi.decode(_args(callData), (bytes32, bytes));
        (address sender, bytes32 subject, bytes memory meta) =
            abi.decode(logs[0].data, (address, bytes32, bytes));
        assertEq(sender, address(this), string.concat("sender for ", key));
        assertEq(subject, subjectArg, string.concat("subject for ", key));
        assertEq(meta, metaArg, string.concat("meta for ", key));
    }

    /// The abi encoded arguments, with the four byte selector dropped.
    ///  callData Whole calldata as the fixture carries it.
    ///  out The arguments alone, decodable as `(bytes32, bytes)`.
    function _args(bytes memory callData) internal pure returns (bytes memory out) {
        out = new bytes(callData.length - 4);
        for (uint256 i = 0; i < out.length; i++) {
            out[i] = callData[i + 4];
        }
    }

    /// `generate_emit_meta_calldata`, the generic producer.
    function testGenericItemCalldataIsAcceptable() external {
        _checkFixtureCase("generic_item");
    }

    /// `generate_dotrain_source_emit_tx_data`, the dotrain producer.
    function testDotrainSourceCalldataIsAcceptable() external {
        _checkFixtureCase("dotrain_source");
    }
}
