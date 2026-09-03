use clap::Parser;
use anyhow::anyhow;
use itertools::izip;
use std::path::PathBuf;
use crate::cli::output::SupportedOutputEncoding;
use crate::meta::{
    RainMetaDocumentV1Item, KnownMeta, ContentType, ContentEncoding, ContentLanguage,
    magic::KnownMagic,
};

/// command for building rain meta
#[derive(Parser)]
pub struct Build {
    /// Output path. If not specified, the output is written to stdout.
    #[arg(short, long)]
    output_path: Option<PathBuf>,
    /// Output encoding. If not specified, the output is written in binary format.
    #[arg(short = 'E', long, default_value = "binary")]
    output_encoding: SupportedOutputEncoding,
    /// Global magic number. If not specified, the default magic number is used.
    /// The default magic number is rain-meta-document-v1. Don't change this
    /// unless you know what you are doing.
    #[arg(short = 'M', long, default_value = "rain-meta-document-v1")]
    global_magic: KnownMagic,
    /// Sequence of input paths. The number of input paths must match the number
    /// of magic numbers, content types, content encodings and content languages.
    /// Reading from stdin is not supported but proccess substitution can be used.
    #[arg(short, long, num_args = 1..)]
    input_path: Vec<PathBuf>,
    /// Sequence of magic numbers. The number of magic numbers must match the
    /// number of input paths, content types, content encodings and content languages.
    /// Magic numbers are arbitrary byte sequences used to build self-describing
    /// payloads.
    #[arg(short, long, num_args = 1..)]
    magic: Vec<KnownMagic>,
    /// Sequence of content types. The number of content types must match the
    /// number of input paths, magic numbers, content encodings and content languages.
    /// Content type is as per http headers.
    #[arg(short = 't', long, num_args = 1..)]
    content_type: Vec<ContentType>,
    /// Sequence of content encodings. The number of content encodings must match the
    /// number of input paths, magic numbers, content types and content languages.
    /// Content encoding is as per http headers.
    #[arg(short = 'e', long, num_args = 1..)]
    content_encoding: Vec<ContentEncoding>,
    /// Sequence of content languages. The number of content languages must match the
    /// number of input paths, magic numbers, content types and content encodings.
    /// Content language is as per http headers.
    #[arg(short = 'l', long, num_args = 1..)]
    content_language: Vec<ContentLanguage>,
}

/// Temporary housing for raw data before it is converted into a RainMetaDocumentV1Item.
#[derive(Clone, Debug)]
pub struct BuildItem {
    /// Raw data. Ostensibly this is the content of a file.
    pub data: Vec<u8>,
    /// Magic number taken from build options.
    pub magic: KnownMagic,
    /// Content type taken from build options.
    pub content_type: ContentType,
    /// Content encoding taken from build options.
    pub content_encoding: ContentEncoding,
    /// Content language taken from build options.
    pub content_language: ContentLanguage,
}

/// Moving from a BuildItem to a RainMetaDocumentV1Item requires normalization
/// according to the magic number and encoding from the build options.
impl TryFrom<&BuildItem> for RainMetaDocumentV1Item {
    type Error = anyhow::Error;
    fn try_from(item: &BuildItem) -> anyhow::Result<Self> {
        let normalized = TryInto::<KnownMeta>::try_into(item.magic)?.normalize(&item.data)?;
        let encoded = item.content_encoding.encode(&normalized);
        Ok(RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from(encoded),
            magic: item.magic,
            content_type: item.content_type,
            content_encoding: item.content_encoding,
            content_language: item.content_language,
            schema: None,
        })
    }
}

/// Build a rain meta document from a sequence of BuildItems.
pub fn build_bytes(magic: KnownMagic, items: Vec<BuildItem>) -> anyhow::Result<Vec<u8>> {
    let mut metas: Vec<RainMetaDocumentV1Item> = vec![];
    for item in items {
        metas.push(RainMetaDocumentV1Item::try_from(&item)?);
    }
    Ok(RainMetaDocumentV1Item::cbor_encode_seq(&metas, magic)?)
}

