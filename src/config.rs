//! Button mapping loaded from TOML.

use std::fmt;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

use crate::protocol::Buttons;

/// Built into the binary at compile time.
const DEFAULT_TOML: &str = include_str!("default_config.toml");

/// The file exactly as written: button *names*.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    next: Vec<String>,
    prev: Vec<String>,
}

/// The validated mapping: button *bits*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub next: Buttons,
    pub prev: Buttons,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Toml(toml::de::Error),
    UnknownButton(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "cannot read config: {e}"),
            ConfigError::Toml(e) => write!(f, "invalid config: {e}"),
            ConfigError::UnknownButton(name) => write!(f, "unknown button {name:?}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::Toml(e)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config::parse(DEFAULT_TOML).expect("built-in default config is valid")
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, ConfigError> {
        let raw: RawConfig = toml::from_str(text)?;
        Ok(Config {
            next: buttons_from_names(&raw.next)?,
            prev: buttons_from_names(&raw.prev)?,
        })
    }

    /// `~/.config/ukyo-clicker/config.toml`, if `$HOME` is set.
    pub fn path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".config/ukyo-clicker/config.toml"))
    }

    /// Load the user's config; fall back to the default if there is none.
    pub fn load() -> Result<Config, ConfigError> {
        let Some(path) = Config::path() else {
            return Ok(Config::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }
}

fn buttons_from_names(names: &[String]) -> Result<Buttons, ConfigError> {
    let mut all = Buttons::default();
    for name in names {
        let button =
            button_by_name(name).ok_or_else(|| ConfigError::UnknownButton(name.clone()))?;
        all = all | button;
    }
    Ok(all)
}

fn button_by_name(name: &str) -> Option<Buttons> {
    let button = match name.to_ascii_uppercase().as_str() {
        "A" => Buttons::A,
        "B" => Buttons::B,
        "X" => Buttons::X,
        "Y" => Buttons::Y,
        "R" => Buttons::R,
        "ZR" => Buttons::ZR,
        "SL" => Buttons::SL,
        "SR" => Buttons::SR,
        "PLUS" => Buttons::PLUS,
        "HOME" => Buttons::HOME,
        "RSTICK" => Buttons::RSTICK,
        _ => return None,
    };
    Some(button)
}
