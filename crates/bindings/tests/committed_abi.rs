//! The committed ABI artifacts held against what `alloy::sol!` actually
//! consumed.
//!
//! The sol lane already holds `crates/bindings/abi` against
//! `LibCopyArtifacts.contracts()` from both ends and asserts every committed
//! file is a fresh copy of the live forge artifact. What it cannot do is read
//! `crates/bindings/src/lib.rs`, so it restates the consumed set as sol
//! literals: a binding dropped from this crate leaves its file behind as an
//! orphan the copy script still refreshes and the sol lane still counts, and a
//! binding generated from anywhere but the committed copy is invisible in both
//! lanes (rainlanguage/rain.metadata#205).
//!
//! The committed directory is the one thing both lanes can read, so it is the
//! oracle here too — held from the rust end this time, against the crate's own
//! report of what it consumed rather than against a third restatement.
//!
//! Native only: the artifacts are read off the filesystem, which the wasm test
//! lane has none of. What is asserted is a property of the committed tree, not
//! of a target.
#![cfg(not(target_family = "wasm"))]
use alloy::json_abi::{ContractObject, EventParam, JsonAbi, Param};
use rain_metadata_bindings::consumed_artifacts;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// The committed artifact directory, relative to the crate root.
///
/// Its own literal rather than anything the `sol!` calls derive their paths
/// from, so a retarget of the directory is a disagreement between this and the
/// bindings instead of something both sides follow together.
const ABI_DIR: &str = "abi";

fn abi_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ABI_DIR)
}

/// `alloy`'s expander never sets `internalType` (`to_abi::ty_to_param` leaves
/// it `None`), so the generated ABI cannot carry what the artifact on disk
/// does, and it is dropped from both sides rather than compared.
fn strip_internal_types(abi: &mut JsonAbi) {
    fn strip_param(param: &mut Param) {
        param.internal_type = None;
        param.components.iter_mut().for_each(strip_param);
    }
    fn strip_event_param(param: &mut EventParam) {
        param.internal_type = None;
        param.components.iter_mut().for_each(strip_param);
    }
    if let Some(constructor) = abi.constructor.as_mut() {
        constructor.inputs.iter_mut().for_each(strip_param);
    }
    for function in abi.functions.values_mut().flatten() {
        function.inputs.iter_mut().for_each(strip_param);
        function.outputs.iter_mut().for_each(strip_param);
    }
    for event in abi.events.values_mut().flatten() {
        event.inputs.iter_mut().for_each(strip_event_param);
    }
    for error in abi.errors.values_mut().flatten() {
        error.inputs.iter_mut().for_each(strip_param);
    }
}

/// The ABI of the committed artifact for a name, read off disk.
fn committed_abi(name: &str) -> JsonAbi {
    let path = abi_dir().join(format!("{name}.json"));
    let json = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let contract: ContractObject =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut abi = contract
        .abi
        .unwrap_or_else(|| panic!("{}: carries no abi for alloy::sol! to read", path.display()));
    strip_internal_types(&mut abi);
    abi
}

/// The committed directory MUST hold exactly the artifacts this crate consumed.
///
/// Every entry, not every `.json`: a file of any name that no binding reads is
/// the orphan this is here to catch.
#[test]
fn committed_directory_is_exactly_what_the_bindings_consume() {
    let on_disk: BTreeSet<String> = fs::read_dir(abi_dir())
        .unwrap_or_else(|e| panic!("{}: {e}", abi_dir().display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|e| panic!("{}: {e}", abi_dir().display()))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let consumed: BTreeSet<String> = consumed_artifacts()
        .iter()
        .map(|(name, _)| format!("{name}.json"))
        .collect();

    assert_eq!(
        on_disk, consumed,
        "{ABI_DIR} does not hold exactly the artifacts alloy::sol! consumed: a file no binding \
         reads is refreshed by the copy script forever and consumed by nothing, and a binding \
         with no file there is generated from outside the committed set"
    );
}

/// Every binding MUST be generated from the committed artifact of its own name.
///
/// The directory check is satisfied by the files merely existing. This is the
/// half that says the bindings came from THEM: a `sol!` reading the live forge
/// artifact, another name's committed copy, or a copy parked elsewhere leaves
/// the committed file unread while the crate still compiles.
#[test]
fn every_binding_is_generated_from_its_committed_artifact() {
    for (name, generated) in consumed_artifacts() {
        assert_eq!(
            generated,
            committed_abi(name),
            "{name}: the generated bindings are not the abi of its committed artifact"
        );
    }
}
