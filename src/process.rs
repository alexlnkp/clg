use std::io::BufRead;
use std::io::BufReader;
use std::process::ExitStatus;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use log::error;
use log::{info, warn};

// Leech output from stdout/stderr while also storing the resulting output
macro_rules! leech_output {
    ($out:ident, $out_buf:ident, $log_method:ident) => {
        thread::spawn({
            let output_buffer_clone = Arc::clone($out_buf);
            move || {
                if let Some(output) = $out {
                    let reader = BufReader::new(output);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            $log_method!("{}", line);
                            match output_buffer_clone.lock() {
                                Err(e) => {
                                    error!("Failed to lock {} buffer! {}", stringify!($out), e);
                                    return;
                                }
                                Ok(mut vec) => {
                                    vec.push(line);
                                }
                            }
                        }
                    }
                }
            }
        })
    };
}

pub fn run_command(command: &str) -> Output {
    info!("Running: `{}`", command);

    let child_process = spawn_process(command);
    match child_process {
        Ok(mut process) => {
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

            let status = match process.wait() {
                Ok(status) => status,
                Err(e) => {
                    error!("Failed to wait for process: {}", e);
                    ExitStatus::default()
                }
            };

            let _ = stdout_handle.join().map_err(|e| {
                error!("Failed to join stdout thread: {:?}", e);
            });
            let _ = stderr_handle.join().map_err(|e| {
                error!("Failed to join stderr thread: {:?}", e);
            });

            let stdout = collect_output(&stdout_buffer);
            let stderr = collect_output(&stderr_buffer);

            Output {
                status,
                stdout,
                stderr,
            }
        }
        Err(e) => {
            error!("Couldn't spawn child process! {}", e);

            let status = ExitStatus::default();
            let stdout = Vec::new();
            let stderr = Vec::new();

            Output {
                status,
                stdout,
                stderr,
            }
        }
    }
}

pub fn spawn_process(command: &str) -> std::io::Result<std::process::Child> {
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

pub fn spawn_output_threads(
    stdout: Option<std::process::ChildStdout>,
    stderr: Option<std::process::ChildStderr>,
    stdout_buffer: &Arc<Mutex<Vec<String>>>,
    stderr_buffer: &Arc<Mutex<Vec<String>>>,
) -> (thread::JoinHandle<()>, thread::JoinHandle<()>) {
    let stdout_handle = leech_output!(stdout, stdout_buffer, info);
    let stderr_handle = leech_output!(stderr, stderr_buffer, warn);

    (stdout_handle, stderr_handle)
}

pub fn collect_output(buffer: &Arc<Mutex<Vec<String>>>) -> Vec<u8> {
    match buffer.lock() {
        Ok(buffer) => buffer.join("\n").into_bytes(),
        Err(err) => {
            error!("Couldn't lock buffer! {}", err);
            // Need to return SOMETHING here.
            Vec::new()
        }
    }
}
