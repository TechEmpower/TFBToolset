//! The config module contains all the structs relating to test implementation
//! configuration files.

use crate::error::ToolsetError::LanguageNotFoundError;
use crate::error::ToolsetResult;
use crate::io;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use toml::Value;

pub trait Named {
    fn get_name(&self) -> String;
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub framework: Framework,
    pub main: Test,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Framework {
    pub name: String,
    pub authors: Option<Vec<String>>,
    pub github: Option<String>,
}

impl Named for Framework {
    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Test {
    pub name: Option<String>,
    pub urls: HashMap<String, String>,
    pub approach: String,
    pub classification: String,
    pub orm: Option<String>,
    pub platform: String,
    pub webserver: String,
    pub os: String,
    pub database_os: Option<String>,
    pub database: Option<String>,
    pub versus: String,
    pub tags: Option<Vec<String>>,
    pub dockerfile: Option<String>,
}

impl Named for Test {
    fn get_name(&self) -> String {
        self.name.clone().unwrap()
    }
}

impl Test {
    pub fn get_tag(&self) -> String {
        format!("tfb.test.{}", self.get_name())
    }
    pub fn specify_test_type(&mut self, test_type: Option<&str>) {
        if let Some(test_type) = test_type {
            self.urls.retain(|key, _| key == test_type);
        }
    }
}

/// Project is the structure that represents the unit of data on which the
/// toolset operates. It houses all the data required about a
/// language-framework-tests relationship, as well as how to reconstruct the
/// path to the config file from which it was built.
#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub language: String,
    pub framework: Framework,
    pub tests: Vec<Test>,
}

impl Project {
    /// Returns the path of the project.
    pub fn get_path(&self) -> ToolsetResult<PathBuf> {
        let mut tfb_path = io::get_tfb_dir()?;
        tfb_path.push(format!(
            "frameworks/{}/{}",
            self.language,
            self.framework.get_name().to_lowercase()
        ));

        Ok(tfb_path)
    }
}

/// Gets the language of the specified config file.
pub fn get_language_by_config_file(framework: &Framework, file: &PathBuf) -> ToolsetResult<String> {
    let mut language = None;
    let mut next = false;
    for segment in file.ancestors() {
        if next {
            language = Some(segment.file_name().unwrap().to_str().unwrap());
            break;
        }
        if segment
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_lowercase()
            == framework.get_name().to_lowercase()
        {
            next = true;
        }
    }
    if language.is_none() {
        return Err(LanguageNotFoundError);
    }

    Ok(String::from(language.unwrap()))
}

/// Parses the given `&PathBuf` of a `config.toml` file and returns the
/// parsed framework block.
pub fn get_framework_by_config_file(file: &PathBuf) -> ToolsetResult<Framework> {
    let contents = std::fs::read_to_string(file)?;
    let config: Config = toml::from_str(&contents)?;

    Ok(config.framework)
}

/// Parses the given `&PathBug` of a `config.toml` file and return the parent
/// directory name as the project's name.
pub fn get_project_name_by_config_file(path_buf: &PathBuf) -> ToolsetResult<String> {
    let parent_dir = path_buf.parent().unwrap();
    Ok(parent_dir
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string())
}

/// Parses the given `&PathBuf` of a `config.toml` file and returns the vector
/// of test implementation blocks.
pub fn get_test_implementations_by_config_file(file: &PathBuf) -> ToolsetResult<Vec<Test>> {
    let mut tests: Vec<Test> = Vec::new();

    let contents = std::fs::read_to_string(file)?;
    let config: Config = toml::from_str(&contents)?;
    let parsed = contents.parse::<Value>()?;
    let table = parsed.as_table().unwrap();

    for key in table.keys() {
        if key != "framework" {
            let mut test: Test = toml::from_str(&toml::to_string(table.get(key).unwrap())?)?;
            let mut test_name = String::new();
            test_name.push_str(&config.framework.name.to_lowercase());
            if key != "main" {
                test_name.push('-');
                test_name.push_str(key);
            }
            test.name = Some(test_name);
            tests.push(test);
        }
    }

    Ok(tests)
}

//
// TESTS
//

#[cfg(test)]
mod tests {
    use glob::glob;

    use crate::config::Named;
    use crate::{config, io};

    #[test]
    fn it_can_get_framework_by_config_file() {
        match io::get_tfb_dir() {
            Ok(tfb_path) => {
                let mut tfb_path = tfb_path;
                tfb_path.push("frameworks/Java/gemini/config.toml");
                for path in glob(tfb_path.to_str().unwrap()).unwrap() {
                    match path {
                        Ok(path) => {
                            match config::get_framework_by_config_file(&path) {
                                Ok(framework) => assert_eq!(framework.get_name(), "Gemini"),
                                Err(e) => panic!(
                                    "config::get_framework_by_config_file(&path.unwrap()) failed. path: {:?}; error: {:?}",
                                    &path,
                                    e,
                                ),
                            };
                        }
                        Err(e) => panic!("glob() failed with error: {:?}", e),
                    }
                }
            }
            Err(e) => panic!("io::get_tfb_dir failed with error: {:?}", e),
        }
    }

    #[test]
    fn it_can_get_test_implementations_by_config_file() {
        match io::get_tfb_dir() {
            Ok(tfb_path) => {
                let mut tfb_path = tfb_path;
                tfb_path.push("frameworks/Java/gemini/config.toml");
                for path in glob(tfb_path.to_str().unwrap()).unwrap() {
                    match path {
                        Ok(path) => {
                            match config::get_test_implementations_by_config_file(&path) {
                                Ok(tests) => !tests.is_empty(),
                                Err(e) => panic!("config::get_test_implementations_by_config_file(&path.unwrap()) failed. path: {:?}; error: {:?}", &path, e),
                            };
                        }
                        Err(e) => panic!("glob() failed with error: {:?}", e),
                    }
                }
            }
            Err(e) => panic!("io::get_tfb_dir() failed with error: {:?}", e),
        }
    }
}
