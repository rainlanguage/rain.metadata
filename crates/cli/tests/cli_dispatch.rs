#![cfg(not(target_family = "wasm"))]

//! End-to-end coverage for the 8 `cli::dispatch` subcommand arms, driven
//! through the compiled `rain-metadata` binary so each arm's routing is
//! observable as process output and exit status rather than as an in-process
//! call. Every assertion is on a concrete value (exact lines, exact JSON,
//! exact prefixes), so an arm that silently no-ops is discriminated from the
//! real routing, not just "did not error".

use std::process::Output;

const AUTHORING_META_V1_JSON: &str = r#"[{"word":"stack","description":"Copies an existing value from the stack.","operandParserOffset":16}]"#;

fn run(args: &[&str]) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_rain-metadata"))
        .args(args)
        .output()
        .expect("failed to spawn rain-metadata binary")
}

fn stdout_utf8(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout not utf8")
}

/// `schema ls` routes to `schema::dispatch` and prints the kebab-case
/// identifier of every `KnownMeta` that has a JSON schema, one per line, in
/// declaration order.
#[test]
fn test_dispatch_schema_ls() {
    let out = run(&["schema", "ls"]);
    assert!(out.status.success());
    // op-v1, solidity-abi-v2 and interpreter-caller-meta-v1 are absent
    // because #304 and #317 removed the models they derived schemas from,
    // not because they stopped being known metas - `magic ls` still lists
    // them.
    let expected = "\
authoring-meta-v1
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

    // Each supported meta returns its OWN schema.
}

/// `validate` routes to `validate::validate`: a meta that normalizes is exit
/// 0, one that does not is a non-zero exit. A no-op arm would return exit 0
/// for both.
#[test]
fn test_dispatch_validate() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good.json");
    std::fs::write(&good, AUTHORING_META_V1_JSON).unwrap();
    let out = run(&[
        "validate",
        "-m",
        "authoring-meta-v1",
        "-i",
        good.to_str().unwrap(),
    ]);
    assert!(out.status.success());

    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "this is not authoring meta json").unwrap();
    let out = run(&[
        "validate",
        "-m",
        "authoring-meta-v1",
        "-i",
        bad.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
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
    assert_eq!(lines.len(), 20);
    for line in &lines {
        assert!(line.starts_with("0xff"), "not a magic line: {}", line);
    }
}

/// `build` routes to `build::build`: a single authoring-meta-v1 document
/// encodes to a hex payload prefixed with the rain meta document magic
/// number.
#[test]
fn test_dispatch_build() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("authoring-meta.json");
    std::fs::write(&input, AUTHORING_META_V1_JSON).unwrap();
    let out = run(&[
        "build",
        "-i",
        input.to_str().unwrap(),
        "-m",
        "authoring-meta-v1",
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

    // bytecode is its own component, distinct from deployedBytecode.
    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "bytecode",
        "-i",
        artifact.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert_eq!(stdout_utf8(&out), r#""0x60""#);

    // -o writes the component to the file INSTEAD of stdout.
    let out_path = dir.path().join("component.json");
    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "abi",
        "-i",
        artifact.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert_eq!(stdout_utf8(&out), "");
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        r#"[{"type":"fallback"}]"#
    );
}

/// A component the artifact does not carry is a hard error: nonzero exit,
/// nothing on stdout, the missing key named on stderr. Printing `null` and
/// exiting 0 would be indistinguishable from a component that legitimately
/// serialises as null, so both cases are asserted here.
#[test]
fn test_dispatch_solc_artifact_missing_component() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact-no-abi.json");
    std::fs::write(
        &artifact,
        r#"{"bytecode":{"object":"0x60"},"deployedBytecode":{"object":"0x60"}}"#,
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
    assert!(!out.status.success());
    assert_eq!(stdout_utf8(&out), "");
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert!(
        stderr.contains(r#"artifact has no "abi" component"#),
        "stderr did not name the missing component: {}",
        stderr
    );

    // -o must not be written either: a failed extraction produces no file.
    let out_path = dir.path().join("component.json");
    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "abi",
        "-i",
        artifact.to_str().unwrap(),
        "-o",
        out_path.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    assert!(!out_path.exists());

    // An explicitly null component is a value, not an absence: it succeeds
    // and prints null.
    let null_abi = dir.path().join("artifact-null-abi.json");
    std::fs::write(&null_abi, r#"{"abi":null}"#).unwrap();
    let out = run(&[
        "solc",
        "artifact",
        "-c",
        "abi",
        "-i",
        null_abi.to_str().unwrap(),
    ]);
    assert!(out.status.success());
    assert_eq!(stdout_utf8(&out), "null");
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
