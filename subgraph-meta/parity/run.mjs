// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { readFileSync } from "node:fs";
import loader from "@assemblyscript/loader";

const [wasmPath, docsPath] = process.argv.slice(2);
const module = new WebAssembly.Module(readFileSync(wasmPath));
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
  imports[name][field] = () => {
    throw new Error(`host import ${name}.${field} called`);
  };
}
const { exports } = loader.instantiateSync(module, imports);
if (docsPath === "magics") {
  process.stdout.write(exports.__getString(exports.magics()) + "\n");
  process.exit(0);
}
const out = [];
for (const line of readFileSync(docsPath, "utf8").split("\n")) {
  if (line === "") continue;
  const [label, doc] = line.split("\t");
  const ptr = exports.__pin(exports.__newString(doc));
  out.push(`${label}\t${exports.__getString(exports.run(ptr))}`);
  exports.__unpin(ptr);
}
process.stdout.write(out.join("\n") + "\n");
