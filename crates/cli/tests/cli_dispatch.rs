#![cfg(not(target_family = "wasm"))]

//! End-to-end coverage for the 8 `cli::dispatch` subcommand arms, driven
//! through the compiled `rain-metadata` binary so each arm's routing is
//! observable as process output and exit status rather than as an in-process
//! call. Every assertion is on a concrete value (exact lines, exact JSON,
//! exact prefixes), so an arm that silently no-ops is discriminated from the
//! real routing, not just "did not error".

use std::process::Output;

fn run(args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_rain-metadata"))
        .args(args)
        .output()
        .expect("failed to spawn rain-metadata binary")
}

fn stdout_utf8(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout not utf8")
}

/// `schema ls` routes to `schema::dispatch` and prints every `KnownMeta`
/// kebab-case identifier, one per line, in declaration order.
#[test]
fn test_dispatch_schema_ls() {
    let out = run(&["schema", "ls"]);
    assert!(out.status.success());
    let expected = "\
op-v1
dotrain-v1
rainlang-v1
solidity-abi-v2
authoring-meta-v1
authoring-meta-v2
interpreter-caller-meta-v1
expression-deployer-v2-bytecode-v1
rainlang-source-v1
address-list
dotrain-source-v1
order-builder-state-v1
raindex-signed-context-oracle-v1
";
    assert_eq!(stdout_utf8(&out), expected);
}

/// `schema show` routes through the same `Schema` subcommand arm and prints
/// a JSON schema document for a supported meta.
#[test]
fn test_dispatch_schema_show() {
    let out = run(&["schema", "show", "authoring-meta-v1"]);
    assert!(out.status.success());
    let schema: serde_json::Value =
        serde_json::from_str(&stdout_utf8(&out)).expect("schema show did not print JSON");
    assert_eq!(
        schema["$schema"].as_str(),
        Some("http://json-schema.org/draft-07/schema#")
    );
    // An unsupported schema is a hard error, not empty output.
    let out = run(&["schema", "show", "dotrain-v1"]);
    assert!(!out.status.success());
}

/// `validate` routes to `validate::validate`: a meta that normalizes is exit
/// 0, one that does not is a non-zero exit. A no-op arm would return exit 0
/// for both.
#[test]
fn test_dispatch_validate() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.json");
    std::fs::write(&good, "[]").unwrap();
    let out = run(&[
        "validate",
        "-m",
        "solidity-abi-v2",
        "-i",
        good.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "this is not abi json").unwrap();
    let out = run(&[
        "validate",
        "-m",
        "solidity-abi-v2",
        "-i",
        bad.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
}

/// `schema-check` routes to `schema_check::schema_check`: an unreadable
/// consumer snapshot is a non-zero exit before any network access. A no-op
/// arm would exit 0.
#[test]
fn test_dispatch_schema_check() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist.graphql");
    let out = run(&["schema-check", "--consumer", missing.to_str().unwrap()]);
    assert!(!out.status.success());

    // Happy path: identical source and consumer entity SDL verifies.
    let sdl = "type Foo @entity {\n  id: Bytes!\n}\n";
    let source = dir.path().join("source.graphql");
    let consumer = dir.path().join("consumer.graphql");
    std::fs::write(&source, sdl).unwrap();
    std::fs::write(&consumer, sdl).unwrap();
    let out = run(&[
        "schema-check",
        "--source",
        source.to_str().unwrap(),
        "--consumer",
        consumer.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert!(stdout_utf8(&out).contains("schema check ok: 1 entities"));
}

/// `magic ls` routes to `magic::dispatch` and prints every `KnownMagic` as
/// `0x<hex> <kebab-name>`, pinning the rain meta document magic to its
/// published literal.
#[test]
fn test_dispatch_magic_ls() {
    let out = run(&["magic", "ls"]);
    assert!(out.status.success());
    let stdout = stdout_utf8(&out);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "0xff0a89c674ee7874 rain-meta-document-v1");
    assert_eq!(lines.len(), 19);
    for line in &lines {
        assert!(line.starts_with("0xff"), "not a magic line: {}", line);
    }
}

/// `build` routes to `build::build`: a single empty-ABI document encodes to a
/// hex payload prefixed with the rain meta document magic number.
#[test]
fn test_dispatch_build() {
    let dir = tempfile::tempdir().unwrap();
    let abi = dir.path().join("abi.json");
    std::fs::write(&abi, "[]").unwrap();
    let out = run(&[
        "build",
        "-i",
        abi.to_str().unwrap(),
        "-m",
        "solidity-abi-v2",
        "-t",
        "json",
        "-e",
        "identity",
        "-l",
        "en",
        "-E",
        "hex",
    ]);
    assert!(out.status.success());
    let stdout = stdout_utf8(&out);
    assert!(
        stdout.starts_with("0xff0a89c674ee7874"),
        "missing magic prefix: {}",
        stdout
    );
    assert!(stdout.len() > "0xff0a89c674ee7874".len());
}

/// `solc artifact` routes to `solc::dispatch` and extracts exactly the
/// requested component of the artifact JSON.
#[test]
fn test_dispatch_solc_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact.json");
    std::fs::write(
        &artifact,
        r#"{"abi":[{"type":"fallback"}],"bytecode":"0x60","deployedBytecode":"0x61"}"#,
    )
    .unwrap();
    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "abi",
        "-i",
        artifact.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert_eq!(stdout_utf8(&out), r#"[{"type":"fallback"}]"#);

    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "deployed-bytecode",
        "-i",
        artifact.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert_eq!(stdout_utf8(&out), r#""0x61""#);
}

/// `subgraph` routes to `subgraph::dispatch`: `all` prints all 9 known URLs,
/// `chain` prints the 3 URLs of a supported chain and hard-errors on an
/// unsupported one.
#[test]
fn test_dispatch_subgraph() {
    let out = run(&["subgraph", "all"]);
    assert!(out.status.success());
    let stdout = stdout_utf8(&out);
    assert_eq!(stdout.lines().count(), 9);
    assert!(stdout.contains(
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-ethereum"
    ));

    let out = run(&["subgraph", "chain", "137"]);
    assert!(out.status.success());
    let stdout = stdout_utf8(&out);
    assert_eq!(stdout.lines().count(), 3);
    assert!(stdout.contains("interpreter-registry-polygon"));

    let out = run(&["subgraph", "chain", "2"]);
    assert!(!out.status.success());
}

/// `generate` routes to `generate::generate`: dotrain source content becomes
/// a JSON blob whose meta bytes carry the rain meta document magic prefix,
/// and empty content is a hard error. A no-op arm would exit 0 for both and
/// print nothing.
#[test]
fn test_dispatch_generate() {
    let dir = tempfile::tempdir().unwrap();
    let rain = dir.path().join("source.rain");
    std::fs::write(&rain, "#calculate-io\n_ _: 1 2;").unwrap();
    let out = run(&["generate", "source", "-i", rain.to_str().unwrap()]);
    assert!(out.status.success());
    let value: serde_json::Value =
        serde_json::from_str(&stdout_utf8(&out)).expect("generate did not print JSON");
    assert!(value["subject"].as_str().unwrap().starts_with("0x"));
    assert!(value["meta_bytes"]
        .as_str()
        .unwrap()
        .starts_with("0xff0a89c674ee7874"));
    assert!(value["calldata"].as_str().unwrap().starts_with("0x"));

    let empty = dir.path().join("empty.rain");
    std::fs::write(&empty, "   \n").unwrap();
    let out = run(&["generate", "source", "-i", empty.to_str().unwrap()]);
    assert!(!out.status.success());
}
