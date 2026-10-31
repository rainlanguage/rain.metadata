// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { readFileSync } from "node:fs";
import { instantiate } from "./instantiate.mjs";

const [wasmPath, docsPath] = process.argv.slice(2);
const exports = instantiate(wasmPath);
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
  let result;
  try {
    result = exports.__getString(exports.run(docPtr, checksPtr));
  } catch (error) {
    process.stderr.write(`trap on ${label}\t${doc}\t${checks}\n`);
    throw error;
  }
  out.push(`${label}\t${result}`);
  exports.__unpin(checksPtr);
  exports.__unpin(docPtr);
}
process.stdout.write(out.join("\n") + "\n");
