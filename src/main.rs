use crate::error::ToolsetResult;

mod benchmarker;
mod cli;
mod config;
mod docker;
mod error;
mod io;
mod metadata;
mod options;

#[macro_use]
extern crate lazy_static;
extern crate regex;

fn main() -> ToolsetResult<()> {
    cli::run()
}
