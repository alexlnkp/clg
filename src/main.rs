use clap::Parser;
use lex::Lexer;
use lex::StepType;
use lex::Token;
use log::{info, warn};
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::{env, fs, thread};

mod lex;

// C(++) library gatherer
// Pardon the messy code, this is like my 2nd rust project

#[derive(Debug)]
struct Library {
    source: String,
    commit: String,
    preparation: Option<Vec<String>>,
    build: Option<Vec<String>>,
    post_build: Option<Vec<String>>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("."))]
    path: String, // Path to the folder with vendor.clg

    #[arg(short, long, default_value_t = String::from("."))]
    working_path: String, // CD here before doing anything (WIP)
}

#[derive(Debug)]
struct Config {
    libraries: HashMap<String, Library>,
}

fn read_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut lexer = Lexer::new(&lines);
    let mut libraries = HashMap::new();
    let mut current_lib: Option<String> = None;
    let mut current_library: Option<Library> = None;

    loop {
        match lexer.next_token() {
            Token::LibraryName(lib_name) => {
                // save previous library if it exists
                if let Some(lib) = current_library.take() {
                    libraries.insert(current_lib.take().unwrap(), lib);
                }

                // start new library
                current_lib = Some(lib_name);
                current_library = Some(Library {
                    source: String::new(),
                    commit: String::new(),
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
                        _ => {}
                    }
                }
            }
            Token::Step(step_type) => {
                if let Some(ref mut lib) = current_library {
                    let mut commands = Vec::new();
                    // collect commands until hit a new step or end of config
                    while let Some(command) = lexer.next_command() {
                        commands.push(command);
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
                if let Some(lib) = current_library.take() {
                    libraries.insert(current_lib.take().unwrap(), lib);
                }
                break;
            }
            Token::EndOfFile => {
                break;
            }
            _ => {}
        }
    }

    // finalize last library if it exists
    if let Some(lib_name) = current_lib {
        if let Some(lib) = current_library {
            libraries.insert(lib_name, lib);
        }
    }

    Ok(Config { libraries })
}

fn run_command(command: &str) -> Output {
    info!("Running: `{}`", command);

    let mut process = spawn_process(command).expect("failed to execute process");

    let (stdout_buffer, stderr_buffer) = (
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    );

    let (stdout_handle, stderr_handle) = spawn_output_threads(
        process.stdout.take(),
        process.stderr.take(),
        &stdout_buffer,
        &stderr_buffer,
    );

    let status = process.wait().expect("Command wasn't running");

    stdout_handle.join().expect("Failed to join stdout thread");
    stderr_handle.join().expect("Failed to join stderr thread");

    let stdout = collect_output(&stdout_buffer);
    let stderr = collect_output(&stderr_buffer);

    Output {
        status,
        stdout,
        stderr,
    }
}

fn spawn_process(command: &str) -> std::io::Result<std::process::Child> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

fn spawn_output_threads(
    stdout: Option<std::process::ChildStdout>,
    stderr: Option<std::process::ChildStderr>,
    stdout_buffer: &Arc<Mutex<Vec<String>>>,
    stderr_buffer: &Arc<Mutex<Vec<String>>>,
) -> (thread::JoinHandle<()>, thread::JoinHandle<()>) {
    let stdout_handle = thread::spawn({
        let stdout_buffer_clone = Arc::clone(stdout_buffer);
        move || {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        info!("{}", line);
                        stdout_buffer_clone.lock().unwrap().push(line);
                    }
                }
            }
        }
    });

    let stderr_handle = thread::spawn({
        let stderr_buffer_clone = Arc::clone(stderr_buffer);
        move || {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        warn!("{}", line);
                        stderr_buffer_clone.lock().unwrap().push(line);
                    }
                }
            }
        }
    });

    (stdout_handle, stderr_handle)
}

fn collect_output(buffer: &Arc<Mutex<Vec<String>>>) -> Vec<u8> {
    buffer.lock().unwrap().join("\n").into_bytes()
}

fn run_step(step: &Option<Vec<String>>, source: &String, commit: &String) {
    if let Some(commands) = step {
        let cmd = commands
            .iter()
            .filter(|comm| !comm.is_empty())
            .map(|comm| format!("{}; ", comm))
            .collect::<String>();

        let cmt = cmd
            .replace("$source", &**source)
            .replace("$commit", &**commit);

        let _ = run_command(&cmt);
    }
}

fn prepare_library(lib: &Library) {
    info!("Running preparation step");
    run_step(&lib.preparation, &lib.source, &lib.commit);

    info!("Running build step");
    run_step(&lib.build, &lib.source, &lib.commit);

    info!("Running post-build step");
    run_step(&lib.post_build, &lib.source, &lib.commit);
}

fn main() {
    // a hack to set default log level to trace
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trace")
    }
    env_logger::init();

    let args = Args::parse();
    let project_path_str = format!("{}/vendor.clg", args.path.as_str());
    let project_path = Path::new(&project_path_str);

    let ret = read_config(project_path);

    match ret {
        Ok(config) => {
            // issue is that we need to keep track of the
            let pre_cd = format!("cd {}", args.working_path);
            run_command(pre_cd.as_str());

            for (_lib_name, library) in &config.libraries {
                prepare_library(library);
            }
        }
        Err(err) => {
            println!("Error when reading config {project_path_str}: {}", err);
        }
    };
}
