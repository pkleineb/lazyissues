pub mod github {
    use std::error::Error;

    use graphql_client::{GraphQLQuery, Response};
    use reqwest::header;

    const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

    pub mod types {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct User(pub String);

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct DateTime(pub String);
    }

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct IssueQuery;

    pub async fn perform_issue_query(
        variables: issue_query::Variables,
        access_token: String,
    ) -> Result<Option<issue_query::ResponseData>, Box<dyn Error>> {
        let request_body = IssueQuery::build_query(variables);

        let client = reqwest::Client::builder()
            .user_agent("LazyIssues/0.1.0")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {}", access_token))?,
                );
                headers
            })
            .build()?;

        let response = client
            .post(GITHUB_GRAPHQL_ENDPOINT)
            .json(&request_body)
            .send()
            .await?;

        let text = response.text().await?;
        log::debug!("Server response was: {:?}", text);
        let response_body: Response<issue_query::ResponseData> = serde_json::from_str(&text)?; //response.json().await?;
        Ok(response_body.data)
    }
}
