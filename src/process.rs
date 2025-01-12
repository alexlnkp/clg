use std::io::BufRead;
use std::io::BufReader;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use log::{info, warn};

pub fn run_command(command: &str) -> Output {
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

pub fn collect_output(buffer: &Arc<Mutex<Vec<String>>>) -> Vec<u8> {
    buffer.lock().unwrap().join("\n").into_bytes()
}
