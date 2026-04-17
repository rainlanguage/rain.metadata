# Rain Metadata Tooling
A library that provides tools and utilities to work with RainLanguage metadata. Dotrain LSP/compiler, Raindex, and others use this library.

Also provides CLI app (executable binary) to generate desireable Rain cbor encoded metadata based on [Metadata Specs](https://github.com/rainprotocol/specs/blob/main/metadata-v1.md) which for example is used in Rain deployment CI.
<br>

## Features
`cli` and `json-schema` features are default however, in most cases non of the features are needed for using the lib crate, so they can be disabled by using `default-features = false`, just be aware that `cli` feature is required for building the binary.

- `cli`: A [clap](https://docs.rs/clap/latest/clap/) based CLI crate for functionalities of this library, this feature has [tokio](https://docs.rs/tokio/latest/tokio/) dependency with features enabled that are compatible for `wasm` family target builds. Enabling this feature will also enable `json-schema` feature, This feature is required for building the binary crate.
- `json-schema`: Enables implementation of Json Schema for different types of Rain meta.
- `tokio-full`: Installs [tokio](https://docs.rs/tokio/latest/tokio/) with full features which is a dependency of `cli` feature, this allows for multi-threading of the CLI app, however it results in erroneous builds for `wasm` target family as explained in [tokio docs](https://docs.rs/tokio/latest/tokio/#wasm-support), this feature is only effective for binary crate and using it for lib crate just installs [tokio](https://docs.rs/tokio/latest/tokio/) with its full features as a dependency as the entire lib crate doesn't depend on [tokio](https://docs.rs/tokio/latest/tokio/). This is because [tokio](https://docs.rs/tokio/latest/tokio/) is only used as a runtime for binray crate.
<br>

## CLI (Binary Crate)
    Tooling and utilities for RainLanguage metadata.

    Usage: rain-metadata <COMMAND>

    Commands:
      schema    command related to meta json schema
      validate  command for validating a meta
      magic     command related to rain magic numbers
      build     command for building rain meta
      solc      command related to solc artifacts
      subgraph  command related to subgraphs
      help      Print this message or the help of the given subcommand(s)

    Options:
      -h, --help     Print help
      -V, --version  Print version
<br>

## Examples
### Encode/Decode
Create a meta item and cbor encode/decode it:
```rust
let authoring_meta_content = r#"[
    {
        "word": "stack",
        "description": "Copies an existing value from the stack.",
        "operandParserOffset": 16
    },
    {
        "word": "constant",
        "description": "Copies a constant value onto the stack.",
        "operandParserOffset": 16
    }
]"#;
let authoring_meta: AuthoringMeta = serde_json::from_str(authoring_meta_content)?;

// abi encode the authoring meta with performing validation
let authoring_meta_abi_encoded = authoring_meta.abi_encode_validate()?;

// Constructing a RainMeta item (cbor map)
let meta_map = RainMetaDocumentV1Item {
    payload: serde_bytes::ByteBuf::from(authoring_meta_abi_encoded),
    magic: KnownMagic::AuthoringMetaV1,
    content_type: ContentType::Cbor,
    content_encoding: ContentEncoding::None,
    content_language: ContentLanguage::None,
};

// cbor encode the meta item
let cbor_encoded = meta_map.cbor_encode()?;

// decode the data back
let cbor_decoded_vec = RainMetaDocumentV1Item::cbor_decode(&cbor_encoded)?;

// unpack the payload into AuthoringMeta
let unpacked_payload: AuthoringMeta = cbor_decoded_vec[0].unpack_into()?;
```
<br>

### Meta Storage (CAS)
`Store` is a struct that provides functionalities to store, read, fetch(remotely) Rain metadata and ExpressionDeployers as a Content Addressed Storage(CAS), which is a critical piece of dotrain language server protocol and compiler implementation by caching all that is imported in .rain file that can be later accessed easily by language server and compiler.
```rust
use rain_meta::meta::Store;
use std::collections::HashMap;

// to instatiate with default rain subgraphs included
let mut store = Store::default();

// add a new subgraph endpoint url to the subgraph list
store.add_subgraphs(&vec!["subgraph-url"]);

// update the store with another Store (merges the stores)
store.merge(&Store::default());

// hash of a meta to search and store
let hash = "0x56ffc3fc82109c33f1e1544157a70144fc15e7c6e9ae9c65a636fd165b1bc51c";

// updates the meta store with a new meta by searching through subgraphs
store.update(&hash);

// searches for an deployer in the subgraphs and stores the result mapped to the hash
store.search_deployer(&hash);

// to get a record from store
let meta = store.get_meta(&hash);

// to get a ExpressionDeployer record from store
let deployer_record = store.get_deployer(&hash);

// path to a .rain file
let dotrain_uri = "path/to/file.rain";

// reading the dotrain content as an example,
// Store is agnostic to dotrain contents it just maps the hash of the content to the given
// uri and puts it as a new meta into the meta cache, so obtaining and passing the correct
// content is up to the implementer
let dotrain_content = std::fs::read_to_string(&dotrain_uri);

// updates the dotrain cache for a dotrain text and uri
let (new_hash, old_hash) = store.set_dotrain(&dotrain_content, &dotrain_uri, false)?;

// to get dotrain meta bytes given a uri
let dotrain_meta_bytes = store.get_dotrain_meta(&dotrain_uri);
```
<br>

## Developers
To build from source first clone this repo and make sure you already have `rustup` installed and next you can build the lib/binary using `cargo build`, if you have nixOS installed you can simply run:
```bash
nix build
```
to build the binary crate or enter the nix with:
```bash
nix develop
```
which will fetch all the required packages and put them in your path and you can proceed to build lib/binary crate using `cargo build`.
for running test use `cargo test` and for generating docs use `cargo doc`.