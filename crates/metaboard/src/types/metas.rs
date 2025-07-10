use crate::schema;
#[derive(cynic::QueryVariables, Debug)]
pub struct MetasByHashVariables {
    pub metahash: Option<Bytes>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "MetasByHashVariables")]
pub struct MetasByHash {
    #[arguments(where: { metaHash: $metahash })]
    pub meta_v1_s: Vec<MetaV1>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct MetasBySubjectVariables {
    pub subject: Option<BigInt>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "MetasBySubjectVariables")]
pub struct MetasBySubject {
    #[arguments(where: { subject: $subject })]
    pub meta_v1_s: Vec<MetaV1>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct MetaV1 {
    pub meta_hash: Bytes,
    pub meta: Bytes,
    pub sender: Bytes,
    pub id: cynic::Id,
    pub meta_board: MetaBoard,
    pub subject: BigInt,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct MetaBoard {
    pub address: Bytes,
}

#[derive(cynic::Scalar, Debug, Clone)]
pub struct BigInt(pub String);

#[derive(cynic::Scalar, Debug, Clone)]
pub struct Bytes(pub String);
