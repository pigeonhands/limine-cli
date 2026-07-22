pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod fs;

use cli::{Args, Command};
use config::Config;

pub use error::{LimeineCliError, Result};

use crate::fs::LockFile;

fn main() -> Result<()> {
    let args = Args::get_args();

    let _lock = LockFile::aquire()?;

    let config = {
        let mut config = if args.default_configuration {
            Default::default()
        } else {
            Config::from_file_or_default(&args.config_path)?
        };
        if let Some(device_path) = args.limine_device_path {
            config.limemine_block_device = Some(device_path.clone());
        }
        config
    };

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
        Command::Discover => {
            commands::discover::run(&config)?;
        }
    }
    Ok(())
}
