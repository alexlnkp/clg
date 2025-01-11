use clap::Parser;
use log::{warn, info};
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::{env, fs, thread};

// C(++) library gatherer

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

macro_rules! get_commands {
    ($lib:expr, $lines:expr, $i:expr, $pool:ident) => {
        $i += 1;
        let commands = parse_commands(&$lines, &mut $i);
        $lib.$pool = Some(commands);
        continue;
    };
}

fn parse_commands(lines: &[&str], i: &mut usize) -> Vec<String> {
    let mut commands = Vec::new();
    while *i < lines.len() {
        let line = lines[*i];
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('#') {
            break;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            commands.push(trimmed_line.to_string());
        } else {
            break;
        }
        *i += 1;
    }

    commands
}

fn read_config(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut libraries = HashMap::new();
    let mut current_lib: Option<String> = None;
    let mut current_library: Option<Library> = None;

    let lines: Vec<&str> = content.lines().collect();

    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with('[') && line.ends_with(']') {
            if let Some(lib_name) = current_lib.take() {
                if let Some(lib) = current_library.take() {
                    libraries.insert(lib_name, lib);
                }
            }

            current_lib = Some(line[1..line.len() - 1].to_string());
            current_library = Some(Library {
                source: String::new(),
                commit: String::new(),
                preparation: None,
                build: None,
                post_build: None,
            });
        } else if let Some(ref mut lib) = current_library {
            if line.starts_with("source") {
                lib.source = line
                    .split('=')
                    .nth(1)
                    .unwrap()
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("commit") {
                lib.commit = line
                    .split('=')
                    .nth(1)
                    .unwrap()
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("#preparation:") {
                get_commands!(lib, lines, i, preparation);
            } else if line.starts_with("#build:") {
                get_commands!(lib, lines, i, build);
            } else if line.starts_with("#post_build:") {
                get_commands!(lib, lines, i, post_build);
            }
        }

        if line.starts_with("%end") {
            break;
        }

        i += 1;
    }

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
