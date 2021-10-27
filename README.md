<div align="center">
  <h1>🦊 koji</h1>

  An interactive CLI for creating [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/),
  built on [cocogitto](https://github.com/oknozor/cocogitto) and inspired by
  [cz-cli](https://github.com/commitizen/cz-cli).
  
  [![Current Release](https://img.shields.io/github/v/release/its-danny/koji)](https://github.com/its-danny/koji/releases)
  [![GitHub Workflow Status](https://img.shields.io/github/workflow/status/its-danny/koji/CI)](https://github.com/its-danny/koji/actions)
  [![Dependency Status](https://deps.rs/repo/github/its-danny/koji/status.svg)](https://deps.rs/repo/github/its-danny/koji)
  [![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-pink.svg)](https://conventionalcommits.org)
  [![License](https://img.shields.io/github/license/oknozor/cocogitto)](LICENSE)

  [![Commit with koji](https://github.com/its-danny/koji/raw/main/meta/demo.gif)](https://github.com/its-danny/koji/raw/main/meta/demo.gif)
</div>

## Features

- Create conventional commits with ease
- Help contributors do the same without them having to know how to write one
- Use emoji with your commit types if you'd like 👋
- ~~Run as a git hook~~ ([soon](https://github.com/its-danny/koji/issues/2))
- A single binary so you don't need to bring along a whole other ecosystem to your project

## Installation

Check the [releases](https://github.com/its-danny/koji/releases) page to download koji for your platform.

This will soon enough be made easier with [webinstall.dev](https://github.com/its-danny/koji/issues/10).

## Usage

### Using koji

```bash
# Do some work
cd dev/work-stuff
git add .env.production

# Create a conventional commit
koji
```

### Fancy it up with emoji

Passing `-e` or `--emoji` to `koji` will prepend your commit message
with an emoji related to the commit type. The default emoji can be seen
[here](https://github.com/its-danny/koji/blob/main/meta/config/koji-default.toml).

### Add custom commit types

You can add custom commit types via a `koji.toml` file in the working directory.
Some examples can be found [here](https://github.com/its-danny/koji/blob/main/meta/config).
