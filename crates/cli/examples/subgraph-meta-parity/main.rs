use alloy::primitives::keccak256;
use rain_metadata::{ContentEncoding, Error, KnownMagic, RainMetaDocumentV1Item};
use strum::IntoEnumIterator;

mod fuzz;

const P: &str = "ff0a89c674ee7874";
const NP: &str = "737430782e726261632e75732e7631";
const NM: &str = "1bffcdcfae6227d5e9";
const TP: &str = "61646472657373";
const TM: &str = "1bffcdb2cc1b1d53be";
const RL: &str = "1bff13109e41336ff2";
const SK: &str = "1bffa8e8a9b9cf4a31";
const FK5: &str = "a3004161011bff13109e41336ff205";

fn name() -> String {
    format!("a2004f{NP}01{NM}")
}

fn type_item() -> String {
    format!("a20047{TP}01{TM}")
}

fn checks() -> Vec<(u64, [u8; 32])> {
    [(NM, NP), (TM, TP)]
        .iter()
        .map(|(magic, preimage)| {
            (
                u64::from_str_radix(&magic[2..], 16).expect("magic"),
                keccak256(unhex(preimage)).0,
            )
        })
        .collect()
}

fn expand(tokens: &[&str]) -> String {
    let mut out = String::new();
    for t in tokens {
        match *t {
            "P" => out.push_str(P),
            "NAME" => out.push_str(&name()),
            "TYPE" => out.push_str(&type_item()),
            "RL" => out.push_str(RL),
            "NM" => out.push_str(NM),
            "NP" => out.push_str(NP),
            "SK" => out.push_str(SK),
            t => match t.split_once('*') {
                Some((h, n)) => out.push_str(&h.repeat(n.parse().expect("repeat count"))),
                None => out.push_str(t),
            },
        }
    }
    out
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn preimage_string(preimage: &[u8]) -> String {
    match std::str::from_utf8(preimage) {
        Ok(text) if !text.contains('\0') => hex(text.as_bytes()),
        _ => "null".to_string(),
    }
}

fn screaming_snake(magic: KnownMagic) -> String {
    let mut name = String::new();
    for (i, c) in format!("{magic:?}").chars().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            name.push('_');
        }
        name.push(c.to_ascii_uppercase());
    }
    name
}

fn decoded(decode: &Result<Vec<RainMetaDocumentV1Item>, Error>) -> String {
    match decode {
        Ok(items) => {
            let mut line = format!("OK {}", items.len());
            for i in items {
                let schema = i.schema.as_deref().map(str::as_bytes);
                line.push_str(&format!(
                    ";{:016x},{},{:?},{:?},{:?},{},{},{}",
                    i.magic as u64,
                    hex(i.payload.as_ref()),
                    i.content_type,
                    i.content_encoding,
                    i.content_language,
                    preimage_string(i.payload.as_ref()),
                    schema.map_or("null".to_string(), hex),
                    schema.map_or("null".to_string(), preimage_string)
                ));
            }
            line
        }
        Err(Error::NotRainMetaDocument) => "NotRainMetaDocument".to_string(),
        Err(_) => "MalformedDocument".to_string(),
    }
}

fn verified(
    decode: &Result<Vec<RainMetaDocumentV1Item>, Error>,
    checks: &[(u64, [u8; 32])],
) -> String {
    let Ok(items) = decode else {
        return "UNVERIFIED".to_string();
    };
    let mut preimages = vec![];
    for &(magic, key) in checks {
        let of: Vec<&RainMetaDocumentV1Item> =
            items.iter().filter(|i| i.magic as u64 == magic).collect();
        let good = |i: &&RainMetaDocumentV1Item| {
            matches!(
                i.content_encoding,
                ContentEncoding::None | ContentEncoding::Identity
            ) && keccak256(i.payload.as_ref()).0 == key
        };
        match of.last() {
            Some(last) if of.iter().all(good) => preimages.push(hex(last.payload.as_ref())),
            _ => return "UNVERIFIED".to_string(),
        }
    }
    format!("VERIFIED {}", preimages.join(","))
}

fn emit(label: impl std::fmt::Display, doc: &[u8], checks: &[(u64, [u8; 32])], mode: &str) {
    let out = match mode {
        "docs" => format!(
            "{}\t{}",
            hex(doc),
            checks
                .iter()
                .map(|(magic, key)| format!("{magic:016x}:{}", hex(key)))
                .collect::<Vec<_>>()
                .join(";")
        ),
        "decode" => {
            let decode = RainMetaDocumentV1Item::cbor_decode(doc);
            format!("{}\t{}", decoded(&decode), verified(&decode, checks))
        }
        m => panic!("unknown mode {m}"),
    };
    println!("{label}\t{out}");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cases = args.next().expect("cases.txt path, fuzz or magics");
    if cases == "magics" {
        let mut named: Vec<String> = KnownMagic::iter()
            .map(|m| format!("{} {:016x}", screaming_snake(m), m as u64))
            .collect();
        named.sort();
        for line in named {
            println!("{line}");
        }
        for m in KnownMagic::iter().filter(|m| KnownMagic::try_from(*m as u64).is_ok()) {
            println!("KNOWN_MAGICS {:016x}", m as u64);
        }
        return;
    }
    if cases == "fuzz" {
        let seed: u64 = args.next().expect("seed").parse().expect("seed");
        let count: u64 = args.next().expect("count").parse().expect("count");
        let mode = args.next().expect("mode: docs | decode");
        for i in 0..count {
            let (doc, checks) = fuzz::document(seed, i);
            emit(i, &doc, &checks, &mode);
        }
        return;
    }
    let mode = args.next().expect("mode: docs | decode");
    let checks = checks();
    let text = std::fs::read_to_string(cases).expect("read cases");
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with("## ") {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let body = expand(&words[1..]);
        let doc = match words[0] {
            "value" => format!("{P}{FK5}{body}{}{}", name(), type_item()),
            "item" => format!("{P}{body}{}{}", name(), type_item()),
            "name" => format!("{P}{body}{}", type_item()),
            "doc" => body,
            g => panic!("unknown group {g}"),
        };
        assert!(doc.len() % 2 == 0, "odd hex: {line}");
        emit(n + 1, &unhex(&doc), &checks, &mode);
    }
}
