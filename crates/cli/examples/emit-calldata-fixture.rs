// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
//! Writes the `emitMeta` calldata the rust encoders produce to
//! `test/fixtures/emit-calldata.json`, for `test/lib/EmitCalldataFixture.t.sol`
//! to send to a real metaboard.
//!
//! That seam is what neither half tested: rust builds the calldata, solidity
//! decides whether it is acceptable, and each was only ever checked against its
//! own idea of the bytes. `generate_emit_meta_calldata` built a bare cbor map
//! that `LibMeta.checkMetaUnhashedV1` reverts `NotRainMetaV1` on, with both
//! suites green.
//!
//! `script/build.sh` runs this and git-clean diffs the result, so a producer
//! that changes without its fixture being committed turns the lane red. That is
//! the same treatment the committed abis get from `CopyArtifacts.sol`.

use std::{fs, path::PathBuf};

use rain_metadata::{
    ContentEncoding, ContentLanguage, ContentType, KnownMagic, RainMetaDocumentV1Item,
    generate_dotrain_source_emit_tx_data, generate_emit_meta_calldata,
};

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

fn main() {
    let mut cases = serde_json::Map::new();

    let generic = generate_emit_meta_calldata(plain_item("emit calldata fixture")).unwrap();
    cases.insert(
        "generic_item".to_string(),
        alloy::hex::encode_prefixed(generic).into(),
    );

    let dotrain = generate_dotrain_source_emit_tx_data("#main _ _: int-add(1 2);").unwrap();
    cases.insert("dotrain_source".to_string(), dotrain.calldata.into());

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/fixtures/emit-calldata.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::Value::Object(cases)).unwrap() + "\n",
    )
    .unwrap();
}
