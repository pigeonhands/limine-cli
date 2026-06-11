use std::fmt::format;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::fs::boot::EspRoot;
use crate::fs::device::BlockDevice;
use crate::fs::efi::EfiBootDevice;
use crate::{
    config::Config,
    fs::{boot::LiminePaths, efi::EfiBootOrder},
};
use owo_colors::OwoColorize;
use tabled::{Table, Tabled};

pub fn run(_config: &Config) -> Result<()> {
    let paths = print_target_limine_install_dir()?;
    print_efi_boot_order(paths.as_ref());
    Ok(())
}

fn option_or_empty_string<T: ToString>(item: &Option<T>) -> String {
    item.as_ref().map(|x| x.to_string()).unwrap_or_default()
}
fn print_efi_boot_order(paths: Option<&LiminePaths>) {
    #[derive(Tabled)]
    struct BootOrderItem {
        #[tabled(rename = "Boot Num")]
        pub item: String,

        #[tabled(rename = "Efi Path")]
        pub path: String,

        #[tabled(rename = "Part Uuid", display("option_or_empty_string"))]
        pub device_part_uuid: Option<String>,

        #[tabled(rename = "Device path", display("option_or_empty_string"))]
        pub device_path: Option<String>,
    }

    println!("{}", "### Efi boot Items (by boot order) ###".green());
    println!();

    let boot_order = match EfiBootOrder::read_boot_order() {
        Ok(b) => b,
        Err(e) => {
            println!(
                "{} {}",
                "Error getting efi boot order: ".bright_red(),
                e.red()
            );
            return;
        }
    };

    let mut table = Vec::new();
    for item in boot_order.iter() {
        match item.try_read_boot_device() {
            Ok(ref boot_device) => {
                let mut current_boot_device = false;
                let part_uuid = boot_device.harddrive.as_ref().map(|x| x.partuuid).flatten();
                let (part_uuid, device_path) = match part_uuid {
                    Some(uuid) => {
                        let boot_block_device = BlockDevice::from_part_uuid(uuid);

                        if let Ok(ref device) = boot_block_device
                            && is_limine_boot_entry(&boot_device, device, paths)
                        {
                            current_boot_device = true;
                        }
                        let device_display = match boot_block_device {
                            Ok(device) => device.path.display().to_string(),
                            Err(e) => {
                                format!("{} {}", "Error boot block device: ".bright_red(), e.red())
                            }
                        };
                        (Some(uuid.blue().to_string()), Some(device_display))
                    }
                    None => (
                        Some("no part uuid".to_string()),
                        Some("no device".to_string()),
                    ),
                };
                table.push(BootOrderItem {
                    item: if current_boot_device {
                        format!("{}*", item.0.green())
                    } else {
                        item.0.purple().to_string()
                    },
                    path: boot_device.firmware_path.display().magenta().to_string(),
                    device_part_uuid: match part_uuid {
                        Some(uuid) => Some(uuid.blue().to_string()),
                        None => Some("no part uuid".to_string()),
                    },
                    device_path,
                });
            }
            Err(_) => {
                table.push(BootOrderItem {
                    item: item.0.purple().to_string(),
                    path: "Not a path boot item".to_string(),
                    device_part_uuid: None,
                    device_path: None,
                });
            }
        }
    }

    println!("{}", Table::new(&table));
    println!("* = current limine boot entry");
    println!();
}

fn print_target_limine_install_dir() -> Result<Option<LiminePaths>> {
    #[derive(Tabled)]
    struct LimineInstallDirs {
        #[tabled(rename = "Item")]
        pub item: String,

        #[tabled(rename = "Value")]
        pub value: String,

        #[tabled(rename = "Status")]
        pub status: String,
    }
    println!("{}", "### Current limine paths being used ###".green());
    println!();

    fn exists_status(path: &Path) -> String {
        if path.exists() {
            "exists".green().to_string()
        } else {
            "will be created".yellow().to_string()
        }
    }
    let paths = match LiminePaths::discover() {
        Ok(paths) => {
            let items = [
                LimineInstallDirs {
                    item: "esp location".blue().to_string(),
                    value: match paths.esp_root {
                        EspRoot::Path { ref device, .. } => device.display().to_string(),
                        EspRoot::Mounted(ref m) => m.device.display().to_string(),
                    }
                    .cyan()
                    .to_string(),
                    status: match paths.esp_root {
                        EspRoot::Path { ref mount_path, .. } => {
                            format!(
                                "{} ({})",
                                "mounted".green().to_string(),
                                mount_path.display()
                            )
                        }
                        EspRoot::Mounted(_) => "will be mounted".yellow().to_string(),
                    },
                },
                LimineInstallDirs {
                    item: "config path".blue().to_string(),
                    value: paths.config_path.display().cyan().to_string(),
                    status: exists_status(&paths.esp_root.as_ref().join(&paths.config_path)),
                },
                LimineInstallDirs {
                    item: "efi firmware path".blue().to_string(),
                    value: paths.efi_path.display().cyan().to_string(),
                    status: exists_status(&paths.esp_root.as_ref().join(&paths.efi_path)),
                },
            ];

            println!("{}", Table::new(&items));
            Some(paths)
        }
        Err(e) => {
            println!(
                "{} {}",
                "Error getting limine install paths: ".bright_red(),
                e.red()
            );
            None
        }
    };
    println!();
    Ok(paths)
}

fn paths_equal_case_insensitive(a: &Path, b: &Path) -> bool {
    let a_components: Vec<_> = a.components().collect();
    let b_components: Vec<_> = b.components().collect();

    if a_components.len() != b_components.len() {
        return false;
    }

    a_components.iter().zip(b_components.iter()).all(|(a, b)| {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    })
}
pub fn is_limine_boot_entry(
    efi_boot_device: &EfiBootDevice,
    efi_block_device: &BlockDevice,
    limine_paths: Option<&LiminePaths>,
) -> bool {
    let Some(limine_paths) = limine_paths else {
        return false;
    };

    let firmware_path_linux: PathBuf = efi_boot_device
        .firmware_path
        .to_string_lossy()
        .replace("\\", "/")
        .trim_start_matches("/")
        .into();

    let Ok(limine_esp_root_block_device) = limine_paths.esp_root.block_device() else {
        return false;
    };

    if efi_block_device.path != limine_esp_root_block_device.path {
        return false;
    }

    paths_equal_case_insensitive(&firmware_path_linux, &limine_paths.efi_path)
}
