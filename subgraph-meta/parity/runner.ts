import { Bytes } from "@graphprotocol/graph-ts";
import { decode, Decoded, KNOWN_MAGICS } from "../assembly/index";

const HEX = "0123456789abcdef";
const CONTENT_TYPE = ["None", "Json", "Cbor", "OctetStream"];
const CONTENT_ENCODING = ["None", "Identity", "Deflate"];
const CONTENT_LANGUAGE = ["None", "En"];

function nibble(c: i32): u8 {
  return <u8>(c <= 57 ? c - 48 : c - 87);
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

export function run(doc: string): string {
  const bytes = new Uint8Array(doc.length >> 1);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = (nibble(doc.charCodeAt(2 * i)) << 4) | nibble(doc.charCodeAt(2 * i + 1));
  }
  const result = decode(changetype<Bytes>(bytes));
  if (!(result instanceof Decoded)) {
    return "ERR";
  }
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
      CONTENT_TYPE[<i32>item.contentType] +
      "," +
      CONTENT_ENCODING[<i32>item.contentEncoding] +
      "," +
      CONTENT_LANGUAGE[<i32>item.contentLanguage];
  }
  return parts.join(";");
}

export function magics(): string {
  const parts = new Array<string>(KNOWN_MAGICS.length);
  for (let i = 0; i < KNOWN_MAGICS.length; i++) {
    parts[i] = hex64(KNOWN_MAGICS[i]);
  }
  return parts.join("\n");
}
