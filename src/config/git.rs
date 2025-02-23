use std::process::Command;

pub fn get_remote_urls() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(["remote", "-v"]).output()?;

    let output_str = String::from_utf8(output.stdout)?;

    // Parse the remote output and extract unique URLs
    let urls: Vec<String> = output_str
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts.get(1).map(|&url| url.to_string())
        })
        .collect();

    Ok(urls)
}

// active_remote is just the first remote
pub fn get_active_remote() -> Result<String, Box<dyn std::error::Error>> {
    let urls = get_remote_urls()?;
    if let Some(first_element) = urls.first() {
        return Ok(first_element.to_owned());
    }

    Err("No remotes found in git repo".into())
}
