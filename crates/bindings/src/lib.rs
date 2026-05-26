use alloy::sol;

sol!(
    #![sol(all_derives = true)]
    IDescribedByMetaV1,
    concat!(env!("CARGO_MANIFEST_DIR"), "/abi/IDescribedByMetaV1.json")
);

sol!(
    #![sol(all_derives = true)]
    MetaBoard,
    concat!(env!("CARGO_MANIFEST_DIR"), "/abi/MetaBoard.json")
);
