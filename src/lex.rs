use log::warn;

#[derive(Debug, PartialEq)]
pub enum Token {
    LibraryName(String),
    KeyValue(String, String),
    // Command(String), // we grab commands straight from steps, no need to have lex type for them?
    Comment(String),
    Step(StepType),
    EndOfFile,
    EndOfConfig,
}

#[derive(Debug, PartialEq)]
pub enum StepType {
    Preparation,
    Build,
    PostBuild,
}

pub struct Lexer<'a> {
    lines: &'a [&'a str],
    current_line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(lines: &'a [&'a str]) -> Self {
        Lexer {
            lines,
            current_line: 0,
        }
    }

    pub fn next_token(&mut self) -> Token {
        while self.current_line < self.lines.len() {
            let line = self.lines[self.current_line].trim();

            if line.is_empty() {
                self.current_line += 1;
                continue;
            }

            if line.starts_with('#') {
                if line.starts_with("#preparation:") {
                    self.current_line += 1;
                    return Token::Step(StepType::Preparation);
                } else if line.starts_with("#build:") {
                    self.current_line += 1;
                    return Token::Step(StepType::Build);
                } else if line.starts_with("#post_build:") {
                    self.current_line += 1;
                    return Token::Step(StepType::PostBuild);
                } else {
                    warn!("Unknown step \"{line}\"! expected one of {{preparation, build, post_build}}!");
                    self.current_line += 1; // ?
                    return Token::Comment(line[1..].trim().to_string());
                }
            }

            if line.starts_with('[') && line.ends_with(']') {
                let lib_name = line[1..line.len() - 1].to_string();
                self.current_line += 1;
                return Token::LibraryName(lib_name);
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().trim_matches('"').to_string();
                self.current_line += 1;
                return Token::KeyValue(key, value);
            }

            if line == "%end" {
                self.current_line += 1;
                return Token::EndOfConfig;
            }

            self.current_line += 1;
        }

        Token::EndOfFile
    }

    pub fn next_command(&mut self) -> Option<String> {
        while self.current_line < self.lines.len() {
            let line = self.lines[self.current_line].trim();

            if line.is_empty() {
                self.current_line += 1;
                continue;
            }

            if line.starts_with('#') || line.starts_with('[') || line == "%end" {
                return None; // end of the commands
            }

            self.current_line += 1;
            return Some(line.to_string());
        }

        None
    }
}
