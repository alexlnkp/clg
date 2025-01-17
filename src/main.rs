use crate::process::run_command;
use clap::Parser;
use indexmap::IndexMap;
use log::{info, warn};
use parser::read_config;
use std::env;
use std::path::Path;

mod lex;
mod parser;
mod process;

// C(++) library gatherer
// Pardon the messy code, this is like my 2nd rust project

#[derive(Debug)]
struct Library {
    source: String,
    commit: String,
    variables: IndexMap<String, String>,
    steps: IndexMap<String, Option<Vec<String>>>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = String::from("."))]
    path: String, // Path to the folder with vendor.clg

    #[arg(short, long, default_value_t = String::from("."))]
    working_path: String, // CD here before doing anything (WIP)
}

fn run_steps(steps: &IndexMap<String, Option<Vec<String>>>) {
    for step in steps {
        info!("Running \"{}\" step", step.0);
        if let Some(commands) = step.1 {
            let cmd = commands
                .iter()
                .filter(|comm| !comm.is_empty())
                .map(|comm| format!("{}; ", comm))
                .collect::<String>();

            let _ = run_command(&cmd);
        } else {
            warn!("No commands defined for step \"{}\"!", step.0);
        }
    }
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

    match read_config(project_path) {
        Ok(config) => {
            // issue is that we need to keep track of the dir we're in
            let pre_cd = format!("cd {}", args.working_path);
            run_command(pre_cd.as_str());

            for (_lib_name, library) in &config.libraries {
                info!("Library: {_lib_name}");
                run_steps(&library.steps);
            }
        }
        Err(err) => {
            println!("Error when reading config {project_path_str}: {}", err);
        }
    };
}
