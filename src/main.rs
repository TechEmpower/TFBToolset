mod benchmarker;
mod cli;
mod config;
mod docker;
mod error;
mod io;
mod metadata;
mod options;
mod results;

#[macro_use]
extern crate lazy_static;
extern crate regex;

use crate::error::ToolsetResult;

fn main() -> ToolsetResult<()> {
    cli::run()
}
