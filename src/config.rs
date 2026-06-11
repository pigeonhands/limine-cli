use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::{LimeineCliError, Result};

#[derive(SmartDefault, Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub limine_config_location: Option<PathBuf>,
    pub limemine_block_device: Option<PathBuf>,
    pub limine: LimineConfig,
}

impl Config {
    pub fn parse(config: &str) -> Result<Self> {
        toml::from_str(config).map_err(LimeineCliError::InvalidConfig)
    }

    pub fn from_file_or_default(path: &Path) -> Result<Self> {
        let config_file = match fs::read_to_string(path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(LimeineCliError::NoPermission {
                    path: path.into(),
                    error: e,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(e.into());
            }
        };
        Ok(config_file
            .map(|s| Self::parse(&s))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn as_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config should always be valid")
    }
    pub fn write_to(&self, path: &Path) -> Result<()> {
        let config_str = self.as_toml();

        fs::write(path, config_str)
            .map_err(|e| LimeineCliError::FailedToWriteConfig(path.into(), e))?;
        Ok(())
    }
}

#[derive(SmartDefault, Debug, Clone, Serialize, Deserialize)]
pub struct LimineConfig {
    #[serde(with = "humantime_serde")]
    #[default(Duration::from_secs(3))]
    pub timeout: Duration,

    /// CMDLINE args that apply to all entries
    #[default(vec!["nowatchdog".into(), "rw".into()])]
    pub cmd_args: Vec<String>,

    /// Enirties are matched in the order in thay they
    /// are defined in the configuration, and displayed
    /// in priority orer
    #[default(vec![
        EntryConfiguration {
            apply_to: Regex::new("$Snapshots^").unwrap(),
            priority: 10,
            ..Default::default()
        },
        EntryConfiguration {
            apply_to: Regex::new("fallback^").unwrap(),
            priority: 9,
            ..Default::default()
        },
        EntryConfiguration {
            apply_to: Regex::new("lts$").unwrap(),
            priority: 2,
            ..Default::default()
        },
        EntryConfiguration {
            apply_to: Regex::new(".*").unwrap(),
            priority: 1,
            ..Default::default()
        }
    ])]
    pub entries: Vec<EntryConfiguration>,
}

#[derive(SmartDefault, Debug, Clone, Serialize, Deserialize)]
pub struct EntryConfiguration {
    #[serde(with = "serde_regex")]
    #[default(Regex::new(".*").unwrap())]
    pub apply_to: Regex,

    /// Priority to show in limine boot menu
    #[default(1)]
    pub priority: usize,

    /// CMDLINE args that apply to matching entries
    #[default(None)]
    pub cmd_args: Option<Vec<String>>,

    pub limine_submenu: String,
}

mod serde_regex {
    use super::*;
    use core::result::Result;
    use regex::Regex;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(re: &Regex, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(re.as_str())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Regex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        Regex::new(&s).map_err(serde::de::Error::custom)
    }
}
