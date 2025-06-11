/// macro for implementing into type <T> for a the VariableStore struct, for passing Variables to
/// the graphql queries
macro_rules! impl_Into_T_for_VariableStore {
    ($module:ident) => {
        impl Into<$module::Variables> for VariableStore {
            fn into(self) -> $module::Variables {
                $module::Variables {
                    repo_name: self.repo_name,
                    repo_owner: self.repo_owner,
                }
            }
        }
    };
}

/// implements ListCollection for a type <T>, for accessing each individual, issue, pull request or
/// project
macro_rules! impl_ListCollection_for_T {
    ($T:ty, $item_identifier:ident, $module:ident, $type:ident) => {
        impl ListCollection for $T {
            /// returns all items(issues, pull requests or projects) that are in the
            /// `ListCollection`
            fn get_items(&self) -> Vec<Box<dyn ListItem>> {
                let mut items: Vec<Box<dyn ListItem>> = Vec::new();
                if let Some(nodes) = &self.repository.$item_identifier.nodes {
                    for node in nodes {
                        if let Some(item) = node {
                            items.push(Box::new(item.clone()));
                        }
                    }
                }
                items
            }

            /// tries to downcast some data into the correct Type <T> this is implemented for to
            /// build a new `ListCollection`
            fn from_repository_data(
                data: Box<dyn std::any::Any>,
            ) -> Result<Self, Box<dyn std::error::Error>> {
                match data.downcast::<$module::$type>() {
                    Ok(repo) => Ok(Self::new(*repo)),
                    Err(other) => Err(format!(
                        "Couldn't downcast to {:?}. Other value was: {:?}",
                        std::any::type_name::<$module::$type>(),
                        other.type_id()
                    )
                    .into()),
                }
            }
        }
    };
}

pub mod github {
    use std::{error::Error, sync::mpsc};

    use graphql_client::{GraphQLQuery, Response};
    use reqwest::header;

    use crate::ui::{
        list_view::{ListCollection, ListItem},
        tab_menu::RepoData,
    };

    const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

    /// `VariablesStore` stores all relevant variables for a graphql query
    pub struct VariableStore {
        repo_name: String,
        repo_owner: String,
    }

    impl VariableStore {
        /// creates a new variables store
        pub fn new(repo_name: String, repo_owner: String) -> Self {
            Self {
                repo_name,
                repo_owner,
            }
        }
    }

    impl_Into_T_for_VariableStore!(issues_query);
    impl_Into_T_for_VariableStore!(pull_requests_query);
    impl_Into_T_for_VariableStore!(projects_query);

    // generic type declaration for graphql requests so that graphql_client does know what type to
    // downcast how and to what
    pub mod types {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct User(pub String);

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct DateTime(pub String);
    }

    /// `IssuesQuery` represents the github issues query for quering all (first 100) issues in a
    /// github repository
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct IssuesQuery;

