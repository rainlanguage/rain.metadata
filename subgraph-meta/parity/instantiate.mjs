// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { readFileSync } from "node:fs";
import loader from "@assemblyscript/loader";
import { keccak_256 } from "@noble/hashes/sha3.js";

/** The runner's exports, with crypto.keccak256 served and every other host import throwing. */
export function instantiate(wasmPath) {
  const module = new WebAssembly.Module(readFileSync(wasmPath));
  let exports;
  // __getUint8Array sizes a subarray by its whole buffer; read the view's own fields.
  const bytesAt = (ptr) => {
    const [, dataStart, byteLength] = new Uint32Array(
      exports.memory.buffer,
      ptr,
      3,
    );
    return new Uint8Array(exports.memory.buffer, dataStart, byteLength);
  };
  const hosts = {
    "crypto.keccak256": (ptr) =>
      exports.__newArray(exports.BYTE_ARRAY_ID, keccak_256(bytesAt(ptr))),
  };
  const imports = {};
  for (
    const { module: name, name: field, kind } of WebAssembly.Module.imports(
      module,
    )
  ) {
    if (kind !== "function") {
      throw new Error(`unexpected import ${name}.${field}`);
    }
    imports[name] ??= {};
    imports[name][field] = hosts[field] ?? (() => {
      throw new Error(`host import ${name}.${field} called`);
    });
  }
  ({ exports } = loader.instantiateSync(module, imports));
  return exports;
}
