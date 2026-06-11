use crate::Result;
use crate::fs::boot::EspRoot;
use crate::fs::device::BlockDevice;
use crate::{
    config::Config,
    fs::{boot::LiminePaths, efi::EfiBootOrder},
};
use owo_colors::OwoColorize;
use tabled::{Table, Tabled};

pub fn run(_config: &Config) -> Result<()> {
    print_efi_boot_order();
    print_target_limine_install_dir()?;
    Ok(())
}

fn option_or_empty_string<T: ToString>(item: &Option<T>) -> String {
    item.as_ref().map(|x| x.to_string()).unwrap_or_default()
}
fn print_efi_boot_order() {
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
            Ok(boot_device) => {
                let part_uuid = boot_device.harddrive.map(|x| x.partuuid).flatten();
                let (part_uuid, device_path) = match part_uuid {
                    Some(uuid) => {
                        let device = match BlockDevice::from_part_uuid(uuid) {
                            Ok(device) => device.path.display().to_string(),
                            Err(e) => {
                                format!("{} {}", "Error boot block device: ".bright_red(), e.red())
                            }
                        };

                        (Some(uuid.blue().to_string()), Some(device))
                    }
                    None => (
                        Some("no part uuid".to_string()),
                        Some("no device".to_string()),
                    ),
                };
                table.push(BootOrderItem {
                    item: item.0.purple().to_string(),
                    path: boot_device.path.display().magenta().to_string(),
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
    println!();
}

fn print_target_limine_install_dir() -> Result<()> {
    #[derive(Tabled)]
    struct LimineInstallDirs {
        #[tabled(rename = "Item")]
        pub item: String,

        #[tabled(rename = "Value")]
        pub value: String,
    }
    println!("{}", "### Current limine paths being used ###".green());
    println!();
    match LiminePaths::discover() {
        Ok(paths) => {
            let items = [
                LimineInstallDirs {
                    item: "esp location".blue().to_string(),
                    value: match paths.esp_root {
                        EspRoot::Path(p) => p.display().to_string(),
                        EspRoot::Mounted(ref m) => m.device.display().to_string(),
                    }
                    .cyan()
                    .to_string(),
                },
                LimineInstallDirs {
                    item: "config path".blue().to_string(),
                    value: paths.config_path.display().cyan().to_string(),
                },
                LimineInstallDirs {
                    item: "efi firmware path".blue().to_string(),
                    value: paths.efi_path.display().cyan().to_string(),
                },
            ];

            println!("{}", Table::new(&items));
        }
        Err(e) => {
            println!(
                "{} {}",
                "Error getting limine install paths: ".bright_red(),
                e.red()
            );
        }
    }
    println!();
    Ok(())
}
