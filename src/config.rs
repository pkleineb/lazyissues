use std::{fs, path::PathBuf};

use dirs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    is_default_config: bool,

    github_token_file_path: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            is_default_config: true,
            github_token_file_path: "".to_string(),
        }
    }

    pub fn is_default(&self) -> bool {
        self.is_default_config
    }

    pub fn initialize(&mut self, path: String) {
        self.github_token_file_path = path;
        self.is_default_config = false;
    }
}

fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|mut path| {
        path.push("lazyissues");
        path.push("config.toml");
        path
    })
}

pub fn read_config() -> Result<Option<Config>, Box<dyn std::error::Error>> {
    match get_config_path() {
        Some(path) => {
            if !path.exists() {
                return Ok(None);
            }

            let contents = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(Some(config))
        }
        None => Err("Couldn't determine config directory".into()),
    }
}

pub fn create_config(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    match get_config_path() {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            let toml = toml::to_string(&config)?;
            fs::write(path, toml)?;
            Ok(())
        }
        None => Err("Couldn't determine config directory".into()),
    }
}
