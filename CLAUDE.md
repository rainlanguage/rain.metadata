# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rain Protocol metadata system — Solidity contracts, Rust CLI/bindings, and a Graph subgraph for emitting and indexing on-chain metadata following the [MetadataV1 spec](https://github.com/rainprotocol/specs/blob/main/metadata-v1.md).

The core contract is **MetaBoard** — deterministically deployed at `0xfb8437AeFBB8031064E274527C5fc08e30Ac6928` across all supported networks. It emits `MetaV1_2` events that the subgraph indexes.

## Build & Test Commands

All commands require the Nix development shell. Use `nix develop` to enter it, or prefix commands with `nix develop -c`.

| Task | Command |
|------|---------|
| Solidity tests | `nix develop -c rainix-sol-test` |
| Solidity static analysis | `nix develop -c rainix-sol-static` |
| Solidity build artifacts | `nix develop -c rainix-sol-artifacts` |
| Rust tests | `nix develop -c rainix-rs-test` |
| Rust static analysis | `nix develop -c rainix-rs-static` |
| Rust build artifacts | `nix develop -c rainix-rs-artifacts` |
| Subgraph build | `nix develop -c subgraph-build` |
| Subgraph tests | `nix develop -c subgraph-test` |
| REUSE license check | `nix develop -c rainix-sol-legal` |

Run a single Solidity test (inside nix shell):

```sh
forge test --match-test testFunctionName
forge test --match-contract MetaBoardTest
```

Run a single Rust test (inside nix shell):

```sh
cargo test test_name
```

## Architecture

### Solidity (`src/`)
- `src/concrete/MetaBoard.sol` — Main contract; emits `MetaV1_2` events with sender, subject, and metadata bytes
- `src/lib/LibMeta.sol` — Metadata validation; checks magic number prefix `0xff0a89c674ee7874`
- `src/lib/LibDescribedByMeta.sol` — Helper for contracts implementing `IDescribedByMetaV1`
- `src/lib/deploy/LibMetaBoardDeploy.sol` — Deterministic deployment using Zoltu deployer pattern

### Rust (`crates/`)
- `crates/cli` — `rain-metadata` binary; metadata generation/validation for multiple types (authoring, dotrain, Solidity ABI, etc.)
- `crates/bindings` — Solidity bindings generated via `alloy::sol!` from JSON ABIs in `/out`
- `crates/metaboard` — GraphQL client (Cynic) for querying MetaBoard subgraph data

### Subgraph (`subgraph/`)
- AssemblyScript handlers indexing `MetaV1_2` events from MetaBoard
- Deployed across ~15 networks (Arbitrum, Base, Polygon, Flare, etc.)

## Key Configuration

- **Solidity**: `foundry.toml` — solc 0.8.25, Cancun EVM, optimizer 1M runs, `bytecode_hash = "none"`, `cbor_metadata = false`
- **Rust workspace**: `Cargo.toml` at root, three crates
- **Fuzz runs**: 5,096 (foundry.toml `[fuzz]`)
- **Remappings**: `rain.deploy/=lib/rain.deploy/src/`

## Licensing

DecentraLicense 1.0 (DCL-1.0). REUSE 3.2 compliant — all files need SPDX license headers.
