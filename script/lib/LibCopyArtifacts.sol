// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 thedavidmeister
pragma solidity ^0.8.25;

import {Vm} from "forge-std-1.16.1/src/Vm.sol";

/// @notice Shared logic between `script/CopyArtifacts.sol` (writes the
/// committed ABI) and `test/script/CopyArtifacts.t.sol` (asserts the
/// committed ABI is fresh).
library LibCopyArtifacts {
    /// @notice Contract artifacts that the rust crate consumes via
    /// alloy::sol!. Adding a new contract here also requires the rust
    /// crate to reference it.
    function contracts() internal pure returns (string[] memory) {
        string[] memory names = new string[](2);
        names[0] = "MetaBoard";
        names[1] = "IDescribedByMetaV1";
        return names;
    }

    /// @notice Path of the live forge build artifact for a contract.
    function livePath(string memory contractName) internal pure returns (string memory) {
        return string.concat("out/", contractName, ".sol/", contractName, ".json");
    }

    /// @notice Path of the committed ABI copy that the rust crate reads
    /// at compile time.
    function committedPath(string memory contractName) internal pure returns (string memory) {
        return string.concat("crates/bindings/abi/", contractName, ".json");
    }

    /// @notice Extracts the deterministic subset of the live forge
    /// artifact via `jq` over `vm.ffi`. The full forge JSON is
    /// non-deterministic across machines (solc source unit IDs in
    /// `metadata.sources`, `sourceMap` and friends shift with filesystem
    /// enumeration order). The kept keys — `abi`, `bytecode.object`,
    /// `deployedBytecode.object` — are pure functions of the input
    /// source and compiler settings. alloy::sol! reads `abi` for type
    /// generation.
    function extractStable(Vm vm, string memory contractName) internal returns (bytes memory) {
        string[] memory cmd = new string[](3);
        cmd[0] = "jq";
        cmd[1] = "{abi, bytecode: {object: .bytecode.object}, deployedBytecode: {object: .deployedBytecode.object}}";
        cmd[2] = livePath(contractName);
        return vm.ffi(cmd);
    }
}
