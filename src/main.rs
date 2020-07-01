use crate::error::ToolsetResult;

mod benchmarker;
mod cli;
mod config;
mod docker;
mod error;
mod io;
mod metadata;
mod options;

fn main() -> ToolsetResult<()> {
    cli::run()
}
