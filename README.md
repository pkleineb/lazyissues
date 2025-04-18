# A TUI issues, pr, projects and discussions tracker
Currently this is a WIP. And only Github integration is implemented.

## Installation
Since this is a WIP the only way to use is by compiling it yourself using: `cargo build`.

## Github Authentication
Github authentication is best achieved by [creating a personal access token(classic)](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-personal-access-token-classic). You may also use a [fine grained personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token) for a more secure way to view issues and pull requests. However you wont be able to view projects.

To use all features of lazyissues you need these scopes:
`repo`
`discussion`
`projects`

## Configuration
```kdl
github_token_path "/path/to/github/personal_access_token"
```
