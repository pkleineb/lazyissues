use std::path::PathBuf;

use git2::Repository;

/// gets all remotes in the currently active git repo
pub fn get_remote_names() -> Result<Vec<String>, git2::Error> {
    let repo = Repository::open(".")?;
    let remotes = repo.remotes()?;

    let remote_names: Vec<String> = remotes
        .iter()
        .filter_map(|remote_name| remote_name.map(|name| name.to_string()))
        .collect();

    Ok(remote_names)
}

/// gets all remote names and urls in the currently active git repo as a tuple
pub fn get_remote_names_and_urls() -> Result<Vec<(String, String)>, git2::Error> {
    let repo = Repository::open(".")?;
    let remotes = repo.remotes()?;

    let remote_names_urls: Vec<(String, String)> = remotes
        .iter()
        .filter_map(|remote_name| {
            remote_name.and_then(|name| {
                repo.find_remote(name)
                    .ok()
                    .and_then(|remote| remote.url().map(|url| (name.to_string(), url.to_string())))
            })
        })
        .collect();

    Ok(remote_names_urls)
}

/// get all remote urls in a the currently active git repo
pub fn get_remote_urls() -> Result<Vec<String>, git2::Error> {
    let repo = Repository::open(".")?;
    let remotes = repo.remotes()?;

    let remote_urls: Vec<String> = remotes
        .iter()
        .filter_map(|remote_name| {
            remote_name.and_then(|name| {
                repo.find_remote(name)
                    .ok()
                    .and_then(|remote| remote.url().map(|url| url.to_string()))
            })
        })
        .collect();

    Ok(remote_urls)
}

/// gets the current preferred remote of the currently active git repo
pub fn get_active_remote() -> Result<String, git2::Error> {
    let repo = Repository::open(".")?;

    // Try to get the upstream branch
    let head = repo.head()?;
    let head_branch = head.name().unwrap_or("HEAD");

    // Try to get the upstream branch
    if let Ok(branch) = repo.find_branch(head_branch, git2::BranchType::Local) {
        if let Ok(upstream) = branch.upstream() {
            match upstream.name() {
                Ok(Some(remote_name)) => {
                    if let Ok(remote) = repo.find_remote(remote_name) {
                        return Ok(remote.url().unwrap_or("").to_string());
                    }
                }
                Ok(None) => log::error!("Remote name was not valid utf-8."),
                _ => (),
            }
        }
    }

    // If no upstream found, try to get the default remote (usually 'origin')
    if let Ok(remote) = repo.find_remote("origin") {
        return Ok(remote.url().unwrap_or("").to_string());
    }

    // If no default remote found, get the first available remote
    if let Ok(remotes) = get_remote_urls() {
        if let Some(first_remote) = remotes.first() {
            return Ok(first_remote.clone());
        }
    }

    Err(git2::Error::from_str("No remote found"))
}

/// gets the currently active's git repo root
pub fn get_git_repo_root() -> Result<PathBuf, git2::Error> {
    // Open the repository at the current directory
    let repo = Repository::open(".")?;

    // Get the repository workdir (root path)
    repo.workdir()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| git2::Error::from_str("Could not find repository root"))
}

/// get the git remote url by the name of that remote
pub fn get_git_remote_url_for_name(name: &str) -> Result<String, git2::Error> {
    let repo = Repository::open(".")?;

    let remote = repo.find_remote(name)?;
    let url = remote.url().ok_or_else(|| {
        git2::Error::from_str(&format!(
            "Remote url not set for remote: {}",
            remote.name().unwrap_or("no remote name set")
        ))
    })?;

    Ok(url.to_string())
}
