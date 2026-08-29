// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { Address } from "@graphprotocol/graph-ts";

export const CONTRACT_ADDRESS = Address.fromString(
  "0xfb8437AeFBB8031064E274527C5fc08e30Ac6928",
);

// A second metaboard, for the case one deployment indexes more than one. It
// has no counterpart on any chain; nothing here calls it.
export const OTHER_CONTRACT_ADDRESS = Address.fromString(
  "0x4a9b0f6c1e3d2a5b8c7d6e9f0a1b2c3d4e5f6a7b",
);
