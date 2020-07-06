use crate::benchmarker::{modes, Benchmarker};
use crate::error::ToolsetError::UnknownBenchmarkerModeError;
use crate::error::ToolsetResult;
use crate::io::get_tfb_dir;
use crate::{io, options};

/// Runs the CLI matching the arguments/options passed and handling each.
pub fn run() -> ToolsetResult<()> {
    let mut app = options::parse();
    let matches = app.clone().get_matches();

    if matches.is_present(options::args::AUDIT) {
        // todo
        println!("AUDIT");
        Ok(())
    } else if matches.is_present(options::args::CLEAN) {
        let mut tfb_dir = get_tfb_dir()?;
        tfb_dir.push("results");
        std::fs::remove_dir_all(&tfb_dir)?;
        Ok(())
    } else if matches.is_present(options::args::LIST_FRAMEWORKS) {
        io::print_all_frameworks()
    } else if matches.is_present(options::args::LIST_TESTS) {
        io::print_all_tests()
    } else if let Some(framework) = matches.value_of(options::args::LIST_TESTS_FOR_FRAMEWORK) {
        io::print_all_tests_for_framework(framework)
    } else if let Some(tag) = matches.value_of(options::args::LIST_TESTS_WITH_TAG) {
        io::print_all_tests_with_tag(tag)
    } else if matches.is_present(options::args::PARSE_RESULTS) {
        // todo
        println!("PARSE_RESULTS");
        Ok(())
    } else if let Some(mode) = matches.value_of(options::args::MODE) {
        let mut benchmarker = Benchmarker::new(matches.clone());
        match mode {
            modes::BENCHMARK => benchmarker.benchmark(),
            modes::VERIFY => benchmarker.verify(),
            modes::DEBUG => benchmarker.debug(),
            _ => Err(UnknownBenchmarkerModeError(mode.to_string())),
        }
    } else {
        Ok(app.print_help().unwrap())
    }
}
