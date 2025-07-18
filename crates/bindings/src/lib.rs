use alloy::sol;

sol!(
    #![sol(all_derives = true)]
    IDescribedByMetaV1,
    "../../out/IDescribedByMetaV1.sol/IDescribedByMetaV1.json"
);

sol!(
    #![sol(all_derives = true)]
    MetaBoard,
    "../../out/MetaBoard.sol/MetaBoard.json"
);
