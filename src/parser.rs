use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use log::error;

use crate::{
    lex::{Lexer, Token},
    Library,
};

#[derive(Debug)]
pub struct Config {
    pub libraries: IndexMap<String, Library>,
}

pub fn read_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut lexer = Lexer::new(&lines);
    let mut libraries = IndexMap::new();
    let mut current_lib_name: Option<String> = None;
    let mut current_library: Option<Library> = None;

    loop {
        match lexer.next_token() {
            Token::LibraryName(lib_name) => {
                // save previous library if it exists
                if let Some(lib) = current_library.take() {
                    match current_lib_name.take() {
                        None => {
                            error!("Library name empty!");
                        }
                        Some(name) => {
                            libraries.insert(name, lib);
                        }
                    }
                };

                // start new library
                current_lib_name = Some(lib_name);
                current_library = Some(Library {
                    source: String::new(),
                    commit: String::new(),
                    variables: IndexMap::new(),
                    steps: IndexMap::new(),
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
            Token::Step(step_name) => {
                if let Some(ref mut lib) = current_library {
                    let mut commands = Vec::new();
                    while let Some(command) = lexer.next_command() {
                        let processed_command = replace_placeholders(&command, &lib.variables)
                            .replace("$source", lib.source.as_str())
                            .replace("$commit", lib.commit.as_str());
                        commands.push(processed_command);
                    }

                    lib.steps.insert(step_name, Some(commands));
                }
            }
            Token::EndOfConfig => break,
            Token::EndOfFile => break,
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

fn replace_placeholders(command: &str, variables: &IndexMap<String, String>) -> String {
    let mut result = command.to_string();
    for (key, value) in variables {
        result = result.replace(&format!("${}", key), value);
    }
    result
}
