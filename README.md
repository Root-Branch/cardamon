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
- [Environment Variables](#environment-variables)
- [Configuration](#configuration)
- [CLI](#cli)
- [FAQ](#faq)
- [License](#license)

# Installation

The easiest way to install Cardamon is using our install script.

**Linux & Mac**

`curl -fsSL https://cardamon.rootandbranch.io/install.sh | sh`

**Windows**

```
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
irm -Uri https://cardamon.rootandbranch.io/install.ps1 | iex
```

**Cargo**

Alternatively you can build Cardamon from source if you have `cargo` installed.

`cargo install cardamon`

# Quickstart

Create a new cardamon configuration file using `cardamon init` and following the on screen instructions.

This will place a new cardamon.toml file with example processes, scenarios and observations in the directory you ran the init command. 

To run an observation use `cardamon run <observation name>`.

To see the stats gathered by previous runs use `cardamon stats`

# Environment Variables

By default, Cardamon saves your data to a locally stored SQLite database. If you would like to stire Cardamon data in any other location then you can set the following environment variables.   
**DATABASE_URL**

(omit database name from URL when using postgresql or mysql, use DATABASE_NAME instead)

**DATABASE_NAME**

(only required for postgresql and mysql)

# Configuration

### CPU

This contains information about the CPU used to run your application. The options are as follows:

***name***
- *type: string*
- *required: true*

*The manufacturers name of your processor.*

***avg_power***
- *type: float*
- *required: true*

*The processors average power consumption in watts*

### Processes

Processes are the things you would like cardamon to start/stop and measure during a run. Currently only executables and docker containers are supported but podman and kubernetes are planned.

You can specify as many processes as you like. The options for each process are as follows: 

***name***
- *type: string*
- *required: true*
   
*must be unique.*

***up***
- *type: string*
- *required: true*
  
*The command to start this process.*

***down***
- *type: string*
- *required: false*
- *default: empty string*

*The command to stop this process. Cardamon will pass the PID of the process to this command. You can
use `{pid}` as a placeholder in the command e.g. `kill {pid}`.*

***proccess.type***
- *type: "baremetal" | "docker"*
- *required: true*

*The type of process which is being executed.*

***process.containers***
- *type: string[]*
- *required: true (if process.type equals "docker"*

*Docker processes may initiate multiple containers from a single command, e.g. `docker compose up -d`. This is the list of containers started by this process that you would like cardamon to measure.*

***redirect.to***
- *type: "null" | "parent" | "file"*
- *required: false*
- *default: "file"*

*Where to redirect this processes stdout and stderr. "null" ignores output, "parent" attaches the processes output to three cardamon process, "file"
writes stdout and stderr to a file of the same name as this process e.g. <process name>.stdout.*

***EXAMPLE:***
```
[[process]]
name = "db"
up = "docker compose up -d"
down = "docker compose down"
redirect.to = "file"
process.type = "docker"
process.containers = ["postgres"]

[[process]]
name = "test_proc"
up = "bash -c \"while true; do shuf -i 0-1337 -n 1; done\""
down = "kill {pid}"
redirect.to = "file"
process.type = "baremetal"
```

### Scenarios

Scenarios are designed to put your application under some amount of load. they should represent some use case of your application. For example, if you're application is a REST API a scenario may simply be a list of curl commands performing some tasks.

***name***
- *type: string*
- *required: true*

*Must be unique.*

***desc***
- *type: string*
- *required: false*

*A short description of the scenario to remind you what it does.*

***command***
- *type: string*
- *required: true*

*The command to execute this scenario.*

***iterations***
- *type: integer*
- *required: false*
- *default: 1*

*The number of times cardamon should execute this scenario per run. It's better to run scenarios multiple times and take an average.*

***processes***
- *type: string[]*
- *required: true*

*A list of the processes which need to be started before executing this scenario.*

***EXAMPLE***
```
[[scenario]]
name = "sleep"
desc = "Sleeps for 10 seconds, a real scenario would call your app"
command = "sleep 10"
iterations = 2
processes = ["test_proc"]
```

### Observations

Observations are named "runs". They can specify one or more scenarios to run out they can run cause cardamom to run in "live monitor" mode.

Observations have the following properties:

***name***
- *type: string*
- *required: true*

*Must be unique.*

***scenarios***
- *type: string[]*
- *required: true if no processes are defined.*

*A list of scenarios to execute whilst observing the application.*

***processes***
- *type: string[]*
- *required - true if no scenarios are defined.*

*A list of processes to execute and observe. Running an observation with this property set runs Cardamon in Live Monitor mode.*

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

# FAQ

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
