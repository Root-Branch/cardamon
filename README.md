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
- [CLI Reference](#cli-reference)
- [Configuration](#configuration)
- [Scenarios](#scenarios)
- [Live Monitor](#live-monitor)
- [FAQ](#faq)
- [License](#license)

## Environment Variables

DATABASE_URL (do not include database name for postgresql or mysql)
DATABASE_NAME (only required for postgresql and mysql)

## Installation

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

## CLI Reference

Coming soon!

## Configuration

Coming soon!

## Scenarios

A scenario encapsulates a usage behaviour that you want to measure (e.g. add items to basket). You can then run your code against these repeateable behaviours and see how your software power consumption changes over time. You can view this in the cardamon-ui. Cardamon scenarios are compatible with [ISO/IEC 21031 - Software Carbon Intensity (SCI) specification](https://www.iso.org/standard/86612.html). 

## Live Monitor

Coming soon!

# FAQ

### Can I use Cardamon on my own project or at my work?

> Cardamon is released under the PolyForm Shield License 1.0. Anyone can use Cardamon in-house to build their own software, including commercially. If you want to use Cardamon to offer a competing service to Root & Branch (e.g. instrument another company's software) then you will need permission, please get in touch. We have lots of green software industry friends who are able to use Cardamon.

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

## License

Cardamon is distributed under the terms of the PolyForm Shield License (Version 1.0).

See [LICENSE](https://www.mozilla.org/en-US/MPL/2.0) for details.

_Copyright © 2023 Root & Branch ltd_
