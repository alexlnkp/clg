use clap::Parser;
use indexmap::IndexMap;
use ishell::IShell;
use log::{info, warn};
use parser::read_config;
use std::env;
use std::path::{Path, PathBuf};

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
    #[arg(short, long, default_value_t = String::from("."), help = "Path to the folder with vendor.clg")]
    path: String, // Path to the folder with vendor.clg

    #[arg(short, long, default_value_t = String::from(""), help = "CD here before running any command")]
    working_path: String, // CD here before doing anything (WIP)

    #[arg(
        short,
        long,
        default_value_t = true,
        help = "Continues gathering current library, even if any of its commands fail"
    )]
    continue_on_fail: bool,
}

fn gather_library(library: &Library, shell: IShell, continue_on_fail: bool) {
    'outer: for step in &library.steps {
        info!("Running \"{}\" step", step.0);
        if let Some(commands) = step.1 {
            for command in commands {
                let output = shell.run_command(command);

                if !output.status.success() && !continue_on_fail {
                    break 'outer;
                }
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

    let project_path = format!("{}/vendor.clg", args.path.as_str());
    let project_path = Path::new(&project_path);

    let run_path_buf: PathBuf = if args.working_path.is_empty() {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&args.working_path)
    };

    let run_path: &Path = run_path_buf.as_path();

    match read_config(project_path) {
        Ok(config) => {
            for (_lib_name, library) in &config.libraries {
                info!("Library: {_lib_name}");
                let shell = IShell::new(run_path.to_str());
                gather_library(&library, shell, args.continue_on_fail);
            }
        }
        Err(err) => {
            println!(
                "Error when reading config {}: {}",
                project_path.to_str().unwrap_or("./vendor.clg"),
                err
            );
        }
    };
}
