//! The seam between the two halves of this repo: rust builds `emitMeta`
//! calldata, solidity's `IMetaBoardV1_2` decides whether it is acceptable.
//! Each half was tested against its own idea of the bytes and neither against
//! the other, which is how `generate_emit_meta_calldata` came to build a bare
//! cbor map that `LibMeta.checkMetaUnhashedV1` reverts `NotRainMetaV1` on.
//!
//! This writes the calldata rust actually produces to a committed fixture.
//! `test/lib/EmitCalldataFixture.t.sol` reads that fixture and sends it to a
//! real `TestMetaBoard`, so the contract itself is what says the bytes are
//! acceptable rather than an assertion here restating what the encoder did.
//!
//! Regenerate with `BLESS=1 cargo test -p rain-metadata --test
//! emit_calldata_fixture`. Without `BLESS` the test asserts the committed
//! fixture still matches, so CI fails if the two drift apart.

use std::{fs, path::PathBuf};

use rain_metadata::{
    ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item,
    generate_dotrain_source_emit_tx_data, generate_emit_meta_calldata,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/fixtures/emit-calldata.json")
}

/// A meta item with every optional field defaulted, so the fixture pins the
/// shortest encoding rather than an unusually decorated one.
fn plain_item(content: &str) -> RainMetaDocumentV1Item {
    RainMetaDocumentV1Item {
        payload: serde_bytes::ByteBuf::from(content.as_bytes().to_vec()),
        magic: KnownMagic::DotrainSourceV1,
        content_type: ContentType::OctetStream,
        content_encoding: ContentEncoding::None,
        content_language: ContentLanguage::None,
        schema: None,
    }
}

/// Every case solidity should be handed. Keyed by name so a failure names the
/// case rather than an index.
fn cases() -> Vec<(String, String)> {
    let mut out = Vec::new();

    let generic = generate_emit_meta_calldata(plain_item("emit calldata fixture")).unwrap();
    out.push((
        "generic_item".to_string(),
        alloy::hex::encode_prefixed(generic),
    ));

    let dotrain = generate_dotrain_source_emit_tx_data("#main _ _: int-add(1 2);").unwrap();
    out.push(("dotrain_source".to_string(), dotrain.calldata));

    out
}

#[test]
fn emit_calldata_fixture_is_current() {
    let map: serde_json::Map<String, serde_json::Value> = cases()
        .into_iter()
        .map(|(name, calldata)| (name, serde_json::Value::String(calldata)))
        .collect();
    let generated = serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap() + "\n";

    let path = fixture_path();
    if std::env::var("BLESS").is_ok() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &generated).unwrap();
        return;
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} is missing ({error}). Regenerate with BLESS=1 cargo test -p rain-metadata --test emit_calldata_fixture",
            path.display()
        )
    });

    assert_eq!(
        committed, generated,
        "committed emit calldata is stale. Regenerate with BLESS=1 cargo test -p rain-metadata --test emit_calldata_fixture"
    );
}
