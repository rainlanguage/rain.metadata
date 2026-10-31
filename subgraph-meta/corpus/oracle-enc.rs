use rain_metadata::RainMetaDocumentV1Item;

fn unhex(s: &str) -> Vec<u8> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn main() {
    for arg in std::env::args().skip(1) {
        let bytes = unhex(&arg);
        match RainMetaDocumentV1Item::cbor_decode(&bytes) {
            Ok(items) => println!(
                "OK {} items: {:?}",
                items.len(),
                items
                    .iter()
                    .map(|i| format!("{:x} {} {:?}", i.magic as u64, hex(i.payload.as_ref()), i.content_encoding))
                    .collect::<Vec<_>>()
            ),
            Err(e) => println!("ERR {:?}", e),
        }
    }
}
