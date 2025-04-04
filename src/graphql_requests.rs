pub mod github {
    use std::{error::Error, sync::mpsc};

    use graphql_client::{GraphQLQuery, Response};
    use reqwest::header;

    use crate::ui::tab_menu::RepoData;

    const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

    pub struct VariableStore {
        repo_name: String,
        repo_owner: String,
    }

    impl VariableStore {
        pub fn new(repo_name: String, repo_owner: String) -> Self {
            Self {
                repo_name,
                repo_owner,
            }
        }
    }

    impl Into<issues_query::Variables> for VariableStore {
        fn into(self) -> issues_query::Variables {
            issues_query::Variables {
                repo_name: self.repo_name,
                repo_owner: self.repo_owner,
            }
        }
    }

    impl Into<pull_requests_query::Variables> for VariableStore {
        fn into(self) -> pull_requests_query::Variables {
            pull_requests_query::Variables {
                repo_name: self.repo_name,
                repo_owner: self.repo_owner,
            }
        }
    }

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
    pub struct IssuesQuery;

    pub async fn perform_issues_query(
        response_sender: mpsc::Sender<RepoData>,
        variables: issues_query::Variables,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let request_body = IssuesQuery::build_query(variables);

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
        let response_body: Response<issues_query::ResponseData> = serde_json::from_str(&text)?; //response.json().await?;

        match response_body.data {
            Some(data) => {
                // very weird syntax to be honest I would expect Ok(Ok(())) to be returned here but
                // it doesn't seem so
                Ok(response_sender.send(RepoData::IssuesData(data))?)
            }
            None => Err("No response data returned.".into()),
        }
    }

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct PullRequestsQuery;

    pub async fn perform_pull_requests_query(
        response_sender: mpsc::Sender<RepoData>,
        variables: pull_requests_query::Variables,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let request_body = PullRequestsQuery::build_query(variables);

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
        let response_body: Response<pull_requests_query::ResponseData> =
            serde_json::from_str(&text)?; //response.json().await?;

        match response_body.data {
            Some(data) => {
                // very weird syntax to be honest I would expect Ok(Ok(())) to be returned here but
                // it doesn't seem so
                Ok(response_sender.send(RepoData::PullRequestsData(data))?)
            }
            None => Err("No response data returned.".into()),
        }
    }
}
