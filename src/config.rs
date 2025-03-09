use dirs::{config_local_dir, home_dir};
use kdl::{KdlDocument, KdlNode};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::io::{Error as IoError, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;
use std::{env, fs};

mod git;

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

macro_rules! get_first_entry_as_string {
    ($node:expr) => {
        $node
            .entries()
            .first()
            .map_or(None, |entry| entry.value().as_string())
    };
}

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

macro_rules! get_first_entry_as_int {
    ($node:expr) => {
        $node
            .entries()
            .first()
            .map_or(None, |entry| entry.value().as_integer())
    };
}

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

pub fn get_config_file() -> PathBuf {
    config_local_dir()
        .unwrap_or(PathBuf::default())
        .join(CONFIG_DIR_NAME)
        .join(CONFIG_NAME)
        .to_owned()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub github_token: Option<String>,
    github_token_path: Option<PathBuf>,
    pub gitlab_token: Option<String>,
    gitlab_token_path: Option<PathBuf>,
    pub gitea_token: Option<String>,
    gitea_token_path: Option<PathBuf>,

    credential_attempts: u64,
    credential_timeout: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            github_token: None,
            github_token_path: None,
            gitlab_token: None,
            gitlab_token_path: None,
            gitea_token: None,
            gitea_token_path: None,

            credential_attempts: 4,
            credential_timeout: 50,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_config_file() -> Result<Self, IoError> {
        let kdl_str = fs::read_to_string(get_config_file())?;
        Self::from_kdl_str(&kdl_str)
    }

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

        log::debug!("{:?}", config);
        Ok(config)
    }

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

    fn set_access_tokens(&mut self) -> Result<(), IoError> {
        self.github_token = report_error_to_log!(self.get_access_token("github"));
        self.gitlab_token = report_error_to_log!(self.get_access_token("gitlab"));
        self.gitea_token = report_error_to_log!(self.get_access_token("gitea"));
        Ok(())
    }

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
