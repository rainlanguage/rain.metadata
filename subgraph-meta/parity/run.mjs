// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { readFileSync } from "node:fs";
import loader from "@assemblyscript/loader";
import { keccak_256 } from "@noble/hashes/sha3.js";

const [wasmPath, docsPath] = process.argv.slice(2);
const module = new WebAssembly.Module(readFileSync(wasmPath));
let exports;
const hosts = {
  "crypto.keccak256": (ptr) =>
    exports.__newArray(
      exports.BYTE_ARRAY_ID,
      keccak_256(exports.__getUint8Array(ptr)),
    ),
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
if (docsPath === "magics") {
  process.stdout.write(exports.__getString(exports.magics()) + "\n");
  process.exit(0);
}
const out = [];
for (const line of readFileSync(docsPath, "utf8").split("\n")) {
  if (line === "") continue;
  const [label, doc, checks] = line.split("\t");
  if (checks === undefined) {
    throw new Error(`docs line without a document and checks: ${line}`);
  }
  const docPtr = exports.__pin(exports.__newString(doc));
  const checksPtr = exports.__pin(exports.__newString(checks));
  out.push(`${label}\t${exports.__getString(exports.run(docPtr, checksPtr))}`);
  exports.__unpin(checksPtr);
  exports.__unpin(docPtr);
}
process.stdout.write(out.join("\n") + "\n");
