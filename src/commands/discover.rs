use std::path::{Path, PathBuf};

use crate::Result;
use crate::fs::boot::EspRoot;
use crate::fs::device::BlockDevice;
use crate::fs::efi::{EfiBootDevice, EfiBootOrderItem};
use crate::fs::mount::FSType;
use crate::{
    config::Config,
    fs::{boot::LiminePaths, efi::EfiBootOrder},
};
use owo_colors::OwoColorize;
use tabled::{Table, Tabled};

pub fn run(config: &Config) -> Result<()> {
    let paths = print_target_limine_install_dir(config)?;
    print_efi_boot_order(paths.as_ref());
    Ok(())
}

fn print_efi_boot_order(paths: Option<&LiminePaths>) {
    #[derive(Tabled)]
    struct BootOrderItem {
        #[tabled(rename = "Boot Num")]
        pub item: String,

        #[tabled(rename = "Efi Path")]
        pub path: String,

        #[tabled(rename = "Part Uuid")]
        pub device_part_uuid: String,

        #[tabled(rename = "Device path")]
        pub device_path: String,
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

    fn boot_order_items_for_boot_device(
        limine_paths: Option<&LiminePaths>,
        item: &EfiBootOrderItem,
    ) -> Result<Option<(String, String, String, bool)>> {
        let boot_device = item.try_read_boot_device()?;
        let mut current_boot_device = false;
        let boot_device_part_uuid =
            match boot_device.harddrive.as_ref().map(|x| x.partuuid).flatten() {
                Some(uuid) => uuid,
                None => {
                    return Ok(Some((
                        "no part uuid".to_string(),
                        "no device".to_string(),
                        "no path".to_string(),
                        false,
                    )));
                }
            };

        let boot_block_device = BlockDevice::from_part_uuid(boot_device_part_uuid);
        if let Ok(ref device) = boot_block_device
            && is_limine_boot_entry(&boot_device, device, limine_paths)
        {
            current_boot_device = true;
        }
        let device_display = match boot_block_device {
            Ok(device) => device.path.display().to_string(),
            Err(e) => {
                format!("{} {}", "Error boot block device: ".bright_red(), e.red())
            }
        };

        Ok(Some((
            boot_device_part_uuid.blue().to_string(),
            device_display,
            boot_device.firmware_path.display().magenta().to_string(),
            current_boot_device,
        )))
    }

    let mut table = Vec::new();
    for item in boot_order.iter() {
        if let Some((uuid_display, device_display, path_display, current_boot_device)) =
            boot_order_items_for_boot_device(paths, &item)
                .ok()
                .flatten()
        {
            table.push(BootOrderItem {
                item: if current_boot_device {
                    format!("*{}*", item.0.green())
                } else {
                    item.0.purple().to_string()
                },
                path: path_display,
                device_part_uuid: uuid_display,
                device_path: device_display,
            });
        } else {
            table.push(BootOrderItem {
                item: item.0.purple().to_string(),
                path: "Not a path boot item".to_string(),
                device_part_uuid: String::new(),
                device_path: String::new(),
            });
        }
    }

    println!("{}", Table::new(&table));
    println!("* = current limine boot entry");
    println!();
}

fn print_target_limine_install_dir(config: &Config) -> Result<Option<LiminePaths>> {
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
    let limine_paths = if let Some(limine_device) = &config.limemine_block_device {
        match LiminePaths::from_device(FSType::VFat, limine_device, false).transpose() {
            Some(paths) => paths,
            None => {
                println!(
                    "{} {}",
                    "Cannot use specified block device as lmine root: ".bright_red(),
                    limine_device.display()
                );

                return Ok(None);
            }
        }
    } else {
        LiminePaths::discover()
    };

    let paths = match limine_paths {
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
