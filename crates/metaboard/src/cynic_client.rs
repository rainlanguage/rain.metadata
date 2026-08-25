use async_trait::async_trait;
use cynic::{
    serde::{Deserialize, Serialize},
    GraphQlError, GraphQlResponse, QueryBuilder, QueryFragment,
};
use reqwest::Url;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CynicClientError {
    #[error("Graphql errors: {}", .0.iter().map(|e| e.message.clone()).collect::<Vec<String>>().join(", "))]
    GraphqlError(Vec<GraphQlError>),
    #[error("Subgraph query returned no data")]
    Empty,
    #[error("Request Error: {0}")]
    Request(#[from] reqwest::Error),
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait CynicClient {
    fn get_base_url(&self) -> Url;

    async fn query<
        R: QueryFragment + QueryBuilder<V> + for<'a> Deserialize<'a>,
        V: Serialize + Send,
    >(
        &self,
        variables: V,
    ) -> Result<R, CynicClientError> {
        let request_body = R::build(variables);

        let response = reqwest::Client::new()
            .post(self.get_base_url())
            .json(&request_body)
            .send()
            .await?;

        let response_deserialized: GraphQlResponse<R> =
            response.json::<GraphQlResponse<R>>().await?;

        // An empty errors array satisfies cynic's "either data or errors must
        // be present" deserializer while carrying neither an error nor data,
        // so it is the only response shape that can reach Empty.
        match (response_deserialized.data, response_deserialized.errors) {
            (_, Some(errors)) if !errors.is_empty() => Err(CynicClientError::GraphqlError(errors)),
            (Some(data), _) => Ok(data),
            (None, _) => Err(CynicClientError::Empty),
        }
    }
}
