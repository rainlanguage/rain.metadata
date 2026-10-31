use rain_metadata::{KnownMagic, RainMetaDocumentV1Item};
use strum::IntoEnumIterator;

const P: &str = "ff0a89c674ee7874";
const NP: &str = "737430782e726261632e75732e7631";
const NM: &str = "1bffcdcfae6227d5e9";
const TP: &str = "61646472657373";
const RL: &str = "1bff13109e41336ff2";
const SK: &str = "1bffa8e8a9b9cf4a31";
const FK5: &str = "a3004161011bff13109e41336ff205";

fn name() -> String {
    format!("a2004f{NP}01{NM}")
}

fn type_item() -> String {
    format!("a20047{TP}011bffcdb2cc1b1d53be")
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

fn decoded(doc: &[u8]) -> String {
    match RainMetaDocumentV1Item::cbor_decode(doc) {
        Ok(items) => {
            let mut line = format!("OK {}", items.len());
            for i in items {
                line.push_str(&format!(
                    ";{:016x},{},{:?},{:?},{:?}",
                    i.magic as u64,
                    hex(i.payload.as_ref()),
                    i.content_type,
                    i.content_encoding,
                    i.content_language
                ));
            }
            line
        }
        Err(_) => "ERR".to_string(),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cases = args.next().expect("cases.txt path or magics");
    if cases == "magics" {
        for m in KnownMagic::iter().filter(|m| KnownMagic::try_from(*m as u64).is_ok()) {
            println!("{:016x}", m as u64);
        }
        return;
    }
    let mode = args.next().expect("mode: docs | decode");
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
        let out = match mode.as_str() {
            "docs" => doc,
            "decode" => decoded(&unhex(&doc)),
            m => panic!("unknown mode {m}"),
        };
        println!("{}\t{}", n + 1, out);
    }
}
