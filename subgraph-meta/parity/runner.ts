// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
import { ByteArray, Bytes } from "@graphprotocol/graph-ts";
import {
  ContentEncoding,
  ContentLanguage,
  ContentType,
  decode,
  Decoded,
  KNOWN_MAGICS,
  MalformedDocument,
  NotRainMetaDocument,
  PreimageCheck,
  verifyPreimages,
} from "../assembly/index";

export const BYTE_ARRAY_ID = idof<ByteArray>();

const HEX = "0123456789abcdef";

function nibble(c: i32): u8 {
  return <u8>(c <= 57 ? c - 48 : c - 87);
}

function unhex(s: string): Bytes {
  const bytes = new Uint8Array(s.length >> 1);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = (nibble(s.charCodeAt(2 * i)) << 4) | nibble(s.charCodeAt(2 * i + 1));
  }
  return changetype<Bytes>(bytes);
}

function hex(b: Uint8Array): string {
  const parts = new Array<string>(b.length);
  for (let i = 0; i < b.length; i++) {
    const x = <i32>b[i];
    parts[i] = HEX.charAt(x >> 4) + HEX.charAt(x & 15);
  }
  return parts.join("");
}

function hex64(v: u64): string {
  const parts = new Array<string>(16);
  for (let i = 0; i < 16; i++) {
    parts[i] = HEX.charAt(<i32>((v >> (<u64>(60 - 4 * i))) & 15));
  }
  return parts.join("");
}

function unhex64(s: string): u64 {
  let v: u64 = 0;
  for (let i = 0; i < s.length; i++) {
    v = (v << 4) | <u64>nibble(s.charCodeAt(i));
  }
  return v;
}

function contentType(t: ContentType): string {
  switch (t) {
    case ContentType.None:
      return "None";
    case ContentType.Json:
      return "Json";
    case ContentType.Cbor:
      return "Cbor";
    case ContentType.OctetStream:
      return "OctetStream";
  }
  return "ContentType(" + (<i32>t).toString() + ")";
}

function contentEncoding(e: ContentEncoding): string {
  switch (e) {
    case ContentEncoding.None:
      return "None";
    case ContentEncoding.Identity:
      return "Identity";
    case ContentEncoding.Deflate:
      return "Deflate";
  }
  return "ContentEncoding(" + (<i32>e).toString() + ")";
}

function contentLanguage(l: ContentLanguage): string {
  switch (l) {
    case ContentLanguage.None:
      return "None";
    case ContentLanguage.En:
      return "En";
  }
  return "ContentLanguage(" + (<i32>l).toString() + ")";
}

function decoded(bytes: Bytes): string {
  const result = decode(bytes);
  if (result instanceof Decoded) {
    const items = (<Decoded>result).items;
    const parts = new Array<string>(items.length + 1);
    parts[0] = "OK " + items.length.toString();
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      parts[i + 1] =
        hex64(item.magic) +
        "," +
        hex(item.payload) +
        "," +
        contentType(item.contentType) +
        "," +
        contentEncoding(item.contentEncoding) +
        "," +
        contentLanguage(item.contentLanguage);
    }
    return parts.join(";");
  }
  if (result instanceof NotRainMetaDocument) {
    return "NotRainMetaDocument";
  }
  if (result instanceof MalformedDocument) {
    return "MalformedDocument";
  }
  return "DecodeResult";
}

function verified(bytes: Bytes, spec: string): string {
  const specs = spec.split(";");
  const checks = new Array<PreimageCheck>(specs.length);
  for (let i = 0; i < specs.length; i++) {
    const pair = specs[i].split(":");
    checks[i] = new PreimageCheck(unhex64(pair[0]), unhex(pair[1]));
  }
  const preimages = verifyPreimages(bytes, checks);
  if (preimages === null) {
    return "UNVERIFIED";
  }
  const parts = new Array<string>(preimages.length);
  for (let i = 0; i < preimages.length; i++) {
    parts[i] = hex(preimages[i]);
  }
  return "VERIFIED " + parts.join(",");
}

export function run(doc: string, checks: string): string {
  const bytes = unhex(doc);
  return decoded(bytes) + "\t" + verified(bytes, checks);
}

export function magics(): string {
  const parts = new Array<string>(KNOWN_MAGICS.length);
  for (let i = 0; i < KNOWN_MAGICS.length; i++) {
    parts[i] = hex64(KNOWN_MAGICS[i]);
  }
  return parts.join("\n");
}
