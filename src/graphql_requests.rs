/// implements ListCollection for a type <T>, for accessing each individual, issue, pull request or
/// project
macro_rules! impl_ListCollection_for_T {
    ($T:ty, $item_identifier:ident, $repo_data:ident, $detail_func:ident) => {
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
            fn from_repository_data(data: RepoData) -> Result<Self, Box<dyn std::error::Error>> {
                match data {
                    RepoData::$repo_data(response_data) => match response_data.repository {
                        Some(repo) => Ok(Self::new(repo)),
                        None => Err("There was no repository data to display".into()),
                    },
                    other => Err(format!(
                        "Received data wasn't of type RepoData::{:?}. Other value was: {other:?}",
                        stringify!($repo_data),
                    )
                    .into()),
                }
            }

            fn get_detail_func() -> ItemDetailFunc {
                $detail_func
            }
        }
    };
}

pub mod github {
    use regex::Regex;
    use types::DateTime;

    use std::{error::Error, future::Future, pin::Pin, sync::mpsc};

    use graphql_client::{GraphQLQuery, Response};
    use reqwest::header;

    use crate::ui::{
        detail_view::{Comment, DetailItem, DetailListItem},
        list_view::{ListCollection, ListItem},
        ItemDetailFunc, RepoData,
    };

    const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

    /// `VariablesStore` stores all relevant variables for a graphql query
    #[derive(Default)]
    pub struct VariableStore {
        pub repo_name: String,
        pub repo_owner: String,
        pub issue_number: i64,
    }

    impl VariableStore {
        pub fn default_with_repo_info(active_remote: &str) -> Option<Self> {
            let repo_regex = match Regex::new(":(?<owner>.*)/(?<name>.*).git$") {
                Ok(reg) => reg,
                Err(error) => {
                    log::debug!("Couldn't create regex because of error: {error}");
                    return None;
                }
            };

            let repo_captures = repo_regex.captures(active_remote)?;

            Some(
                Self::default()
                    .repo_name(repo_captures["name"].to_string())
                    .repo_owner(repo_captures["owner"].to_string()),
            )
        }

        pub fn repo_name(mut self, repo_name: String) -> Self {
            self.repo_name = repo_name;
            self
        }

        pub fn repo_owner(mut self, repo_owner: String) -> Self {
            self.repo_owner = repo_owner;
            self
        }

        pub fn issue_number(mut self, issue_number: i64) -> Self {
            self.issue_number = issue_number;
            self
        }
    }

    // generic type declaration for graphql requests so that graphql_client does know what type to
    // downcast how and to what
    pub mod types {
        use chrono::Utc;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct User(pub String);

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
        pub struct DateTime(chrono::DateTime<Utc>);

        impl DateTime {
            pub fn to_str(&self, fmt: &str) -> String {
                self.0.format(fmt).to_string()
            }
        }
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
        variable_store: VariableStore,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let variables = issues_query::Variables {
            repo_name: variable_store.repo_name,
            repo_owner: variable_store.repo_owner,
        };
        let request_body = IssuesQuery::build_query(variables);

        let client = reqwest::Client::builder()
            .user_agent("LazyIssues/0.1.0")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {access_token}"))?,
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
            Some(data) => Ok(response_sender.send(RepoData::Issues(data))?),
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
        variable_store: VariableStore,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let variables = pull_requests_query::Variables {
            repo_name: variable_store.repo_name,
            repo_owner: variable_store.repo_owner,
        };
        let request_body = PullRequestsQuery::build_query(variables);

        let client = reqwest::Client::builder()
            .user_agent("LazyIssues/0.1.0")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {access_token}"))?,
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
            Some(data) => Ok(response_sender.send(RepoData::PullRequests(data))?),
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
        variable_store: VariableStore,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let variables = projects_query::Variables {
            repo_name: variable_store.repo_name,
            repo_owner: variable_store.repo_owner,
        };
        let request_body = ProjectsQuery::build_query(variables);

        let client = reqwest::Client::builder()
            .user_agent("LazyIssues/0.1.0")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {access_token}"))?,
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
            Some(data) => Ok(response_sender.send(RepoData::Projects(data))?),
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
        fn get_created_at(&self) -> &DateTime {
            &self.created_at
        }

