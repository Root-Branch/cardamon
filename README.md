<div align="center" markdown="1">
  <h1>Cardamon</h1>
  <p>🌱 The <b>Car</b>(<i>bon</i>) <b>da</b>(<i>shboard</i>) and live <b>mon</b>(<i>itor</i>)</p>
  <p>Built with ❤️ by the <a href="https://rootandbranch.io">Root & Branch</a> team</p>
  <small>
    <i>Uh, it's cardmom ACKSUALLY!</i> - we know, but cardamon is a better acronym.
  </small>
</div>

---

Cardamon is a tool to help development teams measure the power consumption and carbon emissions of their software.

- [Installation](#installation)
- [Quickstart](#quickstart)
- [Configuration](#configuration)
- [Scenarios](#scenarios)
- [Live Monitor](#live-monitor)
- [FAQ](#faq)
- [License](#license)

# Environment Variables

DATABASE_URL (do not include database name for postgresql or mysql)
DATABASE_NAME (only required for postgresql and mysql)

# Installation

The easiest way to install Cardamon is using our install script.

**Linux & Mac**

`curl -fsSL https://cardamon.rootandbranch.io/install.sh | sh`

**Windows**

```
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
irm -Uri https://cardamon.rootandbranch.io/install.ps1 | iex
```

### Cargo

Alternatively you can build Cardamon from source if you have `cargo` installed.

`cargo install cardamon`

# Quickstart

Create a new cardamon configuration file using `cardamon init` and following the on screen instructions.

This will place a new cardamon.toml file with example processes, scenarios and observations in the directory you ran the init command. 

To run an observation use `cardamon run <observation name>`.

To see the stats gathered by previous runs use `cardamon stats`

# Configuration

### CPU

### Processes

Processes are the things you would like cardamon to start/stop and measure during a run. Currently only executables and docker containers are supported but podman and kubernetes are planned.

You can specify as many processes as you like. The options for each process are as follows: 

**name** (required)
each process name must be unique

****


Each prices must have a **unique name** and a command for starting the process (the **up** command). This can be any shell command you like. 

Optionally you

### Scenarios

### Observations

# FAQ

### Can I use Cardamon on my own project or at my work?

> Cardamon is released under the PolyForm Shield License 1.0. This allows anyone to use Cardamon, in anyway they wish, as long as it is not used in a product or service which competes with Root & Branch Ltd (the company behind Cardamon).
>
> Root & Branch Ltd sell software consultancy services and use Cardamon internally to provide those services to their clients, so as long as you don't use Cardamon to provide a product or service similar to those offered by Root & Branch then you are free to use it any project, commercial or otherwise.

### I'd like to use Cardamon to measure the power consumption of my software, but I don't know how

> We're a friendly bunch! Feel free to create an issue in github (make sure to give the `help` label) and we will help in anyway we can. Alternatively email us at <hello@rootandbranch.io>

### How can I contribute?

> There are many ways you can contribute to the project.
>
> - Help us improve the documentation.
> - Translate the docs into other languages.
> - Create example projects to show others how to use Cardamon in their projects.
> - Checkout the issues board on github, there's always features and fixes that need implementing.
> - Spread the word! Tell others about the project and encourage them to use it.

# License

Cardamon is distributed under the terms of the PolyForm Shield License (Version 1.0).

See [LICENSE](https://www.mozilla.org/en-US/MPL/2.0) for details.

_Copyright © 2023 Root & Branch ltd_
