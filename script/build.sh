#!/usr/bin/env bash
# SPDX-License-Identifier: LicenseRef-DCL-1.0
# SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
# Regenerate derived artifacts that forge cannot produce.
#
# The `rainix-copy-artifacts` workflow runs this after its forge steps and then
# asserts `git diff --exit-code`, so anything written here is held to the same
# freshness bar as the committed abis: change a producer without committing what
# it now produces and git-clean goes red.
#
# Runs outside any nix devshell, so each command picks its own.
set -euo pipefail

# The `emitMeta` calldata the rust encoders produce, consumed by
# `test/lib/EmitCalldataFixture.t.sol`, which sends it to a real metaboard. The
# fixture is how the solidity half gets to judge bytes the rust half built.
nix develop -c cargo run -p rain-metadata --example emit-calldata-fixture