        /// gets all labels of an issue
        fn get_labels(&self) -> Vec<String> {
            let mut result = Vec::new();
            let Some(labels) = &self.labels else {
                return result;
            };

            let Some(nodes) = &labels.nodes else {
                return result;
            };

            for label in nodes.iter().flatten() {
                result.push(label.name.clone());
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
        Issues,
        perform_detail_issue_query_wrapper
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
        fn get_created_at(&self) -> &DateTime {
            &self.created_at
        }

        /// gets all asigned labels for that pull request
        fn get_labels(&self) -> Vec<String> {
            let mut result = Vec::new();
            let Some(labels) = &self.labels else {
                return result;
            };

            let Some(nodes) = &labels.nodes else {
                return result;
            };

            for label in nodes.iter().flatten() {
                result.push(label.name.clone());
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
        PullRequests,
        perform_detail_issue_query_wrapper
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
        fn get_created_at(&self) -> &DateTime {
            &self.created_at
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
        Projects,
        perform_detail_issue_query_wrapper
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
        variable_store: VariableStore,
        access_token: String,
    ) -> Result<(), Box<dyn Error>> {
        let variables = issue_detail_query::Variables {
            repo_name: variable_store.repo_name,
            repo_owner: variable_store.repo_owner,
            issue_number: variable_store.issue_number,
        };
        let request_body = IssueDetailQuery::build_query(variables);

        let client = reqwest::Client::builder()
            .user_agent("LazyIssues/0.1.0")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(
                    header::AUTHORIZATION,
                    header::HeaderValue::from_str(&format!("Bearer {access_token}"))?,
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
            Some(data) => match data.repository {
                Some(repo) => match repo.issue {
                    Some(issue) => {
                        Ok(response_sender.send(RepoData::ItemDetails(Box::new(issue)))?)
                    }
                    None => Err("No issue in repository returned".into()),
                },
                None => Err("No repository returned for request".into()),
            },
            None => Err("No response data returned.".into()),
        }
    }

    // this is kinda shit ngl
    type RequestReturnType = Result<(), Box<dyn Error>>;

    pub fn perform_detail_issue_query_wrapper(
        response_sender: mpsc::Sender<RepoData>,
        variable_store: VariableStore,
        access_token: String,
    ) -> Pin<Box<dyn Future<Output = RequestReturnType> + Send>> {
        Box::pin(perform_detail_issue_query(
            response_sender,
            variable_store,
            access_token,
        ))
    }

    impl ListItem for issue_detail_query::IssueDetailQueryRepositoryIssue {
        fn get_title(&self) -> &str {
            &self.title
        }

        fn is_closed(&self) -> bool {
            self.closed
        }

        fn get_number(&self) -> i64 {
            self.number
        }

        fn get_labels(&self) -> Vec<String> {
            let mut result = Vec::new();

            let Some(labels) = &self.labels else {
                return result;
            };

            let Some(nodes) = &labels.nodes else {
                return result;
            };

            for label in nodes.iter().flatten() {
                result.push(label.name.clone());
            }

            result
        }

        fn get_created_at(&self) -> &DateTime {
            &self.created_at
        }

        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }
    }

    impl DetailItem for issue_detail_query::IssueDetailQueryRepositoryIssue {
        fn get_num_comments(&self) -> usize {
            self.comments
                .edges
                .iter()
                .flatten()
                .flatten()
                .flat_map(|edge| &edge.node)
                .count()
        }

        fn get_comments(&self) -> Vec<&dyn Comment> {
            let comments: Vec<_> = self
                .comments
                .edges
                .iter()
                .flatten()
                .flatten()
                .flat_map(|edge| &edge.node)
                .map(|node| node as &dyn Comment)
                .collect();
            comments
        }
    }

    impl Comment for issue_detail_query::IssueDetailQueryRepositoryIssue {
        fn get_body(&self) -> &str {
            &self.body
        }

        fn get_created_at(&self) -> &DateTime {
            &self.created_at
        }

        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }
    }

    impl DetailListItem for issue_detail_query::IssueDetailQueryRepositoryIssue {}

    impl Comment for issue_detail_query::IssueDetailQueryRepositoryIssueCommentsEdgesNode {
        fn get_author_login(&self) -> Option<&str> {
            self.author.as_ref().map(|author| &author.login[..])
        }

        fn get_created_at(&self) -> &DateTime {
            &self.created_at
        }

        fn get_body(&self) -> &str {
            &self.body
        }
    }
}
