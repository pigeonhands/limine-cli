pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod fs;

use cli::{Args, Command};
pub use error::{LimeineCliError, Result};

use crate::{config::Config, fs::mount::Mount};

fn main() -> Result<()> {
    let args = Args::get_args();

    let config = if args.default_configuration {
        Default::default()
    } else {
        Config::from_file_or_default(&args.config_path)?
    };

    for mount in Mount::get_mounts().unwrap().iter() {
        println!("{:#?}", mount);
    }

    match args.command {
        Command::GenerateCliConfig => {
            config.write_to(&args.config_path)?;
        }
        Command::PrintConfig => {
            println!("{:#?}", config);
        }
        Command::Install(subargs) => {
            commands::install::run(&config, &subargs)?;
        }
        Command::Update => {
            commands::update::run(&config)?;
        }
    }
    Ok(())
}
