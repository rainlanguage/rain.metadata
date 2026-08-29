// End-to-end check of `rain-metadata magic ls` output.
#![cfg(not(target_family = "wasm"))]

use std::process::Command;

/// `magic ls` prints every known magic number, one per line, as the
/// 0x-prefixed hex value followed by the kebab-case name, in declaration
/// order, and nothing else.
#[test]
fn test_magic_ls_exact_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_rain-metadata"))
        .args(["magic", "ls"])
        .output()
        .expect("failed to run rain-metadata");
    assert!(output.status.success(), "exit: {:?}", output.status);
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
0xffa8e8a9b9cf4a31 oa-schema
0xff9fae3cc645f463 oa-hash-list
0xffc47a6299e8a911 oa-structure
0xff8cd2927c8c86cb oa-token-image
0xffbc38eb14ad2209 oa-token-credential-links
";
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}
