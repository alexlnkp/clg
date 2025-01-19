use log::error;

#[derive(Debug, PartialEq)]
pub enum Token {
    LibraryName(String),
    KeyValue(String, String),
    Step(String),
    EndOfFile,
    EndOfConfig,
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

            if line.starts_with('#') && line.ends_with(':') {
                let step_name = line.strip_prefix('#').and_then(|s| s.strip_suffix(':'));

                self.current_line += 1;
                match step_name {
                    // step_name could be an empty string, and we don't want that, so if
                    // step_name IS an empty string - we should treat it as a None.
                    Some(name) if !name.is_empty() => {
                        return Token::Step(name.to_string());
                    }
                    _ => {
                        error!("Couldn't parse step name on line {}!", self.current_line);
                        error!("Causing issues: `{}'", self.lines[self.current_line - 1]);
                        error!("Did you forget to give the step a name?");
                        panic!(); // panicking might be overkill
                    }
                };
            }

            if line.starts_with("/*") && line.ends_with("*/") {
                self.current_line += 1;
                continue;
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

            if line.is_empty() || (line.starts_with("/*") && line.ends_with("*/")) {
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
