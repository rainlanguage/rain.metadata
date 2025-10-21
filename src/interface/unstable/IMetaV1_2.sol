// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity ^0.8.19;

//forge-lint: disable-next-line(unused-import)
import {UnexpectedMetaHash, NotRainMetaV1, META_MAGIC_NUMBER_V1} from "../deprecated/IMetaV1.sol";

/// @title IMetaV1_2
/// Identical to `IMetaV1` but with the subject as a `bytes32` instead of a
/// `uint256`.
//slither-disable-next-line naming-convention
interface IMetaV1_2 {
    /// An onchain wrapper to carry arbitrary Rain metadata. Assigns the sender
    /// to the metadata so that tooling can easily drop/ignore data from unknown
    /// sources. As metadata is about something, the subject MUST be provided.
    /// @param sender The msg.sender.
    /// @param subject The entity that the metadata is about. MAY be the address
    /// of the emitting contract (as `bytes32`) OR anything else. The
    /// interpretation of the subject is context specific, so will often be a
    /// hash of some data/thing that this metadata is about.
    /// @param meta Rain metadata V1 compliant metadata bytes.
    /// https://github.com/rainprotocol/specs/blob/main/metadata-v1.md
    //slither-disable-next-line naming-convention
    event MetaV1_2(address sender, bytes32 subject, bytes meta);
}
