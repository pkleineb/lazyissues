# A TUI issues, pr, projects and discussions tracker
Currently this is a WIP. And only Github integration is implemented.

## Installation
Since this is a WIP the only way to use is by compiling it yourself using: `cargo build`.

## Github Authentication
Github authentication is best achieved by [creating a personal access token(classic)](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-personal-access-token-classic). 

You may also use a [fine grained personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#creating-a-fine-grained-personal-access-token) for a more secure way to view issues and pull requests. However you wont be able to view projects.

To use all features of lazyissues you need these scopes:
`repo`
`discussion`
`projects`

## Configuration
Config is written in [kdl](https://kdl.dev/) and Lazyissues looks for a directory with a file in the [`config_local_dir`](https://docs.rs/dirs/latest/dirs/fn.config_local_dir.html) named `lazyissues/config.kdl`.

|          Platform              |                                                  Path                                               |
---------------------------------|------------------------------------------------------------------------------------------------------
|           Linux                |           `$XDG_CONFIG_HOME/lazyissues/config.kdl` or `$HOME/.config/lazyissues/config.kdl`         |
|           MacOS                |                       `$HOME/Library/Application Support/lazyissues/config.kdl`                     |
|          Windows               |`{FOLDERID_LocalAppData}\lazyissues\config.kdl`(`C:\Users\Alice\AppData\Local\lazyissues\config.kdl`)|

### Config options

#### github_token_path
To be able to view issues, pr's, and projects you need to set this option.
The option takes one path argument pointing to the location where the personal access token file is located on your machine.
```kdl
github_token_path "/path/to/github/personal_access_token"
```

#### tags
`tags` defines custom tags and their color for displaying on your issues and pr's.
It takes an array of key value pairs where the key represents the tag name and the value represents the color that is associated with said tag. This color can be a named color, rgb values or an [indexed color](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).
```kdl
tags {
    bug "blue"
    critical "#F06996"
    documentation "10"
}
```

### time_format
`time_format` declares a format in which to display the timestamps of issue, pr's, projects and comments on them. It takes any valid time format as a string. The default one is displayed down below.
```kdl
time_format "%H:%M %d.%m.%Y"
```
