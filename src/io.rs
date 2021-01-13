use crate::config::{Named, Test};
use crate::docker::Verification;
use crate::error::ToolsetError::InvalidFrameworkBenchmarksDirError;
use crate::error::{ToolsetError, ToolsetResult};
use crate::metadata;
use crate::results::Results;
use chrono::Utc;
use colored::Colorize;
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// `Logger` is used for logging to stdout and optionally to a file.
///
/// Note: `Logger` **is not** threadsafe. In most cases, if you *have* a
///       reference to a `Logger` that does not have a `log_file`, in order
///       to log to a file, clone the `Logger` then set `log_file`.
#[derive(Debug, Clone)]
pub struct Logger {
    prefix: Option<String>,
    results_dir: Option<PathBuf>,
    log_dir: Option<PathBuf>,
    log_file: Option<PathBuf>,
    pub quiet: bool,
}

impl Logger {
    /// Helper function for creating a simple Logger which will only print to
    /// stdout by default.
    /// Note: this Logger can later be configured to write to a file, but the
    /// other convenience functions are probably preferable.
    pub fn default() -> Logger {
        Logger {
            prefix: None,
            results_dir: None,
            log_dir: None,
            log_file: None,
            quiet: false,
        }
    }

    /// Helper function for creating a simple Logger with a given `prefix`.
    /// Note: this Logger can later be configured to write to a file, but the
    /// other convenience functions are probably preferable.
    pub fn with_prefix(prefix: &str) -> Logger {
        Logger {
            prefix: Some(prefix.to_string()),
            results_dir: None,
            log_dir: None,
            log_file: None,
            quiet: false,
        }
    }

    /// Helper function for creating a simple Logger with a given `log_dir`.
    /// Note: this Logger can later be configured to write to a file, but the
    /// other convenience functions are probably preferable.
    pub fn in_dir(log_dir: &str) -> Logger {
        let log_dir = PathBuf::from(log_dir);

        Logger {
            prefix: None,
            results_dir: Some(log_dir.clone()),
            log_dir: Some(log_dir),
            log_file: None,
            quiet: false,
        }
    }

    /// Sets the `prefix` of this `Logger` to the `Test`'s name and creates the
    /// sub-directory for this `Test`s logs.
    ///
    /// Note: This function updates `log_dir` to be the directory beneath
    /// `log_dir` given by `Test`'s name.
    ///
    /// Example: If `log_dir` was `/results/20200619191252` and this function
    ///          was passed `gemini`, `log_dir` would be updated to
    ///          `/results/20200619191252/gemini`.
    pub fn set_test(&mut self, test: &Test) {
        if let Some(log_dir) = &self.log_dir {
            let mut log_dir = log_dir.clone();
            log_dir.push(test.get_name());

            if !log_dir.exists() && std::fs::create_dir_all(&log_dir).is_err() {
                return;
            }

            self.log_dir = Some(log_dir);
        }

        self.prefix = Some(test.get_name());
    }

    /// Sets the path to the file to which `log` calls will write.
    ///
    /// Note: This function relies upon `log_dir` being set prior to the call.
    ///       If this `Logger` does not have a `log_dir` set prior, it will
    ///       result in a no-op.
    pub fn set_log_file(&mut self, file_name: &str) {
        if let Some(mut log_file) = self.log_dir.clone() {
            log_file.push(file_name);

            if !log_file.exists() && File::create(&log_file).is_err() {
                return;
            }
            self.log_file = Some(log_file);
        }
    }

    /// Logs output to standard out and optionally to the given file in the
    /// configured `log_dir`.
    pub fn log<T>(&self, text: T) -> ToolsetResult<()>
    where
        T: std::fmt::Display,
    {
        for line in text.to_string().lines() {
            if !line.trim().is_empty() {
                let bytes_with_colors = line.as_bytes();
                if let Some(log_file) = &self.log_file {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(log_file)
                        .unwrap();
                    file.write_all(strip_ansi_escapes::strip(&bytes_with_colors)?.as_slice())?;
                    file.write_all(&[b'\n'])?;
                }
                if !self.quiet {
                    if let Some(prefix) = &self.prefix {
                        print!("{}: ", prefix.white().bold());
                    }
                    println!("{}", line.trim_end());
                }
            }
        }
        Ok(())
    }

    /// Serializes and writes the given `results` to `results.json` in the root
    /// of the current `results` directory.
    pub fn write_results(&self, results: &Results) -> ToolsetResult<()> {
        if let Some(results_dir) = &self.results_dir {
            let mut results_file = results_dir.clone();
            results_file.push("results.json");

            if !results_file.exists() {
                File::create(&results_file)?;
            }

            let mut file = OpenOptions::new()
                .write(true)
                .append(false)
                .open(results_file)
                .unwrap();
            file.write_all(serde_json::to_string(results).unwrap().as_bytes())?;
            file.write_all(&[b'\n'])?;
        }

        Ok(())
    }

    /// Logs output to standard out and optionally to the given file in the
    /// configured `log_dir`.
    pub fn error<T>(&self, text: T) -> ToolsetResult<()>
    where
        T: std::fmt::Display,
    {
        self.log(text.to_string().red())
    }
}

/// Walks the FrameworkBenchmarks directory (and subs) searching for test
/// implementation config files, parses the configs, collects the list of all
/// frameworks, and prints their name to standard out.
pub fn print_all_frameworks() -> ToolsetResult<()> {
    print_all(metadata::list_all_frameworks())
}

