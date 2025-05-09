pub mod github {
    use std::{error::Error, sync::mpsc};

    use graphql_client::{GraphQLQuery, Response};
    use reqwest::header;

    use crate::ui::{
        list_view::{ListCollection, ListItem},
        tab_menu::RepoData,
    };

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

    impl Into<projects_query::Variables> for VariableStore {
        fn into(self) -> projects_query::Variables {
            projects_query::Variables {
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
        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

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

        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => {
                // very weird syntax to be honest I would expect Ok(Ok(())) to be returned here but
                // it doesn't seem so
                Ok(response_sender.send(RepoData::PullRequestsData(data))?)
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
    pub struct ProjectsQuery;

    pub async fn perform_projects_query(
        response_sender: mpsc::Sender<RepoData>,
        variables: projects_query::Variables,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let request_body = ProjectsQuery::build_query(variables);

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
        let response_body: Response<projects_query::ResponseData> = serde_json::from_str(&text)?; //response.json().await?;

        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => {
                // very weird syntax to be honest I would expect Ok(Ok(())) to be returned here but
                // it doesn't seem so
                Ok(response_sender.send(RepoData::ProjectsData(data))?)
            }
            None => Err("No response data returned.".into()),
        }
    }

    impl ListItem for issues_query::IssuesQueryRepositoryIssuesNodes {
        fn get_title(&self) -> &str {
            &self.title
        }

        fn get_number(&self) -> i64 {
            self.number
        }

        fn is_closed(&self) -> bool {
            self.closed
        }

        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }

        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        fn get_labels(&self) -> Vec<String> {
            let mut result = Vec::new();
            if let Some(labels) = &self.labels {
                if let Some(nodes) = &labels.nodes {
                    for node in nodes {
                        if let Some(label) = node {
                            result.push(label.name.clone());
                        }
                    }
                }
            }
            result
        }
    }

    #[derive(Debug)]
    pub struct IssuesCollection {
        repository: issues_query::IssuesQueryRepository,
    }

    impl IssuesCollection {
        pub fn new(repository: issues_query::IssuesQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl ListCollection for IssuesCollection {
        fn get_items(&self) -> Vec<Box<dyn ListItem>> {
            let mut items: Vec<Box<dyn ListItem>> = Vec::new();
            if let Some(nodes) = &self.repository.issues.nodes {
                for node in nodes {
                    if let Some(issue) = node {
                        items.push(Box::new(issue.clone()));
                    }
                }
            }
            items
        }

        fn from_repository_data(
            data: Box<dyn std::any::Any>,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            match data.downcast::<issues_query::IssuesQueryRepository>() {
                Ok(issues_repo) => Ok(Self::new(*issues_repo)),
                Err(other) => Err(
                    format!("Couldn't downcast to pull_requests_query::PullRequestsQueryRepository. Other value was: {:?}", other.type_id()).into(),
                ),
            }
        }
    }

    impl ListItem for pull_requests_query::PullRequestsQueryRepositoryPullRequestsNodes {
        fn get_title(&self) -> &str {
            &self.title
        }

        fn get_number(&self) -> i64 {
            self.number
        }

        fn is_closed(&self) -> bool {
            self.closed
        }

        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }

        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        fn get_labels(&self) -> Vec<String> {
            let mut result = Vec::new();
            if let Some(labels) = &self.labels {
                if let Some(nodes) = &labels.nodes {
                    for node in nodes {
                        if let Some(label) = node {
                            result.push(label.name.clone());
                        }
                    }
                }
            }
            result
        }
    }

    #[derive(Debug)]
    pub struct PullRequestsCollection {
        repository: pull_requests_query::PullRequestsQueryRepository,
    }

    impl PullRequestsCollection {
        pub fn new(repository: pull_requests_query::PullRequestsQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl ListCollection for PullRequestsCollection {
        fn get_items(&self) -> Vec<Box<dyn ListItem>> {
            let mut items: Vec<Box<dyn ListItem>> = Vec::new();
            if let Some(nodes) = &self.repository.pull_requests.nodes {
                for node in nodes {
                    if let Some(pr) = node {
                        items.push(Box::new(pr.clone()));
                    }
                }
            }
            items
        }

        fn from_repository_data(
            data: Box<dyn std::any::Any>,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            match data.downcast::<pull_requests_query::PullRequestsQueryRepository>() {
                Ok(issues_repo) => Ok(Self::new(*issues_repo)),
                Err(other) => Err(
                    format!("Couldn't downcast to pull_requests_query::PullRequestsQueryRepository. Other value was: {:?}", other.type_id()).into(),
                ),
            }
        }
    }

    impl ListItem for projects_query::ProjectsQueryRepositoryProjectsV2Nodes {
        fn get_title(&self) -> &str {
            &self.title
        }

        fn get_number(&self) -> i64 {
            self.number
        }

        fn is_closed(&self) -> bool {
            self.closed
        }

        fn get_author_login(&self) -> Option<&str> {
            self.creator.as_ref().map(|author| &author.login[..])
        }

        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        fn get_labels(&self) -> Vec<String> {
            vec![]
        }
    }

    #[derive(Debug)]
    pub struct ProjectsCollection {
        repository: projects_query::ProjectsQueryRepository,
    }

    impl ProjectsCollection {
        pub fn new(repository: projects_query::ProjectsQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl ListCollection for ProjectsCollection {
        fn get_items(&self) -> Vec<Box<dyn ListItem>> {
            let mut items: Vec<Box<dyn ListItem>> = Vec::new();
            if let Some(nodes) = &self.repository.projects_v2.nodes {
                for node in nodes {
                    if let Some(issue) = node {
                        items.push(Box::new(issue.clone()));
                    }
                }
            }
            items
        }

        fn from_repository_data(
            data: Box<dyn std::any::Any>,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            match data.downcast::<projects_query::ProjectsQueryRepository>() {
                Ok(projects_repo) => {
                    log::debug!("{:?}", projects_repo);
                    Ok(Self::new(*projects_repo))
                },
                Err(other) => Err(
                    format!("Couldn't downcast to projects_query::ProjectsQueryRepository. Other value was: {:?}", other.type_id()).into(),
                ),
            }
        }
    }
}
