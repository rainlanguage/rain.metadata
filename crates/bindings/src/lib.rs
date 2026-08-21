use alloy::sol;

sol!(
    #![sol(all_derives = true)]
    IDescribedByMetaV1,
    concat!(env!("CARGO_MANIFEST_DIR"), "/abi/IDescribedByMetaV1.json")
);

sol!(
    #![sol(all_derives = true)]
    IMetaBoardV1_2,
    concat!(env!("CARGO_MANIFEST_DIR"), "/abi/IMetaBoardV1_2.json")
);