/// Build a rain meta document from command line options.
/// Enforces length constraints on the input paths, magic numbers, content types,
/// content encodings and content languages.
/// Handles reading input files and writing to files/stdout according to the
/// build options.
pub fn build(b: Build) -> anyhow::Result<()> {
    if b.input_path.len() != b.magic.len() {
        return Err(anyhow!(
            "{} inputs does not match {} magic numbers.",
            b.input_path.len(),
            b.magic.len()
        ));
    }

    if b.input_path.len() != b.content_type.len() {
        return Err(anyhow!(
            "{} inputs does not match {} content types.",
            b.input_path.len(),
            b.content_type.len()
        ));
    }

    if b.input_path.len() != b.content_encoding.len() {
        return Err(anyhow!(
            "{} inputs does not match {} content encodings.",
            b.input_path.len(),
            b.content_encoding.len()
        ));
    }

    if b.input_path.len() != b.content_language.len() {
        return Err(anyhow!(
            "{} inputs does not match {} content languages.",
            b.input_path.len(),
            b.content_language.len()
        ));
    }
    let mut items: Vec<BuildItem> = vec![];
    for (input_path, magic, content_type, content_encoding, content_language) in izip!(
        b.input_path.iter(),
        b.magic.iter(),
        b.content_type.iter(),
        b.content_encoding.iter(),
        b.content_language.iter()
    ) {
        items.push(BuildItem {
            data: std::fs::read(input_path)?,
            magic: *magic,
            content_type: *content_type,
            content_encoding: *content_encoding,
            content_language: *content_language,
        });
    }
    crate::cli::output::output(
        &b.output_path,
        b.output_encoding,
        &build_bytes(b.global_magic, items)?,
    )
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use strum::IntoEnumIterator;
    use crate::meta::{
        magic::{self, KnownMagic},
        ContentType, ContentEncoding, ContentLanguage, RainMetaDocumentV1Item,
    };
    use super::BuildItem;
    use super::build_bytes;
    use crate::meta::types::authoring::v1::AuthoringMeta;

    const AUTHORING_META_V1_JSON: &str = r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#;

    /// Test that the magic number prefix is correct for all known magic numbers
    /// in isolation from all build items.
    #[test]
    fn test_build_empty() -> anyhow::Result<()> {
        for global_magic in magic::KnownMagic::iter() {
            let built_bytes = build_bytes(global_magic, vec![])?;
            assert_eq!(built_bytes, global_magic.to_prefix_bytes());
        }
        Ok(())
    }

    /// We can build a single document item from a single build item. A
    /// dotrain-v1 payload passes through normalisation untouched, so this
    /// tests the item construction and nothing else.
    #[test]
    fn test_into_meta_document() -> anyhow::Result<()> {
        let build_item = BuildItem {
            data: "[]".as_bytes().to_vec(),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::En,
        };

        let meta_document = RainMetaDocumentV1Item::try_from(&build_item)?;
        let expected_meta_document = RainMetaDocumentV1Item {
            payload: serde_bytes::ByteBuf::from("[]".as_bytes().to_vec()),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::None,
            content_language: ContentLanguage::En,
            schema: None,
        };
        assert_eq!(meta_document, expected_meta_document);
        Ok(())
    }

    /// The final CBOR bytes are as expected for a single json content type
    /// item. A dotrain-v1 payload passes through normalisation untouched, so
    /// the asserted bytes are all envelope.
    #[test]
    fn test_empty_item() -> anyhow::Result<()> {
        let build_item = BuildItem {
            data: "[]".as_bytes().to_vec(),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::Identity,
            content_language: ContentLanguage::En,
        };

        let bytes = super::build_bytes(KnownMagic::RainMetaDocumentV1, vec![build_item.clone()])?;

        // https://github.com/rainprotocol/specs/blob/main/metadata-v1.md#example
        // 8 byte magic number prefix
        assert_eq!(
            &bytes[0..8],
            KnownMagic::RainMetaDocumentV1.to_prefix_bytes()
        );
        // cbor map with 5 keys
        assert_eq!(bytes[8], 0xa5);
        // key 0
        assert_eq!(bytes[9], 0x00);
        // major type 2 (bytes) length 2
        assert_eq!(bytes[10], 0b010_00010);
        // payload
        assert_eq!(bytes[11..13], "[]".as_bytes()[..]);
        // key 1
        assert_eq!(bytes[13], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(bytes[14], 0b000_11011);
        // magic number
        assert_eq!(&bytes[15..23], KnownMagic::DotrainV1.to_prefix_bytes());
        // key 2
        assert_eq!(bytes[23], 0x02);
        // text string application/json length 16
        assert_eq!(bytes[24], 0b011_10000);
        // the string application/json
        assert_eq!(&bytes[25..41], "application/json".as_bytes());
        // key 3
        assert_eq!(bytes[41], 0x03);
        // text string identity length 8
        assert_eq!(bytes[42], 0b011_01000);
        // the string identity
        assert_eq!(&bytes[43..51], "identity".as_bytes());
        // key 4
        assert_eq!(bytes[51], 0x04);
        // text string en length 2
        assert_eq!(bytes[52], 0b011_00010);
        // the string en
        assert_eq!(&bytes[53..55], "en".as_bytes());

        assert_eq!(bytes.len(), 55);

        Ok(())
    }

    #[test]
    fn test_cbor_encoding_type() -> anyhow::Result<()> {
        let build_item = BuildItem {
            data: "[]".as_bytes().to_vec(),
            magic: KnownMagic::DotrainV1,
            content_type: ContentType::Cbor,
            content_encoding: ContentEncoding::Identity,
            content_language: ContentLanguage::En,
        };

        let bytes = super::build_bytes(KnownMagic::RainMetaDocumentV1, vec![build_item.clone()])?;

        // https://github.com/rainprotocol/specs/blob/main/metadata-v1.md#example
        // 8 byte magic number prefix
        assert_eq!(
            &bytes[0..8],
            KnownMagic::RainMetaDocumentV1.to_prefix_bytes()
        );
        // cbor map with 5 keys
        assert_eq!(bytes[8], 0xa5);
        // key 0
        assert_eq!(bytes[9], 0x00);
        // major type 2 (bytes) length 2
        assert_eq!(bytes[10], 0b010_00010);
        // payload
        assert_eq!(bytes[11..13], "[]".as_bytes()[..]);
        // key 1
        assert_eq!(bytes[13], 0x01);
        // major type 0 (unsigned integer) value 27
        assert_eq!(bytes[14], 0b000_11011);
        // magic number
        assert_eq!(&bytes[15..23], KnownMagic::DotrainV1.to_prefix_bytes());
        // key 2
        assert_eq!(bytes[23], 0x02);
        // text string application/cbor length 16
        assert_eq!(bytes[24], 0b011_10000);
        // the string application/cbor
        assert_eq!(&bytes[25..41], "application/cbor".as_bytes());
        // key 3
        assert_eq!(bytes[41], 0x03);
        // text string identity length 8
        assert_eq!(bytes[42], 0b011_01000);
        // the string identity
        assert_eq!(&bytes[43..51], "identity".as_bytes());
        // key 4
        assert_eq!(bytes[51], 0x04);
        // text string en length 2
        assert_eq!(bytes[52], 0b011_00010);
        // the string en
        assert_eq!(&bytes[53..55], "en".as_bytes());

        assert_eq!(bytes.len(), 55);

        Ok(())
    }

    use clap::Parser;
    use std::io::Write;
    use super::{Build, build};

    /// Conversion normalizes the payload for the item's magic and then
    /// applies the content encoding.
    #[test]
    fn test_item_normalize_then_encode() -> anyhow::Result<()> {
        // Json authoring meta normalizes to its abi encoding, then deflates.
        let build_item = BuildItem {
            data: AUTHORING_META_V1_JSON.as_bytes().to_vec(),
            magic: KnownMagic::AuthoringMetaV1,
            content_type: ContentType::Json,
            content_encoding: ContentEncoding::Deflate,
            content_language: ContentLanguage::En,
        };
        let meta_document = RainMetaDocumentV1Item::try_from(&build_item)?;
        let abi =
            serde_json::from_str::<AuthoringMeta>(AUTHORING_META_V1_JSON)?.abi_encode_validate()?;
        assert_ne!(abi, build_item.data);
        assert_eq!(
            meta_document.payload.as_ref(),
            ContentEncoding::Deflate.encode(&abi)
        );

        // Un-normalizable data is rejected.
        let invalid_item = BuildItem {
            data: "not json".as_bytes().to_vec(),
            ..build_item
        };
        assert!(RainMetaDocumentV1Item::try_from(&invalid_item).is_err());
        Ok(())
    }

    fn parse_build(args: &[&str]) -> Build {
        Build::try_parse_from(args).unwrap()
    }

    /// Each arity guard fires with its own message, before any file IO:
    /// the input path never exists and yet the mismatch is what errors.
    #[test]
    fn test_build_arity_guards() {
        let b = parse_build(&[
            "build",
            "-i",
            "does-not-exist.json",
            "-m",
            "authoring-meta-v1",
            "-m",
            "authoring-meta-v1",
        ]);
        assert_eq!(
            build(b).unwrap_err().to_string(),
            "1 inputs does not match 2 magic numbers."
        );

        let b = parse_build(&[
            "build",
            "-i",
            "does-not-exist.json",
            "-m",
            "authoring-meta-v1",
            "-t",
            "json",
            "-t",
            "json",
        ]);
        assert_eq!(
            build(b).unwrap_err().to_string(),
            "1 inputs does not match 2 content types."
        );

        let b = parse_build(&[
            "build",
            "-i",
            "does-not-exist.json",
            "-m",
            "authoring-meta-v1",
            "-t",
            "json",
            "-e",
            "identity",
            "-e",
            "identity",
        ]);
        assert_eq!(
            build(b).unwrap_err().to_string(),
            "1 inputs does not match 2 content encodings."
        );

        let b = parse_build(&[
            "build",
            "-i",
            "does-not-exist.json",
            "-m",
            "authoring-meta-v1",
            "-t",
            "json",
            "-e",
            "identity",
            "-l",
            "en",
            "-l",
            "en",
        ]);
        assert_eq!(
            build(b).unwrap_err().to_string(),
            "1 inputs does not match 2 content languages."
        );
    }

    /// build() reads each input file, builds the document under the
    /// global magic and writes it to the output path; the hex output
    /// encoding is honored.
    #[test]
    fn test_build_reads_files_and_encodes_output() -> anyhow::Result<()> {
        let mut input = tempfile::NamedTempFile::new()?;
        input.write_all(AUTHORING_META_V1_JSON.as_bytes())?;
        let output = tempfile::NamedTempFile::new()?;

        let expected = build_bytes(
            KnownMagic::RainMetaDocumentV1,
            vec![BuildItem {
                data: AUTHORING_META_V1_JSON.as_bytes().to_vec(),
                magic: KnownMagic::AuthoringMetaV1,
                content_type: ContentType::Json,
                content_encoding: ContentEncoding::Identity,
                content_language: ContentLanguage::En,
            }],
        )?;

        let input_path = input.path().to_str().unwrap().to_string();
        let output_path = output.path().to_str().unwrap().to_string();

        let b = parse_build(&[
            "build",
            "-i",
            &input_path,
            "-m",
            "authoring-meta-v1",
            "-t",
            "json",
            "-e",
            "identity",
            "-l",
            "en",
            "-o",
            &output_path,
        ]);
        build(b)?;
        assert_eq!(std::fs::read(output.path())?, expected);

        let b = parse_build(&[
            "build",
            "-i",
            &input_path,
            "-m",
            "authoring-meta-v1",
            "-t",
            "json",
            "-e",
            "identity",
            "-l",
            "en",
            "-o",
            &output_path,
            "-E",
            "hex",
        ]);
        build(b)?;
        assert_eq!(
            std::fs::read_to_string(output.path())?,
            alloy::primitives::hex::encode_prefixed(&expected)
        );
        Ok(())
    }
}
