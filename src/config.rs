use dirs::config_local_dir;
use kdl::{KdlDocument, KdlNode, KdlNodeFormat};
use keyring::Entry;
use ratatui::style::Color;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Error as IoError, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output};
use std::str::FromStr;
use std::time::Duration;
use std::{env, fs};

pub mod git;

/// gets a kdl nodes value as a string or emits an error passing through code using the ?-operator
macro_rules! get_kdl_string_value_or_error {
    ($node:expr) => {
        $node
            .as_string()
            .ok_or_else(|| {
                IoError::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{:?} does not have a value of type string", $node),
                )
            })?
            .to_string()
    };
}

/// gets the first entry of a node as a string
/// :return Option<String>
macro_rules! get_first_entry_as_string {
    ($node:expr) => {
        $node
            .entries()
            .first()
            .map_or(None, |entry| entry.value().as_string())
    };
}

/// gets all entries of a node as a std::vec<String>
macro_rules! get_entries_as_string_vec {
    ($node:expr) => {
        $node
            .entries()
            .iter()
            .filter_map(|node| node.value().as_string())
            .collect()
    };
}

/// gets the first entry of a node as a PathBuf
/// :return Option<PathBuf>
macro_rules! get_first_entry_as_PathBuf {
    ($node:expr) => {
        $node.entries().first().map_or(None, |entry| {
            entry
                .value()
                .as_string()
                .map_or(None, |str_value| Some(PathBuf::from(str_value)))
        })
    };
}

/// gets the first entry of a node as an int
/// :return Option<int>
macro_rules! get_first_entry_as_int {
    ($node:expr) => {
        $node
            .entries()
            .first()
            .map_or(None, |entry| entry.value().as_integer())
    };
}

/// reads the token file of a specific backend(github, gitlab, gitea)
/// :return Result<String, IoError>
macro_rules! read_token_file_backend {
    ($backend:expr) => {
        if let Some(path) = $backend {
            Ok(fs::read_to_string(path)?.trim().to_string())
        } else {
            Err(IoError::new(
                std::io::ErrorKind::NotFound,
                format!("No path for {:?} token file set", $backend),
            ))
        }
    };
}

/// unwraps an error value, logging on error and returning Some on Ok
/// :return Option<type <T>>
macro_rules! report_error_to_log {
    ($expr:expr) => {
        match $expr {
            Ok(value) => Some(value),
            Err(error) => {
                log::error!("{}", error);
                None
            }
        }
    };
}

pub const CONFIG_NAME: &str = "config.kdl";
pub const CONFIG_DIR_NAME: &str = "lazyissues";

pub const STATE_NAME: &str = "state.kdl";

/// gets the lazyissues config filepath
pub fn get_config_file() -> PathBuf {
    config_local_dir()
        .unwrap_or(PathBuf::default())
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_NAME)
        .to_owned()
}

/// gets the lazyissues state filepath
pub fn get_state_file() -> PathBuf {
    config_local_dir()
        .unwrap_or(PathBuf::default())
        .join(CONFIG_DIR_NAME)
        .join(STATE_NAME)
        .to_owned()
}

/// `Config` struct for storing user set config for lazyissues
#[derive(Debug, Clone)]
pub struct Config {
    pub github_token: Option<String>,
    github_token_path: Option<PathBuf>,
    pub gitlab_token: Option<String>,
    gitlab_token_path: Option<PathBuf>,
    pub gitea_token: Option<String>,
    gitea_token_path: Option<PathBuf>,

    tag_styles: HashMap<String, Color>,

    credential_attempts: u64,
    credential_timeout: u64,
}

impl Default for Config {
    /// creates a new instance of `Config` using default values
    fn default() -> Self {
        Self {
            github_token: None,
            github_token_path: None,
            gitlab_token: None,
            gitlab_token_path: None,
            gitea_token: None,
            gitea_token_path: None,

            tag_styles: HashMap::from([
                ("bug".to_string(), Color::Red),
                ("documentation".to_string(), Color::Blue),
                ("duplicate".to_string(), Color::Gray),
                ("enhancement".to_string(), Color::LightCyan),
                ("good first issue".to_string(), Color::LightMagenta),
                ("help wanted".to_string(), Color::Green),
                ("invalid".to_string(), Color::Yellow),
                ("question".to_string(), Color::Magenta),
                ("wontfix".to_string(), Color::White),
            ]),

            credential_attempts: 4,
            credential_timeout: 50,
        }
    }
}

impl Config {
    /// reads in config file creating config based on this file
    pub fn from_config_file() -> Result<Self, IoError> {
        let kdl_str = fs::read_to_string(get_config_file())?;
        Self::from_kdl_str(&kdl_str)
    }

