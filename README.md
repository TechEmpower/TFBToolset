# TFBToolset

[![Build Status](https://github.com/TechEmpower/TFBToolset/workflows/build/badge.svg?branch=master&event=push)](https://github.com/TechEmpower/TFBToolset/actions?query=workflow%3Abuild+branch%3Amaster)

The suite of tools that are run in the TechEmpower Framework
Benchmarks. This application is a stand-alone executable which orchestrates
several functions: auditing existing test implementations, running benchmarks, 
running test implementation verifications, etc.

The goal of this application is to live in isolation from the test 
implementations. Separately, the executable that is built from this toolset
will execute against the test implementations as local data.

## Getting Started

These instructions will get you a copy of the project up and running on your 
local machine for development and testing purposes.

### Prerequisites

* [TechEmpower Frameworks](https://github.com/TechEmpower/FrameworkBenchmarks)
* [Rust](https://rustup.rs/)
* [Docker](https://docs.docker.com/engine/install/) or [Docker4Windows](https://docs.docker.com/docker-for-windows/install/)
* [Git](https://git-scm.com/) (required for benchmarking only)

#### Windows Only

* [Expose daemon on `tcp://localhost:2375`](https://docs.docker.com/docker-for-windows/#general)

### EnvVars

To run any tests, the toolset needs to know the location of `FrameworkBenchmarks`.
There are three places the toolset searches (in order):

* Environment variable `TFB_HOME`
* Home directory; e.g. `~/.tfb`
* Current directory

### Running the tests

```
$ cargo test
```

### Building

```
$ cargo build --release
```

### Installing

The executable `tfb_toolset` (`tfb_toolset.exe` on Windows) only needs to be on 
the `PATH`.

## Running

#### Verify Example

Unix:
```
$ cd TFBToolset
$ cargo build --release
$ cd ..
$ git clone https://github.com/TechEmpower/FrameworkBenchmarks.git
$ cd FrameworkBenchmarks
$ ../TFBToolset/target/release/tfb_toolset -m verify --test gemini
```

Windows:
```
> cd TFBToolset
> cargo build --release
> cd ..
> git clone https://github.com/TechEmpower/FrameworkBenchmarks.git
> cd FrameworkBenchmarks
> ..\TFBToolset\target\release\tfb_toolset.exe -m verify --test gemini
```

## Authors

* **Mike Smith** - *Initial work* - [msmith-techempower](https://github.com/msmith-techempower)

## License

This project is licensed under the BSD-3-Clause License - see the [LICENSE](LICENSE) file for details
