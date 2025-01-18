use clap::Parser;
use indexmap::IndexMap;
use ishell::IShell;
use log::{info, warn};
use parser::read_config;
use std::env;
use std::path::Path;

mod lex;
mod parser;

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

fn run_steps(steps: &IndexMap<String, Option<Vec<String>>>, run_dir: &Path) {
    for step in steps {
        info!("Running \"{}\" step", step.0);
        if let Some(commands) = step.1 {
            let shell = IShell::new(run_dir.to_str());
            for command in commands {
                let _ = shell.run_command(command);
            }
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
    let run_path = Path::new(&args.working_path);

    match read_config(project_path) {
        Ok(config) => {
            for (_lib_name, library) in &config.libraries {
                info!("Library: {_lib_name}");
                run_steps(&library.steps, &run_path);
            }
        }
        Err(err) => {
            println!("Error when reading config {project_path_str}: {}", err);
        }
    };
}