    /// creates config based on a KdlDocument parsed to a string
    fn from_kdl_str(kdl_str: &str) -> Result<Self, IoError> {
        let kdl_config = KdlDocument::parse(kdl_str).map_err(|e| {
            IoError::new(
                std::io::ErrorKind::InvalidData,
                format!("KDL parse error: {}", e),
            )
        })?;

        let mut config = Self::default();

        for node in kdl_config.nodes() {
            match config.apply_option(&kdl_config, node.name().value()) {
                Ok(_) => (),
                Err(error) => log::error!("{} occured while parsing config", error),
            }
        }

        match config.set_access_tokens() {
            Ok(_) => (),
            Err(error) => log::error!("{} occured during setting of access tokens", error),
        }

        Ok(config)
    }

    /// applies an option to the config modifying it inplace
    fn apply_option(
        &mut self,
        parent_node: &KdlDocument,
        option_name: &str,
    ) -> Result<(), IoError> {
        let option_node = parent_node.get(option_name);

        match option_node {
            Some(node) => match option_name {
                "github_token_path" => {
                    self.github_token_path = get_first_entry_as_PathBuf!(node);
                }
                "gitlab_token_path" => {
                    self.gitlab_token_path = get_first_entry_as_PathBuf!(node);
                }
                "gitea_token_path" => {
                    self.gitea_token_path = get_first_entry_as_PathBuf!(node);
                }
                "credentials_attempts" => {
                    self.credential_attempts = get_first_entry_as_int!(node)
                        .unwrap_or(4)
                        .try_into()
                        .unwrap_or(4);
                }
                "credentials_timeout" => {
                    self.credential_timeout = get_first_entry_as_int!(node)
                        .unwrap_or(50)
                        .try_into()
                        .unwrap_or(50);
                }
                "tags" => {
                    self.read_tag_node(node);
                }
                _ => {
                    log::info!("Option: {} is not a recognized option", option_name);
                }
            },
            _ => {
                log::debug!(
                    "Option: {} is not a child of node: {:?}",
                    option_name,
                    parent_node
                );
            }
        }
        Ok(())
    }

    /// reads a tag node determining the associated color and it's name
    fn read_tag_node(&mut self, tag_node: &KdlNode) {
        for child in tag_node.iter_children() {
            self.tag_styles.insert(
                child.name().value().to_string(),
                match Color::from_str(get_first_entry_as_string!(child).unwrap_or("white")) {
                    Ok(color) => color,
                    Err(error) => {
                        log::error!(
                            "While parsing custom tag node: {} got error {}",
                            child.name().value(),
                            error
                        );
                        Color::White
                    }
                },
            );
        }
    }

    /// returns a color for a given tag. If the tag is not found return White
    pub fn get_tag_color(&self, tag: &str) -> Color {
        self.tag_styles.get(tag).copied().unwrap_or(Color::White)
    }

    /// sets the access tokens for the different backends
    fn set_access_tokens(&mut self) -> Result<(), IoError> {
        self.github_token = report_error_to_log!(self.get_access_token("github"));
        self.gitlab_token = report_error_to_log!(self.get_access_token("gitlab"));
        self.gitea_token = report_error_to_log!(self.get_access_token("gitea"));
        Ok(())
    }

    /// tries to parse access tokens for the git backends so that we can use this to authenticate
    /// with the git backend in our request
    fn get_access_token(&self, token_type: &str) -> Result<String, IoError> {
        if let Ok(token) = env::var(format!("{}_TOKEN", token_type.to_uppercase())) {
            return Ok(token);
        }

        if let Ok(entry) = Entry::new("lazyissues", token_type) {
            if let Ok(token) = entry.get_password() {
                return Ok(token);
            }
        }

        match self.get_git_credential() {
            Ok(token) => return Ok(token),
            Err(error) => log::info!("{}", error),
        }

        match token_type {
            "github" => read_token_file_backend!(&self.github_token_path),
            "gitlab" => read_token_file_backend!(&self.gitlab_token_path),
            "gitea" => read_token_file_backend!(&self.gitea_token_path),
            _ => Err(IoError::new(
                std::io::ErrorKind::NotFound,
                format!("No token for {} found", token_type),
            )),
        }
    }

    /// tries to get git credentials stored in git locally
    fn get_git_credential(&self) -> Result<String, Box<dyn Error>> {
        let mut child = Command::new("git").args(["credential", "fill"]).spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            let active_remote = git::get_active_remote()?;
            stdin.write(&format!("protocol=https\nhost={}\n\n", active_remote).into_bytes())?;
        }

        let output = self.wait_for_credential_output(child)?;

        let output_str = String::from_utf8(output.stdout)?;

        for line in output_str.lines() {
            if line.starts_with("password=") {
                return Ok(line.replace("password=", ""));
            }
        }

