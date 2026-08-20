// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test, Vm} from "forge-std-1.16.2/src/Test.sol";
import {IMetaBoardV1_2} from "src/interface/unstable/IMetaBoardV1_2.sol";
import {NotRainMetaV1, META_MAGIC_NUMBER_V1} from "src/interface/unstable/IMetaV1_2.sol";
import {LibMeta} from "src/lib/LibMeta.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";
import {TestLibIMetaBoardV1_2} from "./TestLibIMetaBoardV1_2.sol";

/// @title MetaBoardLibEquivalenceTest
/// @notice Per-function equivalence between the shipped `MetaBoard` and
/// `LibIMetaBoardV1_2`: for the one entry point the concrete has, its
/// observable behaviour — success, return data, the `MetaV1_2` event and its
/// emitter, the typed revert — is held equal to the library's, where the
/// library is observed as `TestLibIMetaBoardV1_2`, the same delegation
/// declared independently. A concrete that grows any behaviour beyond
/// delegation diverges here.
///
/// `emitMeta` writes no storage, so unlike a factory's clone entry points the
/// two surfaces can be exercised back to back from the same state with no
/// snapshot and rollback between them: neither call can affect the other.
contract MetaBoardLibEquivalenceTest is Test {
    /// The shipped concrete under test. Stateless, so reused everywhere.
    MetaBoard internal immutable I_META_BOARD;

    /// The library run bare behind an independent delegating surface.
    TestLibIMetaBoardV1_2 internal immutable I_LIB_META_BOARD;

    constructor() {
        I_META_BOARD = new MetaBoard();
        I_LIB_META_BOARD = new TestLibIMetaBoardV1_2();
    }

    /// Everything observable about one `emitMeta` call on one metaboard,
    /// captured so two metaboards' observations can be held equal.
    struct EmitObservation {
        /// Whether the call succeeded.
        bool success;
        /// The raw return data, which carries the typed revert on failure.
        bytes returnData;
        /// How many logs the call emitted.
        uint256 logCount;
        /// The emitter of the sole log, zero if there was none.
        address emitter;
        /// `topics[0]` of the sole log, zero if there was none.
        bytes32 topic;
        /// How many topics the sole log carried.
        uint256 topicCount;
        /// The abi-encoded body of the sole log, empty if there was none.
        bytes logData;
    }

    /// Run one `emitMeta` call on one metaboard and capture everything
    /// observable about it.
    /// @param metaBoard The metaboard to call.
    /// @param sender The pranked caller.
    /// @param subject The subject to emit.
    /// @param meta The metadata to emit.
    /// @return The observation.
    function observeEmitMeta(address metaBoard, address sender, bytes32 subject, bytes memory meta)
        internal
        returns (EmitObservation memory)
    {
        vm.recordLogs();
        vm.prank(sender);
        //slither-disable-next-line low-level-calls
        (bool success, bytes memory returnData) =
            metaBoard.call(abi.encodeCall(IMetaBoardV1_2.emitMeta, (subject, meta)));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        EmitObservation memory observation;
        observation.success = success;
        observation.returnData = returnData;
        observation.logCount = logs.length;
        if (logs.length > 0) {
            observation.emitter = logs[0].emitter;
            observation.topicCount = logs[0].topics.length;
            observation.topic = logs[0].topics[0];
            observation.logData = logs[0].data;
        }
        return observation;
    }

    /// The strongest form of "the concrete only delegates": `MetaBoard` and
    /// `TestLibIMetaBoardV1_2` are the same single delegation declared
    /// independently, so with metadata stripped they compile to identical
    /// runtime bytecode. Any behaviour added to the concrete breaks this
    /// before it breaks anything behavioural.
    function testEquivalenceRuntimeBytecode() external pure {
        assertEq(type(MetaBoard).runtimeCode, type(TestLibIMetaBoardV1_2).runtimeCode);
    }

    /// On the success path the concrete and the bare library emit the same
    /// single `MetaV1_2` with the same topic and the same body, each from its
    /// own address, and both return nothing.
    /// @param sender The pranked caller the event must be attributed to.
    /// @param subject The subject to emit.
    /// @param data Arbitrary bytes to carry after the magic number.
    function testEquivalenceEmitMeta(address sender, bytes32 subject, bytes memory data) external {
        vm.assume(sender != address(vm));
        bytes memory meta = abi.encodePacked(META_MAGIC_NUMBER_V1, data);

        EmitObservation memory concrete = observeEmitMeta(address(I_META_BOARD), sender, subject, meta);
        EmitObservation memory bare = observeEmitMeta(address(I_LIB_META_BOARD), sender, subject, meta);

        assertTrue(concrete.success, "concrete success");
        assertEq(concrete.success, bare.success, "success");
        assertEq(concrete.returnData, bare.returnData, "return data");
        assertEq(concrete.returnData.length, 0, "return data empty");

        assertEq(concrete.logCount, 1, "concrete log count");
        assertEq(concrete.logCount, bare.logCount, "log count");
        assertEq(concrete.topicCount, bare.topicCount, "topic count");
        assertEq(concrete.topic, bare.topic, "topic");
        assertEq(concrete.logData, bare.logData, "log data");

        // The one thing that legitimately differs: the library is inlined into
        // whichever contract calls it, so each surface emits from its own
        // address. That is the delegation working, not a divergence.
        assertEq(concrete.emitter, address(I_META_BOARD), "concrete emitter");
        assertEq(bare.emitter, address(I_LIB_META_BOARD), "bare emitter");
    }

    /// On the reject path the concrete and the bare library fail identically,
    /// returning the same typed `NotRainMetaV1` revert and emitting nothing.
    /// @param sender The pranked caller, which must not change the outcome.
    /// @param subject The subject, which must not change the outcome.
    /// @param data Arbitrary bytes that are not rain meta.
    function testEquivalenceEmitMetaNotRainMeta(address sender, bytes32 subject, bytes memory data) external {
        vm.assume(sender != address(vm));
        vm.assume(!LibMeta.isRainMetaV1(data));

        EmitObservation memory concrete = observeEmitMeta(address(I_META_BOARD), sender, subject, data);
        EmitObservation memory bare = observeEmitMeta(address(I_LIB_META_BOARD), sender, subject, data);

        assertFalse(concrete.success, "concrete failure");
        assertEq(concrete.success, bare.success, "success");
        assertEq(concrete.returnData, bare.returnData, "return data");
        assertEq(concrete.returnData, abi.encodeWithSelector(NotRainMetaV1.selector, data), "typed revert");

        assertEq(concrete.logCount, 0, "concrete log count");
        assertEq(concrete.logCount, bare.logCount, "log count");
    }
}
