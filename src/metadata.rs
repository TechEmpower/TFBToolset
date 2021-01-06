//! The metadata module supports the functionality of finding and parsing of
//! test implementation configuration files, as well as returning useful
//! structs representing those configuration files.

use crate::config::{Framework, Named, Project, Test};
use crate::error::ToolsetResult;
use crate::io::Logger;
use crate::{config, io, options};
use clap::ArgMatches;
use glob::glob;
use std::path::PathBuf;

/// Walks the FrameworkBenchmarks directory's `framework` sub-dir to find all
/// test implementations' `config.toml`, parse each file, and pushes the top-
/// level `framework` to the return Vec.
pub fn list_all_frameworks() -> ToolsetResult<Vec<Framework>> {
    let mut frameworks: Vec<Framework> = Vec::new();
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push("frameworks/*/*/config.toml");
    for path in glob(tfb_path.to_str().unwrap()).unwrap() {
        frameworks.push(config::get_framework_by_config_file(&path.unwrap())?);
    }

    Ok(frameworks)
}

/// Walks the FrameworkBenchmarks directory's `framework` sub-dir to find all
/// test implementations' `config.toml`, parse each file, and pushes the top-
/// level `tests` to the return Vec.
pub fn list_all_tests() -> ToolsetResult<Vec<Test>> {
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push("frameworks/*/*/config.toml");

    get_test_implementations_by_path(&tfb_path)
}

/// Walks the FrameworkBenchmarks directory's `framework` sub-dir to find all
/// test implementations' `config.toml`, parse each file, and pushes each test
/// implementation found.
pub fn list_tests_for_framework(framework_name: &str) -> ToolsetResult<Vec<Test>> {
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push(format!(
        "frameworks/*/{}/config.toml",
        framework_name.to_lowercase()
    ));

    get_test_implementations_by_path(&tfb_path)
}

/// Walks the FrameworkBenchmarks directory's `framework` sub-dir to find all
/// test implementations' `config.toml`, parse each file, and pushes the top-
/// level `Test`s with the given `tag` to the return Vec.
pub fn list_tests_by_tag(tag: &str) -> ToolsetResult<Vec<Test>> {
    let mut test_implementations = Vec::new();
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push("frameworks/*/*/config.toml");
    for path in glob(tfb_path.to_str().unwrap()).unwrap() {
        for test in config::get_test_implementations_by_config_file(&path.unwrap())? {
            if test.tags.is_some() && test.clone().tags.unwrap().contains(&tag.to_string()) {
                test_implementations.push(test);
            }
        }
    }

    Ok(test_implementations)
}

/// Walks the FrameworkBenchmarks directory's `framework` sub-dir to find all
/// test implementations' `config.toml`, parses each file, and returns the
/// granular `Project`s which have `Test`s with the given name.
/// This returns a `Vec` type because there is no guarantee of uniqueness in a
/// `Framework` name - there can be many `Gemini` frameworks of different
/// languages. However, there is a uniqueness constraint on
/// (language, framework name).
///
/// Example:
/// Say that (Java, FooFramework) and (C#, FooFramework)
/// both have a `default` test implementation, then this would return the
/// `Project`s for both when queried with "FooFramework".
pub fn list_projects_by_test_name(
    test_name: Option<String>,
    test_type: Option<&str>,
) -> ToolsetResult<Vec<Project>> {
    let mut projects = Vec::new();
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push("frameworks/*/*/config.toml");
    for path in glob(tfb_path.to_str().unwrap()).unwrap() {
        let path_buf: &PathBuf = &path.unwrap();
        let project_name = config::get_project_name_by_config_file(&path_buf)?;
        let framework = config::get_framework_by_config_file(&path_buf)?;
        let mut tests = Vec::new();
        let language = config::get_language_by_config_file(&framework, &path_buf)?;
        for mut test in config::get_test_implementations_by_config_file(&path_buf)? {
            test.specify_test_type(test_type);
            if let Some(name) = &test_name {
                if test.get_name() == *name {
                    tests.push(test);
                }
            } else {
                tests.push(test);
            }
        }
        if !tests.is_empty() {
            projects.push(Project {
                name: project_name,
                framework,
                tests,
                language,
            });
        }
    }

    Ok(projects)
}

