use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use strum::{AsRefStr, EnumString};

#[derive(Parser, Debug)]
#[command(
    version, 
    about = concat!(
        env!("CARGO_PKG_DESCRIPTION"),
        "\n\nRepository: ",
        env!("CARGO_PKG_REPOSITORY")
    ),
    long_about = None,
)]
pub struct Args {
    /// Path to liminecli configuration path.
    #[arg(short, long, default_value = "/etc/liminecli.toml")]
    pub config_path: PathBuf,

    /// Use the default configuration for liminecli
    /// even if there is a configuration file avalible.
    #[arg(short = 'D', long, default_value_t = false)]
    pub default_configuration: bool,

    /// Path to vfat device to install limine to
    /// and where to write limine configuration.
    /// (e.g. /dev/sda1). This will update the in-memory
    /// config (generate-cli-config will write it to disk)
    ///
    ///
    /// If this flag is not set, liminecli will
    /// look for a boot partition on the same device
    /// as the root partition. If there is a single
    /// boot partition, it will use that.
    #[arg(short, long)]
    pub limine_device_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

impl Args {
    pub fn get_args() -> Self {
        Self::parse()
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install limine bootloader to boot partition and
    /// generate limine config.
    Install(InstallArgs),
    /// Generate limine config.
    Update,
    /// Write the current limine-cli toml configuration to file.
    /// If there is already a valid configutaion in the path,
    /// this will format. If there is no config, this will
    /// generate a default configuration.
    GenerateCliConfig,
    /// Print the liminecli config being used.
    PrintConfig,
    /// Show information that will be used by limine-cli
    Discover,
}

#[derive(Debug, Clone, Copy, ValueEnum, EnumString, AsRefStr)]
pub enum InstallationType {
    /// Install firmare into
    /// $ESP/EFI/LIMINE/limine_x64.efi.
    UefiEntry,
    /// Install firmare into
    /// $ESP/EFI/BOOT/BOOTX64.EFI.
    UefiDefault,
    /// Install limine as both
    /// a uefi entry and as efi default.
    Both,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct InstallArgs {
    /// Where to install limine EFI firmware to
    #[arg(short, long, value_enum, default_value_t=InstallationType::UefiEntry)]
    pub install_type: InstallationType,

    /// Limine configuration path to install to relitive
    /// to mounted ESP device.
    #[arg(short, long, default_value = "/limine.conf")]
    pub limine_config: PathBuf,
}
