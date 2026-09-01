//! End-to-end tests of the rain-metadata binary's stdin/stdout behaviour,
//! which in-process unit tests cannot observe.
#![cfg(not(target_family = "wasm"))]

use rain_metadata::meta::KnownMeta;
use std::io::Write;
use std::process::{Command, Stdio};
use strum::IntoEnumIterator;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rain-metadata"))
}

/// `magic ls` prints every known magic number, in declaration order, as
/// `{:#x} {kebab-name}` lines. Expected list is derived from the magic
/// number table of the rain metadata-v1 spec:
/// https://github.com/rainprotocol/specs/blob/main/metadata-v1.md
#[test]
fn magic_ls_prints_all_known_magic_numbers() {
    let out = bin().args(["magic", "ls"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let expected = "\
0xff0a89c674ee7874 rain-meta-document-v1
0xffe5282f43e495b4 op-meta-v1
0xffdac2f2f37be894 dotrain-v1
0xff1c198cec3b48a7 rainlang-v1
0xffe5ffb4a3ff2cde solidity-abi-v2
0xffe9e3a02ca8e235 authoring-meta-v1
0xff52fe42f1a05093 authoring-meta-v2
0xffc21bbf86cc199b interpreter-caller-meta-v1
0xffdb988a8cd04d32 expression-deployer-v2-bytecode-v1
0xff13109e41336ff2 rainlang-source-v1
0xffb2637608c09e38 address-list
0xffa15ef0fc437099 dotrain-source-v1
0xffda7b2fb167c286 order-builder-state-v1
0xff7a1507ba4419ca raindex-signed-context-oracle-v1
0xff5dcce9b571ba42 web-data-v1
0xffa8e8a9b9cf4a31 oa-schema
0xff9fae3cc645f463 oa-hash-list
0xffc47a6299e8a911 oa-structure
0xff8cd2927c8c86cb oa-token-image
0xffbc38eb14ad2209 oa-token-credential-links
";
    assert_eq!(stdout, expected);
}

/// `schema ls` prints the metas that have a JSON schema, in declaration
/// order, and no meta that does not.
#[test]
fn schema_ls_prints_metas_that_have_a_schema() {
    let out = bin().args(["schema", "ls"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    // op-v1 is absent: #304 removed the model, so there is no type left to
    // derive a schema from. It is still a known meta and `magic ls` still
    // lists it - having a magic number and having a json schema are different
    // things, which is the distinction this listing now draws.
    let expected = "\
solidity-abi-v2
authoring-meta-v1
interpreter-caller-meta-v1
";
    assert_eq!(stdout, expected);
}

/// The listing is a contract with `show`: every name `ls` prints, `show`
/// produces a schema for, and every `KnownMeta` the listing omits, `show`
/// rejects. Asserted over the whole enum, so a variant added to one side
/// only fails here.
#[test]
fn schema_ls_lists_exactly_what_schema_show_accepts() {
    let out = bin().args(["schema", "ls"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let listed: Vec<&str> = stdout.lines().collect();
    assert!(!listed.is_empty());

    for meta in KnownMeta::iter() {
        let name = meta.to_string();
        let out = bin().args(["schema", "show", &name]).output().unwrap();
        if listed.contains(&name.as_str()) {
            assert!(out.status.success(), "listed but unsupported: {name}");
            serde_json::from_slice::<serde_json::Value>(&out.stdout)
                .unwrap_or_else(|_| panic!("listed but not JSON: {name}"));
        } else {
            assert!(!out.status.success(), "unlisted but supported: {name}");
        }
    }
}

/// `schema show` writes the schema to stdout when no output path is
/// given (the stdout branch of cli::output::output).
#[test]
fn schema_show_prints_schema_to_stdout() {
    let out = bin()
        .args(["schema", "show", "authoring-meta-v1"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["title"].is_string());
}

/// op-v1 is still a known meta - it is listed, and its magic number is in
/// the spec table - but this crate models no payload for it, so `schema
/// show` refuses rather than inventing one.
#[test]
fn schema_show_refuses_op_v1() {
    let out = bin().args(["schema", "show", "op-v1"]).output().unwrap();
    assert!(!out.status.success());
}

/// `schema-check` reports the number of verified entities and the source
/// label on success.
#[test]
fn schema_check_prints_verified_entity_count() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("schema.graphql");
    std::fs::write(
        &source,
        "type MetaBoard @entity { id: Bytes! }\ntype MetaV1 @entity { id: ID! }\n",
    )
    .unwrap();
    let consumer = dir.path().join("consumer.graphql");
    std::fs::write(
        &consumer,
        "type MetaBoard { id: Bytes! }\ntype MetaV1 { id: ID! }\n",
    )
    .unwrap();

    let out = bin()
        .args([
            "schema-check",
            "--source",
            source.to_str().unwrap(),
            "--consumer",
            consumer.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout,
        "schema check ok: 2 entities verified against source\n"
    );
}

/// `generate source` reads the dotrain content from stdin when no input
/// path is given, and writes the emit data JSON to stdout when no output
/// path is given.
#[test]
fn generate_source_reads_stdin_and_writes_stdout() {
    let content = "#main _ _: int-add(1 2);";
    let mut child = bin()
        .args(["generate", "source"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let subject = v["subject"].as_str().unwrap();
    assert!(subject.starts_with("0x"));
    assert_eq!(subject.len(), 66);
    // The meta bytes carry the rain meta document magic prefix.
    assert!(v["meta_bytes"]
        .as_str()
        .unwrap()
        .starts_with("0xff0a89c674ee7874"));
    assert!(v["calldata"].as_str().unwrap().starts_with("0x"));
}
