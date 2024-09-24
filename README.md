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

- [Introduction](#introduction)
- [Installation](#installation)
- [Quickstart](#quickstart)
- [Environment Variables](#environment-variables)
- [Configuration](#configuration)
- [CLI](#cli)
- [FAQ](#faq)
- [License](#license)

# Introduction

Cardamon is built around the concept of observations and scenarios.

A scenario encapsulates a usage behaviour that you want to measure (e.g. add items to basket). You can then run your code against these repeateable behaviours and see how your software power consumption changes over time. You can view this in the cardamon-ui. Cardamon scenarios are compatible with [ISO/IEC 21031 - Software Carbon Intensity (SCI) specification](https://www.iso.org/standard/86612.html).

An observation is a measurement of one or more scenarios.

# Installation

The easiest way to install Cardamon is using our install script.

**Linux & Mac**

`curl -fsSL https://cardamon.io/install.sh | sh`

**Windows**

```
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
irm -Uri https://cardamon.io/install.ps1 | iex
```

**Cargo**

Alternatively you can build Cardamon from source if you have `cargo` installed.

`cargo install cardamon`

# Quickstart

`cardamon init` - create a new cardamon configuration file.

`cardamon run <observation name>` - runs the specified observation.

`cardamon stats` - shows stats per scenario

# Environment Variables

By default, Cardamon saves your data to a locally stored SQLite database. If you would like to store Cardamon data in any other location then you can set the following environment variables.   
**DATABASE_URL**

(omit database name from URL when using postgresql or mysql, use DATABASE_NAME instead)

**DATABASE_NAME**

(only required for postgresql and mysql)

# Configuration

### CPU

This contains information about the CPU used to run your application. The options are as follows:

```toml
[cpu]

# The manufacturers name for your processor
name = "AMD Ryzen 7 PRO 6850U with Radeon Graphics"

# The processors average power consumption in watts
avg_power = 11.223
```

### Processes

Processes are the things you would like cardamon to start/stop and measure during a run. Currently only executables and docker containers are supported but podman and kubernetes are planned. You can specify as many processes as you like. Below is an example process: 

```toml
[[process]]

# must be unique
name = "db"

# The command to start this process
up = "docker compose up -d"

# (OPTIONAL) The command to stop this process. Cardamon will pass the PID of the process to this command. You can
# use `{pid}` as a placeholder in the command e.g. `kill {pid}`
down = "docker compose down"

# The type of process which is being executed. Can be "docker" | "baremetal"
process.type = "docker"

# (OPTIONAL) Docker processes may initiate multiple containers from a single command, e.g. `docker compose up -d`.
# This is the list of containers started by this process that you would like cardamon to measure
process.containers = ["postgres"]

# (OPTIONAL) Where to redirect this processes stdout and stderr. "null" ignores output, "parent" attaches the
# processes output to three cardamon process, "file" writes stdout and stderr to a file of the same name as this
# process e.g. <process name>.stdout. Will default to "file"
redirect.to = "file"
```

### Scenarios

Scenarios are designed to put your application under some amount of load. they should represent some use case of your application. For example, if you're application is a REST API a scenario may simply be a list of curl commands performing some tasks.

```toml
[[scenario]]

# Must be unique
name = "sleep"

# (OPTIONAL) A short description of the scenario to remind you what it does
desc = "Sleeps for 10 seconds, a real scenario would call your app"

# The command to execute this scenario
command = "sleep 10"

# (OPTIONAL) The number of times cardamon should execute this scenario per run. It's better to run scenarios
# multiple times and take an average. Defaults to 1
iterations = 2

# A list of the processes which need to be started before executing this scenario
processes = ["test_proc"]
```

### Observations

An observation is how we take a 'measurement'. Observations can be run in two modes. As a live monitor, where you specify processes to measure. Or as a scenario runner, where you specify scenarios to run. 

```toml
[[observation]]

# Must be unique
name = "my_observation"

# A list of scenarios to execute whilst observing the application. Only required if no processes are defined
scenarios = ["sleep"]

# A list of processes to execute and observe. Running an observation with this property set runs Cardamon in
# Live Monitor mode
processes = ["test_proc"]
```

# CLI

### Init

`cardamon init`

Produces a new cardamon.toml file.

### Run

`cardamon run <observation_name>`

Runs a single observation.

***Options***
- ***name**: The name of the observation you would like to run*
- ***pids**: A comma separated list of PIDs started externally to cardamon that you would like cardamon to measure*
- ***containers**: A comma separated list of container names, started externally to cardamon, that you would like cardamon to measure*
- ***external_only**: If set, cardamon will not try to start any processes and will instead only measure the pids specified by the `pids` and `containers` option*

### Stats

`cardamon stats [scenario_name]`

Shows the stats for previous runs of scenarios.

***Options***
- ***scenario_name**: An optional argument for the scenario you want to show stats for*
- ***previous_runs**: The number of previous runs to show*

### Ui

`cardamon ui [port]`

Start the UI server.

***Options***
- ***port**: The port to listen on*

**DATABASE_URL**
Connection string to the database

examples:
`sqlite://cardamon.db?mode=rwc` (rwc required to create db file if it doesn't exist)
`postgresql://postgres@localhost:5432` (don't include db name for postgres or mysql)

**DATABASE_NAME**
only required for postgres and mysql

### Migrations

`cargo run --bin migrator -- <COMMAND>`

### Generating Entities

`sea-orm-cli generate entity -o src/entities`

## FAQ

### Can I use Cardamon on my own project or at my work?

> Cardamon is released under the PolyForm Shield License 1.0. Anyone can use Cardamon in-house to build their own software, including commercially. 
> If you want to use Cardamon to offer a competing service to Root & Branch (e.g. instrument another company's software) then you will need permission, please get in touch. We have lots of green software industry friends who are able to use Cardamon.

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
