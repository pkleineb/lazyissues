use std::path::PathBuf;

use git2::Repository;

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

pub fn get_git_repo_root() -> Result<PathBuf, git2::Error> {
    // Open the repository at the current directory
    let repo = Repository::open(".")?;

    // Get the repository workdir (root path)
    repo.workdir()
        .map(|path| path.to_path_buf())
        .ok_or_else(|| git2::Error::from_str("Could not find repository root"))
}