/// Walks the FrameworkBenchmarks directory (and subs) searching for test
/// implementation config files, parses the configs, collects the list of all
/// test implementations, and prints their name to standard out.
pub fn print_all_tests() -> ToolsetResult<()> {
    print_all(metadata::list_all_tests())
}

/// Walks the FrameworkBenchmarks directory (and subs) searching for test
/// implementation config files, parses the configs, collects the list of
/// all framework, filters out ones without the given tag, and prints each
/// to standard out.
pub fn print_all_tests_with_tag(tag: &str) -> ToolsetResult<()> {
    print_all(metadata::list_tests_by_tag(tag))
}

/// Walks the FrameworkBenchmarks directory (and subs) searching for test
/// implementation config files, parses the configs, collects the list of
/// all frameworks with the given name, and prints each test to standard
/// out.
pub fn print_all_tests_for_framework(framework: &str) -> ToolsetResult<()> {
    print_all(metadata::list_tests_for_framework(framework))
}

/// Gets the `FrameworkBenchmarks` `PathBuf` for the running context.
pub fn get_tfb_dir() -> ToolsetResult<PathBuf> {
    let mut tfb_path = PathBuf::new();
    if let Ok(tfb_home) = env::var("TFB_HOME") {
        tfb_path.push(tfb_home);
    } else if let Some(mut home_dir) = dirs::home_dir() {
        home_dir.push(".tfb");
        tfb_path = home_dir;
        if !tfb_path.exists() {
            if let Ok(current_dir) = env::current_dir() {
                tfb_path = current_dir;
            }
        }
    }

    let mut frameworks_dir = tfb_path.clone();
    frameworks_dir.push("frameworks");
    if !frameworks_dir.exists() {
        return Err(InvalidFrameworkBenchmarksDirError(
            frameworks_dir.to_str().unwrap().to_string(),
        ));
    }

    Ok(tfb_path)
}

/// Creates the result directory and timestamp subdirectory for this run.
pub fn create_results_dir() -> ToolsetResult<String> {
    let result_dir = format!("results/{}", Utc::now().format("%Y%m%d%H%M%S"));
    std::fs::create_dir_all(&result_dir)?;

    Ok(result_dir)
}

/// Produces user-consumable output for the given verifications.
pub fn report_verifications(
    verifications: Vec<Verification>,
    mut logger: Logger,
) -> ToolsetResult<()> {
    logger.set_log_file("benchmark.txt");
    let mut test_results = HashMap::new();
    for verification in &verifications {
        if !test_results.contains_key(&verification.test_name) {
            let array: Vec<Verification> = Vec::new();
            test_results.insert(verification.test_name.clone(), array);
        }
        test_results
            .get_mut(&verification.test_name)
            .unwrap()
            .push(verification.clone());
    }
    let mut border_buffer = String::new();
    let mut mid_line_buffer = String::new();
    for _ in 0..79 {
        border_buffer.push('=');
        mid_line_buffer.push('-');
    }
    logger.log(&border_buffer.cyan())?;
    logger.log("Verification Summary".cyan())?;
    logger.log(&mid_line_buffer.cyan())?;

    for test_result in test_results {
        logger.log(format!("{} {}", "|".cyan(), test_result.0.cyan()))?;
        for verification in test_result.1 {
            if !verification.errors.is_empty() {
                logger.log(format!(
                    "{:8}{:13}: {:5} - {}",
                    "|".cyan(),
                    &verification.type_name.cyan(),
                    "ERROR".red(),
                    verification.errors.get(0).unwrap().short_message
                ))?;
            } else if !verification.warnings.is_empty() {
                logger.log(format!(
                    "{:8}{:13}: {:5} - {}",
                    "|".cyan(),
                    &verification.type_name.cyan(),
                    "WARN".yellow(),
                    verification.warnings.get(0).unwrap().short_message
                ))?;
            } else {
                logger.log(format!(
                    "{:8}{:13}: {:5}",
                    "|".cyan(),
                    &verification.type_name.cyan(),
                    "PASS".green(),
                ))?;
            }
        }
    }
    logger.log(format!("{}{}", &border_buffer.cyan(), "".clear()))?;

    Ok(())
}

//
// PRIVATES
//

/// Helper function to print a vector of `Named` entries to standard out.
fn print_all<T: Named>(result: Result<Vec<T>, ToolsetError>) -> ToolsetResult<()> {
    match result {
        Ok(list) => {
            for test in list {
                println!("{}", test.get_name());
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

//
// TESTS
//

#[cfg(test)]
mod tests {
    use crate::io::get_tfb_dir;
    use crate::io::print_all_frameworks;
    use crate::io::print_all_tests;
    use crate::io::print_all_tests_with_tag;
    use crate::metadata::TAG_BROKEN;

    #[test]
    fn it_will_get_a_valid_tfb_dir() {
        match get_tfb_dir() {
            Ok(_) => {}
            Err(e) => panic!("io::get_tfb_dir failed. error: {:?}", e),
        };
    }

    #[test]
    fn it_can_print_all_tests() {
        match print_all_tests() {
            Ok(_) => {}
            Err(e) => panic!("io::print_all_tests failed. error: {:?}", e),
        };
    }

    #[test]
    fn it_can_print_all_frameworks() {
        match print_all_frameworks() {
            Ok(_) => {}
            Err(e) => panic!("io::print_all_frameworks failed. error: {:?}", e),
        };
    }

    #[test]
    fn it_can_print_all_tests_with_tag() {
        match print_all_tests_with_tag(TAG_BROKEN) {
            Ok(_) => {}
            Err(e) => panic!("io::print_all_tests_with_tag failed. error: {:?}", e),
        };
    }
}
