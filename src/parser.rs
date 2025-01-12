use std::{collections::HashMap, fs, path::Path};

use log::error;

use crate::{
    lex::{Lexer, StepType, Token},
    Library,
};

#[derive(Debug)]
pub struct Config {
    pub libraries: HashMap<String, Library>,
}

macro_rules! insert_library {
    ($libraries:ident, $current_library:ident, $current_lib_name:ident) => {
        if let Some(lib) = $current_library.take() {
            match $current_lib_name.take() {
                None => {
                    error!("Library name empty!");
                }
                Some(name) => {
                    $libraries.insert(name, lib);
                }
            }
        }
    };
}

pub fn read_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut lexer = Lexer::new(&lines);
    let mut libraries = HashMap::new();
    let mut current_lib_name: Option<String> = None;
    let mut current_library: Option<Library> = None;

    loop {
        match lexer.next_token() {
            Token::LibraryName(lib_name) => {
                // save previous library if it exists
                insert_library!(libraries, current_library, current_lib_name);

                // start new library
                current_lib_name = Some(lib_name);
                current_library = Some(Library {
                    source: String::new(),
                    commit: String::new(),
                    variables: HashMap::new(),
                    preparation: None,
                    build: None,
                    post_build: None,
                });
            }
            Token::KeyValue(key, value) => {
                if let Some(ref mut lib) = current_library {
                    match key.as_str() {
                        "source" => lib.source = value,
                        "commit" => lib.commit = value,
                        _ => {
                            lib.variables.insert(key, value);
                        }
                    }
                }
            }
            Token::Step(step_type) => {
                if let Some(ref mut lib) = current_library {
                    let mut commands = Vec::new();
                    // Collect commands until we hit a new step or end of config
                    while let Some(command) = lexer.next_command() {
                        // Replace placeholders in the command
                        let processed_command = replace_placeholders(&command, &lib.variables)
                            .replace("$source", lib.source.as_str())
                            .replace("$commit", lib.commit.as_str());
                        commands.push(processed_command);
                    }
                    match step_type {
                        StepType::Preparation => lib.preparation = Some(commands),
                        StepType::Build => lib.build = Some(commands),
                        StepType::PostBuild => lib.post_build = Some(commands),
                    }
                }
            }
            Token::EndOfConfig => {
                // save last library if it exists
                insert_library!(libraries, current_library, current_lib_name);
                break;
            }
            Token::EndOfFile => {
                break;
            }
            _ => {}
        }
    }

    // finalize last library if it exists
    if let Some(lib_name) = current_lib_name {
        if let Some(lib) = current_library {
            libraries.insert(lib_name, lib);
        }
    }

    Ok(Config { libraries })
}

fn replace_placeholders(command: &str, variables: &HashMap<String, String>) -> String {
    let mut result = command.to_string();
    for (key, value) in variables {
        result = result.replace(&format!("${}", key), value);
    }
    result
}
