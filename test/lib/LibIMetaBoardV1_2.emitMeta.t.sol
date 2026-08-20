// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test, Vm} from "forge-std-1.16.2/src/Test.sol";
import {LibIMetaBoardV1_2} from "src/lib/LibIMetaBoardV1_2.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {IMetaV1_2, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";

/// @title LibIMetaBoardV1_2EmitMetaTest
/// @notice Unit tests for `LibIMetaBoardV1_2.emitMeta`, which is the whole of
/// the `IMetaBoardV1_2.emitMeta` entry point as library logic. The library is
/// reached through an external wrapper on this test contract rather than
/// called inline, so `msg.sender` is a real caller `vm.prank` can set and a
/// revert is a real revert: both are properties of the entry point that an
/// inline call from the test body could not observe.
contract LibIMetaBoardV1_2EmitMetaTest is Test, IMetaV1_2 {
    /// The library under test behind an external call boundary.
    /// @param subject As per `IMetaV1_2`.
    /// @param meta As per `IMetaV1_2`.
    function emitMetaExternal(bytes32 subject, bytes memory meta) external {
        LibIMetaBoardV1_2.emitMeta(subject, meta);
    }

    /// Metadata prefixed with the rain magic number is emitted verbatim as a
    /// single unindexed `MetaV1_2`, attributed to the CALLER of the contract
    /// the library is inlined into rather than to that contract, and emitted
    /// BY that contract rather than by the library.
    /// @param sender The pranked caller the event must be attributed to.
    /// @param subject The subject to emit.
    /// @param data Arbitrary bytes to carry after the magic number.
    function testEmitMetaHappy(address sender, bytes32 subject, bytes memory data) external {
        vm.assume(sender != address(vm));
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);

        vm.recordLogs();
        vm.prank(sender);
        this.emitMetaExternal(subject, meta);

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 1, "log count");
        assertEq(logs[0].emitter, address(this), "emitter");
        assertEq(logs[0].topics.length, 1, "topic count");
        assertEq(logs[0].topics[0], MetaV1_2.selector, "topic");
        assertEq(logs[0].data, abi.encode(sender, subject, meta), "log data");
    }

    /// Metadata that is not rain meta is rejected by the library's own check,
    /// reverting `NotRainMetaV1` carrying the offending bytes.
    /// @param sender The pranked caller, which must not change the outcome.
    /// @param subject The subject, which must not change the outcome.
    /// @param data Arbitrary bytes that are not rain meta.
    function testEmitMetaNotRainMeta(address sender, bytes32 subject, bytes memory data) external {
        vm.assume(sender != address(vm));
        vm.assume(!LibMeta.isRainMetaV1(data));

        vm.expectRevert(abi.encodeWithSelector(NotRainMetaV1.selector, data));
        vm.prank(sender);
        this.emitMetaExternal(subject, data);
    }

    /// The magic number alone, with no body at all, is rain meta and emits.
    /// @param sender The pranked caller the event must be attributed to.
    /// @param subject The subject to emit.
    function testEmitMetaEmptyBody(address sender, bytes32 subject) external {
        vm.assume(sender != address(vm));
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1);

        vm.recordLogs();
        vm.prank(sender);
        this.emitMetaExternal(subject, meta);

        Vm.Log[] memory logs = vm.getRecordedLogs();
        assertEq(logs.length, 1, "log count");
        assertEq(logs[0].data, abi.encode(sender, subject, meta), "log data");
    }
}