        return Err("No GitHub token found in git credentials".into());
    }

    /// waits for credential output trying `self.credential_attempts` times
    fn wait_for_credential_output(&self, mut child: Child) -> Result<Output, Box<dyn Error>> {
        let mut exit_status = child.try_wait();
        for _ in 0..self.credential_attempts {
            match exit_status {
                Ok(Some(_)) => {
                    return Ok(child.wait_with_output()?);
                }
                Err(_) | Ok(None) => {
                    std::thread::sleep(Duration::from_millis(self.credential_timeout));
                    exit_status = child.try_wait();
                }
            }
        }

        child.kill()?;
        Err("Credentialhelper timed out".into())
    }
}

/// `State` struct storing application state like currently prefered repository endpoint for a
/// specific repository
pub struct State {
    //               <local repo path, active remote>
    repository_state: HashMap<PathBuf, String>,
}

impl Default for State {
    /// creates a new instance of `State` using default values
    fn default() -> Self {
        Self {
            repository_state: HashMap::new(),
        }
    }
}

impl State {
    /// reads in the current state of lazyissues
    pub fn read() -> std::io::Result<Self> {
        let kdl_str = fs::read_to_string(get_state_file())?;
        Self::from_kdl_str(&kdl_str)
    }

    /// creates a new `State` object by reading a KdlDocument's parsed string
    fn from_kdl_str(kdl_str: &str) -> std::io::Result<Self> {
        let kdl_state = KdlDocument::parse(kdl_str).map_err(|e| {
            IoError::new(
                std::io::ErrorKind::InvalidData,
                format!("KDL parse error: {}", e),
            )
        })?;

        let mut state = Self::default();

        for node in kdl_state.nodes() {
            match state.apply_option(&kdl_state, node.name().value()) {
                Err(error) => log::error!("{} occured while parsing config", error),
                _ => (),
            }
        }

        Ok(state)
    }

    /// applies an option found in the state file to itself. Modifying itself inplace
    fn apply_option(
        &mut self,
        parent_node: &KdlDocument,
        option_name: &str,
    ) -> Result<(), IoError> {
        let option_node = parent_node.get(option_name);

        match option_node {
            Some(node) => match option_name {
                "repositories" => self.read_repositories(node),
                _ => {
                    log::debug!("Option: {} is not a recognized option", option_name);
                }
            },
            _ => {
                log::debug!(
                    "Option: {} is not a child of node: {:?}",
                    option_name,
                    parent_node
                );
            }
        }
        Ok(())
    }

    /// reads repositories found in state file
    fn read_repositories(&mut self, repositories_node: &KdlNode) {
        for child in repositories_node.iter_children() {
            match child.name().value() {
                "repo" => {
                    let entries: Vec<&str> = get_entries_as_string_vec!(child);
                    if entries.len() < 2 {
                        log::warn!("repo tag is malformed. Missing either local repo path, active remote or both: {:?}", child);
                        continue;
                    }

                    let repo_path = PathBuf::from(entries[0]);
                    let active_remote = entries[1];

                    self.repository_state
                        .insert(repo_path, active_remote.to_string());
                }
                _ => (),
            }
        }
    }

    /// returns the saved repository data for a given repository root
    pub fn get_repository_data(&self, repo_root: &PathBuf) -> Option<String> {
        self.repository_state.get(repo_root).cloned()
    }

    /// sets repository state for a repository
    pub fn set_repository_data(
        &mut self,
        repo_root: PathBuf,
        active_remote: String,
    ) -> std::io::Result<()> {
        self.repository_state.insert(repo_root, active_remote);
        self.write_to_kdl()?;

        Ok(())
    }

    /// writes the State set for a repository into the state file
    fn write_to_kdl(&self) -> std::io::Result<()> {
        let mut kdl_state = KdlDocument::new();

        let mut repositories_node = KdlNode::new("repositories");
        let mut repositories_node_fmt = KdlNodeFormat::default();
        repositories_node_fmt.trailing = "\n".to_string();
        repositories_node_fmt.before_children = " ".to_string();
        repositories_node.set_format(repositories_node_fmt);

        let mut repositories_children = KdlDocument::new();

        for (local_path, remote) in self.repository_state.iter() {
            let mut repo_node = KdlNode::new("repo");
            let mut node_fmt = KdlNodeFormat::default();
            node_fmt.leading = "    ".to_string();
            node_fmt.trailing = "\n".to_string();
            repo_node.set_format(node_fmt);

            repo_node.push(local_path.to_str().unwrap_or(""));
            repo_node.push(remote.clone());

            repositories_children.nodes_mut().push(repo_node);
        }

        repositories_node.set_children(repositories_children);
        kdl_state.nodes_mut().push(repositories_node);

        fs::write(get_state_file(), kdl_state.to_string())?;

        Ok(())
    }
}
