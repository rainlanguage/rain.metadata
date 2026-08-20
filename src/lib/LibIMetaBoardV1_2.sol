// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
//
// Qualified access to another contract's event (`emit IMetaV1_2.MetaV1_2`) is
// a Solidity 0.8.21 language feature, and a library cannot inherit the
// interface to bring the event into scope any other way. That sets the floor
// here, above the `^0.8.19` the rest of the interfaces and libraries carry.
pragma solidity ^0.8.21;

import {IMetaV1_2} from "../interface/unstable/IMetaV1_2.sol";
import {LibMeta} from "./LibMeta.sol";

/// @title LibIMetaBoardV1_2
/// @notice The whole of an `IMetaBoardV1_2` metaboard as internal library
/// logic, so a concrete metaboard is nothing but one delegation per entry
/// point (rainlanguage/rain.metadata#144). The validation and the event live
/// here, unit tested here, and the shipped `MetaBoard` adds no behaviour of
/// its own — the equivalence suite holds its entry point to exactly this.
///
/// `msg.sender` is read INSIDE this library and the internal functions execute
/// in the calling contract's own call context, so the `MetaV1_2` event is
/// emitted BY the metaboard the library is inlined into and attributed TO
/// whoever called it. A delegating concrete cannot get either wrong, and any
/// other contract that wants to be a metaboard gets the same behaviour by
/// inlining the same code rather than by reimplementing it.
library LibIMetaBoardV1_2 {
    /// The whole of `IMetaBoardV1_2.emitMeta`: reject anything that is not
    /// rain metadata, then emit it verbatim under the caller and subject.
    ///
    /// The check is `checkMetaUnhashedV1` because a metaboard is an open
    /// bulletin board with no expected hash to check against — anons MAY send
    /// garbage that happens to carry the magic number, and per `IMetaBoardV1_2`
    /// it is tooling's job to discard suspect data. The magic number is the
    /// only thing a metaboard promises to enforce.
    /// @param subject As per the `MetaV1_2` event.
    /// @param meta As per the `MetaV1_2` event. MUST be prefixed with the rain
    /// metadata magic number or this reverts `NotRainMetaV1`.
    function emitMeta(bytes32 subject, bytes memory meta) internal {
        LibMeta.checkMetaUnhashedV1(meta);
        emit IMetaV1_2.MetaV1_2(msg.sender, subject, meta);
    }
}