    /// performs the issue query sending it to the server
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
        let response_body: Response<issues_query::ResponseData> = serde_json::from_str(&text)?;
        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => Ok(response_sender.send(RepoData::IssuesData(data))?),
            None => Err("No response data returned.".into()),
        }
    }

    /// `PullRequestsQuery` represents the github pull requests query for quering all (first 100)
    /// pull requests on a github repository
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct PullRequestsQuery;

    /// performs the pull request query sending it to the server
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
            serde_json::from_str(&text)?;

        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => Ok(response_sender.send(RepoData::PullRequestsData(data))?),
            None => Err("No response data returned.".into()),
        }
    }

    /// `ProjectsQuery` represents the github projects query for viewing all (first 100) projects a
    /// user on a specific github repository has
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct ProjectsQuery;

    /// performs the projects query sending it to the server
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
        let response_body: Response<projects_query::ResponseData> = serde_json::from_str(&text)?;

        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => Ok(response_sender.send(RepoData::ProjectsData(data))?),
            None => Err("No response data returned.".into()),
        }
    }

    impl ListItem for issues_query::IssuesQueryRepositoryIssuesNodes {
        /// gets the title of an issue of a repository
        fn get_title(&self) -> &str {
            &self.title
        }

        /// gets the number of an issue of a repository
        fn get_number(&self) -> i64 {
            self.number
        }

        /// checks wether or not the issue is closed in a repository
        fn is_closed(&self) -> bool {
            self.closed
        }

        /// gets the login(username) of the author of that issue
        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }

        /// gets the timestamp when the issue got created
        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        /// gets all labels of an issue
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

    /// `IssuesCollection` represents all issues that the `IssuesQuery` returned
    #[derive(Debug)]
    pub struct IssuesCollection {
        repository: issues_query::IssuesQueryRepository,
    }

    impl IssuesCollection {
        /// creates a new instance of the `IssuesCollection`
        pub fn new(repository: issues_query::IssuesQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl_ListCollection_for_T!(
        IssuesCollection,
        issues,
        issues_query,
        IssuesQueryRepository
    );

    impl ListItem for pull_requests_query::PullRequestsQueryRepositoryPullRequestsNodes {
        /// gets the title of the pull request
        fn get_title(&self) -> &str {
            &self.title
        }

        /// gets the number of the pull request
        fn get_number(&self) -> i64 {
            self.number
        }

        /// checks wether or not the pull request has been closed
        fn is_closed(&self) -> bool {
            self.closed
        }

        /// gets the login(username) of the author for that pull request
        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }

        /// gets the timestamp when the pull request was created
        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        /// gets all asigned labels for that pull request
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

    /// `PullRequestsCollection` represents all pull requests that the `PullRequestsQuery` returned
    #[derive(Debug)]
    pub struct PullRequestsCollection {
        repository: pull_requests_query::PullRequestsQueryRepository,
    }

    impl PullRequestsCollection {
        /// creates a new instance of `PullrequestsCollection`
        pub fn new(repository: pull_requests_query::PullRequestsQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl_ListCollection_for_T!(
        PullRequestsCollection,
        pull_requests,
        pull_requests_query,
        PullRequestsQueryRepository
    );

    impl ListItem for projects_query::ProjectsQueryRepositoryProjectsV2Nodes {
        /// gets the title of the project
        fn get_title(&self) -> &str {
            &self.title
        }

        /// gets the number of the project
        fn get_number(&self) -> i64 {
            self.number
        }

        /// checks wether or not the project is closed
        fn is_closed(&self) -> bool {
            self.closed
        }

        /// gets the login(username) of the author of the project
        fn get_author_login(&self) -> Option<&str> {
            self.creator.as_ref().map(|author| &author.login[..])
        }

        /// gets the timestamp when the project got created
        fn get_created_at(&self) -> &str {
            &self.created_at.0
        }

        /// gets the labels of the project. Since projects don't have labels we return an empty
        /// vector
        fn get_labels(&self) -> Vec<String> {
            vec![]
        }
    }

    /// `ProjectsCollection` represents all projects that the `ProjectsQuery` returned
    #[derive(Debug)]
    pub struct ProjectsCollection {
        repository: projects_query::ProjectsQueryRepository,
    }

    impl ProjectsCollection {
        /// creast a new instance of `ProjectsCollection`
        pub fn new(repository: projects_query::ProjectsQueryRepository) -> Self {
            Self { repository }
        }
    }

    impl_ListCollection_for_T!(
        ProjectsCollection,
        projects_v2,
        projects_query,
        ProjectsQueryRepository
    );

    /// `IssueDetailQuery` represents the detailed query about an issue like comments
    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "src/graphql/schema.github.graphql",
        query_path = "src/graphql/queries.github.graphql",
        response_derives = "Debug, Clone, PartialEq",
        custom_scalars_module = "types"
    )]
    pub struct IssueDetailQuery;

    /// performs the `IssueDetailQuery` sending it to the server
    pub async fn perform_detail_issue_query(
        response_sender: mpsc::Sender<RepoData>,
        variables: issue_detail_query::Variables,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let request_body = IssueDetailQuery::build_query(variables);

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
        let response_body: Response<issue_detail_query::ResponseData> =
            serde_json::from_str(&text)?;
        if let Some(errors) = response_body.errors {
            log::debug!("Found errors in request: {:?}", errors);
        }

        match response_body.data {
            Some(data) => Ok(response_sender.send(RepoData::IssueInspectData(data))?),
            None => Err("No response data returned.".into()),
        }
    }
}