pub fn list_projects_by_language_name(
    language_name: Option<String>,
    test_type: Option<&str>,
) -> ToolsetResult<Vec<Project>> {
    let mut projects = Vec::new();
    let mut tfb_path = io::get_tfb_dir()?;
    tfb_path.push("frameworks/*/*/config.toml");
    for path in glob(tfb_path.to_str().unwrap()).unwrap() {
        let path_buf: &PathBuf = &path.unwrap();
        let project_name = config::get_project_name_by_config_file(&path_buf)?;
        let framework = config::get_framework_by_config_file(&path_buf)?;
        let mut tests = Vec::new();
        let language = config::get_language_by_config_file(&framework, &path_buf)?;
        if let Some(language_name) = &language_name {
            if language_name.to_lowercase() == language.to_lowercase() {
                for mut test in config::get_test_implementations_by_config_file(&path_buf)? {
                    test.specify_test_type(test_type);
                    tests.push(test);
                }
                if !tests.is_empty() {
                    projects.push(Project {
                        name: project_name,
                        framework,
                        tests,
                        language,
                    });
                }
            }
        }
    }

    Ok(projects)
}

/// Convenience function for calling `metadata::list_projects_by_test_name(None)`.
pub fn list_all_projects() -> ToolsetResult<Vec<Project>> {
    list_projects_by_test_name(None, None)
}

/// Helper method to get the tests to run, specified or not.
pub fn list_projects_to_run(matches: &ArgMatches) -> Vec<Project> {
    let logger = Logger::default();
    let mut projects = Vec::new();
    if let Some(list) = matches.values_of(options::args::TEST_NAMES) {
        let test_names: Vec<&str> = list.collect();
        for test_name in test_names {
            match list_projects_by_test_name(
                Some(String::from(test_name)),
                matches.value_of(options::args::TYPES),
            ) {
                Ok(mut projects_found) => projects.append(&mut projects_found),
                Err(e) => logger
                    .error(format!(
                        "Error thrown collecting projects for test name: {}; {:?}",
                        test_name, e
                    ))
                    .unwrap(),
            };
        }
    } else if let Some(list) = matches.values_of(options::args::TEST_LANGUAGES) {
        let test_languages: Vec<&str> = list.collect();
        for language in test_languages {
            match list_projects_by_language_name(
                Some(String::from(language)),
                matches.value_of(options::args::TYPES),
            ) {
                Ok(mut projects_found) => projects.append(&mut projects_found),
                Err(e) => logger
                    .error(format!(
                        "Error thrown collecting projects for language name: {}; {:?}",
                        language, e
                    ))
                    .unwrap(),
            }
        }
    } else {
        match list_all_projects() {
            Ok(mut projects_found) => projects.append(&mut projects_found),
            Err(e) => logger
                .error(format!("Error thrown collecting all projects: {:?}", e))
                .unwrap(),
        };
    }

    if let Some(project) = projects.get(0) {
        if project.tests.is_empty() {
            logger
                .error(format!(
                    "Found no test implementations for project: {}",
                    project.framework.name
                ))
                .unwrap();
        }
    } else {
        logger
            .error(format!(
                "Found no project for the supplied test name(s): {}",
                matches
                    .values_of(options::args::TEST_NAMES)
                    .unwrap()
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
            .unwrap();
    }

    projects
}

//
// PRIVATES
//

fn get_test_implementations_by_path(path: &PathBuf) -> ToolsetResult<Vec<Test>> {
    let mut test_implementations = Vec::new();
    for path in glob(path.to_str().unwrap()).unwrap() {
        test_implementations
            .append(config::get_test_implementations_by_config_file(&path.unwrap())?.as_mut());
    }
    Ok(test_implementations)
}

//
// TESTS
//

#[cfg(test)]
mod tests {
    use crate::metadata::{
        list_all_frameworks, list_all_projects, list_all_tests, list_tests_by_tag,
        list_tests_for_framework,
    };

    #[test]
    fn it_can_list_all_frameworks() {
        if let Err(e) = list_all_frameworks() {
            panic!("metadata::list_all_frameworks failed. error: {:?}", e);
        };
    }

    #[test]
    fn it_can_list_all_tests() {
        if let Err(e) = list_all_tests() {
            panic!("metadata::list_all_tests failed. error: {:?}", e);
        };
    }

    #[test]
    fn it_can_list_all_projects() {
        if let Err(e) = list_all_projects() {
            panic!("metadata::list_all_projects failed. error: {:?}", e);
        };
    }

    #[test]
    fn it_can_list_all_tests_for_framework() {
        match list_tests_for_framework("Gemini") {
            Ok(tests) => !tests.is_empty(),
            Err(e) => panic!(
                "metadata::list_tests_for_framework(\"Gemini\") failed. error: {:?}",
                e
            ),
        };
    }

    #[test]
    fn it_can_list_all_tests_by_tag() {
        match list_tests_by_tag("Non-Existent Tag") {
            Ok(tests) => tests.is_empty(),
            Err(e) => panic!(
                "metadata::list_tests_by_tag(\"Non-Existent Tag\") failed. error: {:?}",
                e
            ),
        };
    }
}
