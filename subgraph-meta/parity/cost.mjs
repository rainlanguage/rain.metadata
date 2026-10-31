// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { keccak_256 } from "@noble/hashes/sha3.js";
import { instantiate } from "./instantiate.mjs";

const SCALE = 16;
const SLACK = 3;
const WARMUP = 3;
const ROUNDS = 15;

const P = "ff0a89c674ee7874";
const RL = "1bff13109e41336ff2";
const NM = "1bffcdcfae6227d5e9";
const NP = "737430782e726261632e75732e7631";
const ITEM = "a2004161" + "01" + RL;

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const width = (v, bytes) => v.toString(16).padStart(bytes * 2, "0");
const head = (major, n) => {
  const m = major << 5;
  if (n < 24) return width(m | n, 1);
  if (n < 0x100) return width(m | 24, 1) + width(n, 1);
  if (n < 0x10000) return width(m | 25, 1) + width(n, 2);
  return width(m | 26, 1) + width(n, 4);
};
const key = (i) =>
  width(
    (BigInt(i) * 0x9e3779b97f4a7c15n + 0x0123456789abcdefn) &
      0xffffffffffffffffn,
    8,
  );

const shapes = [
  {
    name: "distinct item keys",
    n: 6000,
    doc: (n) =>
      P + head(5, n + 2) + "004161" + "01" + RL +
      Array.from({ length: n }, (_, i) => "1b" + key(i) + "00").join(""),
    expect: () => 1,
  },
  {
    name: "deepest nesting",
    n: 500,
    doc: (n) =>
      P + "a3004161" + "01" + RL + "05" + head(4, n) +
      ("81".repeat(125) + "00").repeat(n),
    expect: () => 1,
  },
  {
    name: "deepest tags",
    n: 500,
    doc: (n) =>
      P + "a3004161" + "01" + RL + "05" + head(4, n) +
      ("c1".repeat(125) + "00").repeat(n),
    expect: () => 1,
  },
  {
    name: "indefinite byte string payload",
    n: 30000,
    doc: (n) => P + "a200" + "5f" + "4161".repeat(n) + "ff" + "01" + RL,
    expect: () => 1,
  },
  {
    name: "indefinite array payload",
    n: 60000,
    doc: (n) => P + "a200" + "9f" + "00".repeat(n) + "ff" + "01" + RL,
    expect: () => 1,
  },
  {
    name: "indefinite text string",
    n: 20000,
    doc: (n) =>
      P + "a3004161" + "01" + RL + "05" + "7f" + "62c3a9".repeat(n) + "ff",
    expect: () => 1,
  },
  {
    name: "many items",
    n: 1000,
    doc: (n) => P + ITEM.repeat(n),
    expect: (n) => n,
  },
  {
    name: "many verified items",
    n: 500,
    doc: (n) => P + ("a2004f" + NP + "01" + NM).repeat(n),
    checks: NM.slice(2) + ":" + hex(keccak_256(Buffer.from(NP, "hex"))),
    expect: () => 1,
  },
];

const exports = instantiate(process.argv[2]);
const bytes = (doc) =>
  exports.__pin(exports.__newArray(exports.BYTES_ID, Buffer.from(doc, "hex")));

let failed = false;
for (const shape of shapes) {
  const checks = shape.checks === undefined
    ? undefined
    : exports.__pin(exports.__newString(shape.checks));
  const call = (ptr) =>
    checks === undefined
      ? exports.decodedItems(ptr)
      : exports.verifiedPreimages(ptr, checks);
  const sizes = [shape.n, shape.n * SCALE].map((n) => {
    const doc = shape.doc(n);
    return { n, length: doc.length / 2, ptr: bytes(doc), best: Infinity };
  });
  for (const size of sizes) {
    let result;
    try {
      result = call(size.ptr);
    } catch (error) {
      console.error(`${shape.name} n=${size.n}: trap`);
      throw error;
    }
    if (result !== shape.expect(size.n)) {
      console.error(
        `${shape.name} n=${size.n}: ${result}, expected ${
          shape.expect(size.n)
        }`,
      );
      failed = true;
    }
  }
  for (let round = 0; round < WARMUP + ROUNDS; round++) {
    for (const size of sizes) {
      exports.__collect();
      const start = process.hrtime.bigint();
      call(size.ptr);
      const ns = Number(process.hrtime.bigint() - start);
      if (round >= WARMUP) size.best = Math.min(size.best, ns);
    }
  }
  const [small, large] = sizes;
  const ratio = large.best / small.best;
  const ok = ratio <= SCALE * SLACK;
  failed ||= !ok;
  console.log(
    `${ok ? "ok" : "FAIL"}\t${shape.name}\t${small.length}B ${
      (small.best / 1e6).toFixed(3)
    }ms\t${large.length}B ${(large.best / 1e6).toFixed(3)}ms\tratio ${
      ratio.toFixed(2)
    } (limit ${SCALE * SLACK})`,
  );
  for (const size of sizes) exports.__unpin(size.ptr);
  if (checks !== undefined) exports.__unpin(checks);
}
process.exit(failed ? 1 : 0);
