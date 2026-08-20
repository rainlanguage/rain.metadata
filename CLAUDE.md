# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Project Overview

Rain Protocol metadata system — Solidity interfaces and libraries, a Rust
CLI/bindings, and a Graph subgraph for emitting and indexing on-chain metadata
following the
[MetadataV1 spec](https://github.com/rainprotocol/specs/blob/main/metadata-v1.md).

This repo is the **library half** (rainlanguage/rain.metadata#134): the
`IMetaBoardV1_2` / `IMetaV1_2` / `IDescribedByMetaV1` interface surface plus the
`Lib*` logic that implements it. It autopublishes to Soldeer as `rain-metadata`
on merge to main. It holds no concrete contract and no deployment.

The **deploy half** is `rain.metadata.deploy`: the concrete `MetaBoard`, its
deterministic-deploy address and codehash pins, the frozen release snapshots and
`script/Deploy.sol`. It depends on this repo's Soldeer package and releases only
on a manual `sol-v*` tag. The deployed `MetaBoard` address lives there, not
here. `MetaBoard` is what emits the `MetaV1_2` events the subgraph here indexes.

## Build & Test Commands

All commands require the Nix development shell. Use `nix develop` to enter it,
or prefix commands with `nix develop -c`.

| Task                     | Command                               |
| ------------------------ | ------------------------------------- |
| Solidity tests           | `nix develop -c rainix-sol-test`      |
| Solidity static analysis | `nix develop -c rainix-sol-static`    |
| Solidity build artifacts | `nix develop -c rainix-sol-artifacts` |
| Rust tests               | `nix develop -c rainix-rs-test`       |
| Rust static analysis     | `nix develop -c rainix-rs-static`     |
| Rust build artifacts     | `nix develop -c rainix-rs-artifacts`  |
| Subgraph build           | `nix develop -c subgraph-build`       |
| Subgraph tests           | `nix develop -c subgraph-test`        |
| REUSE license check      | `nix develop -c rainix-sol-legal`     |

Run a single Solidity test (inside nix shell):

```sh
forge test --match-test testFunctionName
forge test --match-contract LibIMetaBoardV1_2EmitMetaTest
```

Run a single Rust test (inside nix shell):

```sh
cargo test test_name
```

## Architecture

### Solidity (`src/`)

- `src/interface/unstable/IMetaBoardV1_2.sol` — The metaboard entry point:
  `emitMeta(bytes32,bytes)`. Extends `IMetaV1_2`
- `src/interface/unstable/IMetaV1_2.sol` — Declares the `MetaV1_2` event the
  subgraph indexes
- `src/interface/IDescribedByMetaV1.sol` — For contracts that describe
  themselves with metadata
- `src/interface/deprecated/` — Superseded `IMetaV1` / `IMetaBoardV1`, kept for
  consumers still pinned to them. `IMetaV1.sol` also carries the file-level
  `NotRainMetaV1` and `UnexpectedMetaHash` errors the current libs revert with
- `src/lib/LibIMetaBoardV1_2.sol` — The whole of `IMetaBoardV1_2` as library
  logic; validates then emits `MetaV1_2` with sender, subject and meta bytes. A
  conforming concrete is one delegation per entry point into this and nothing
  else
- `src/lib/LibMeta.sol` — Metadata validation; checks magic number prefix
  `0xff0a89c674ee7874`
- `src/lib/LibDescribedByMeta.sol` — Helper for contracts implementing
  `IDescribedByMetaV1`

`test/concrete/TestMetaBoard.sol` is a pure-delegation `IMetaBoardV1_2` that
exists only so tests can drive the library across a real external call —
`msg.sender` attribution and event emission are not observable otherwise. It is
test scaffolding, not a shipped contract; the shipped concrete is in
`rain.metadata.deploy`.

### Rust (`crates/`)

- `crates/cli` — `rain-metadata` binary; metadata generation/validation for
  multiple types (authoring, dotrain, Solidity ABI, etc.)
- `crates/bindings` — Solidity bindings generated via `alloy::sol!` from the
  committed interface ABIs in `crates/bindings/abi/`. Those are written by
  `forge script script/CopyArtifacts.sol --ffi` (a deterministic `jq` subset of
  the forge artifact) and asserted fresh by `test/script/CopyArtifacts.t.sol`.
  Adding a contract means adding it to `LibCopyArtifacts.contracts()` and to
  `crates/bindings/src/lib.rs` together
- `crates/metaboard` — GraphQL client (Cynic) for querying MetaBoard subgraph
  data

### Subgraph (`subgraph/`)

- AssemblyScript handlers indexing `MetaV1_2` events from MetaBoard
- Deployed across ~15 networks (Arbitrum, Base, Polygon, Flare, etc.)

## Key Configuration

- **Solidity**: `foundry.toml` — solc 0.8.25, Cancun EVM, optimizer 1M runs,
  `bytecode_hash = "none"`, `cbor_metadata = false`. `ffi = true` and the
  `out/` + `crates/bindings/abi/` filesystem permissions exist solely for
  `CopyArtifacts`; nothing else here touches the filesystem or shells out
- **Rust workspace**: `Cargo.toml` at root, three crates
- **Fuzz runs**: 5,096 (foundry.toml `[fuzz]`)
- **Dependencies**: managed by [Soldeer](https://soldeer.xyz) (`[dependencies]`
  in `foundry.toml`, `libs = ["dependencies"]`), not git submodules; remappings
  point into `dependencies/` (e.g.
  `forge-std-1.16.2/=dependencies/forge-std-1.16.2/`). The published `src/`
  imports nothing external, so the only dependency is the test harness

## Licensing

DecentraLicense 1.0 (DCL-1.0). REUSE 3.2 compliant — all files need SPDX license
headers.
