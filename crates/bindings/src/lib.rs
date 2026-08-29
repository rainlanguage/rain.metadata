use alloy::sol;

/// One `alloy::sol!` binding per committed ABI artifact, plus the report of
/// which artifacts that was.
///
/// The file is DERIVED from the binding's name rather than spelled per
/// binding. `crates/bindings/abi/<name>.json` is `LibCopyArtifacts.committedPath`,
/// and the sol lane holds that directory against `LibCopyArtifacts.contracts()`
/// — but it cannot read rust, so a path spelled per binding here is a place the
/// two lanes drift apart in silence (rainlanguage/rain.metadata#205).
///
/// `consumed_artifacts` expands from the same token list as the `sol!` calls,
/// so it reports what was consumed rather than a restatement of it.
macro_rules! committed_bindings {
    ($($name:ident),+ $(,)?) => {
        $(
            sol!(
                #![sol(all_derives = true, abi)]
                $name,
                concat!(env!("CARGO_MANIFEST_DIR"), "/abi/", stringify!($name), ".json")
            );
        )+

        /// The committed ABI artifacts this crate consumes, each paired with
        /// the ABI its bindings were generated from.
        pub fn consumed_artifacts() -> Vec<(&'static str, alloy::json_abi::JsonAbi)> {
            vec![$((stringify!($name), $name::abi::contract())),+]
        }
    };
}

committed_bindings!(IDescribedByMetaV1, IMetaBoardV1_2);
