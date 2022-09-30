use crate::{
    data::{Driver, Id, Navigator},
    Result,
};
use directories::ProjectDirs;
use eyre::{bail, ensure, eyre, WrapErr};
use std::{
    convert::TryFrom,
    fs::{self, File},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

const APPLICATION: &str = env!("CARGO_PKG_NAME");
const OLD_CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.json");
const CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.gitdrive");

#[derive(Debug, Default)]
pub struct Config {
    pub navigators: Vec<Navigator>,
    pub drivers: Vec<Driver>,
}

pub fn load() -> Result<Config> {
    let file = config_file(Mode::Read)?;

    match &file {
        ConfigFile::New(path) => {
            let content = fs::read_to_string(path)?;
            load_from_cfg(content)
        }
        ConfigFile::Old(path) => {
            let content = fs::read_to_string(path)?;
            let cfg = load_from_json(content)
                .with_context(|| format!("Reading config data from {}", path.display()))?;
            store(&cfg)?;
            Ok(cfg)
        }
        ConfigFile::Missing => Ok(Config::default()),
    }
    .wrap_err_with(|| {
        format!(
            concat!(
                "The file `{}` could not be read.\n",
                "Please make sure that the config file is accessible and is properly formatted.",
            ),
            file
        )
    })
}

fn load_from_cfg(content: String) -> Result<Config> {
    fn read_nav(alias: &str, line: &str) -> Result<Navigator> {
        let co_author = co_authors::CoAuthor::try_from(line)?;
        Ok(Navigator {
            alias: Id(String::from(alias)),
            name: String::from(co_author.name),
            email: co_author.mail.map(String::from).unwrap_or_default(),
        })
    }

    let mut lines = content.lines().enumerate().map(|(ln, l)| (ln + 1, l));

    let (_, version) = lines
        .next()
        .ok_or_else(|| eyre!("The config file is empty, expected at least a version key."))?;
    let version = version
        .strip_prefix("version: ")
        .ok_or_else(|| eyre!("Expected `version: $version`, but got `{version}` in line 1."))?;
    ensure!(version == "1", "Unknown version: {}", version);

    let mut navigators = Vec::new();
    let mut drivers = Vec::new();

    while let Some((line_number, line)) = lines.next() {
        let (kind, alias) = line.split_once(": ").ok_or_else(|| {
            eyre!("Expected `$type: $alias`, but got `{line}` in line {line_number}.")
        })?;

        match kind {
            "navigator" => {
                let (_, nav) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected `Co-Authored-By: $name <$email>` in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let nav = read_nav(alias, nav)?;
                navigators.push(nav);
            }
            "driver" => {
                let (line_number, key) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected `key: $key` in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let key = match key.split_once(": ") {
                    Some(("key", key)) => Some(key).map(str::trim).filter(|k| !k.is_empty()),
                    Some(_) | None => {
                        bail!("Expected `key: $key` in line {line_number}, but got {key}.")
                    }
                };

                let (_, nav) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected a Co-Authored-By in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let nav = read_nav(alias, nav)?;
                let drv = Driver {
                    navigator: nav,
                    key: key.map(String::from),
                };

                drivers.push(drv);
            }
            otherwise => bail!(
                concat!(
                    "Unexpted type `{} in line {}, ",
                    "expected either `navigator` or `driver`."
                ),
                otherwise,
                line_number
            ),
        }
    }

    Ok(Config {
        navigators,
        drivers,
    })
}

fn load_from_json(content: String) -> Result<Config> {
    use serde_json::Value;

    fn read_nav(nav: &Value) -> Result<Navigator> {
        let alias = nav["alias"]
            .as_str()
            .ok_or_else(|| eyre!("alias is not a string"))?;

        let name = nav["name"]
            .as_str()
            .ok_or_else(|| eyre!("name is not a string"))?;

        let email = nav["email"]
            .as_str()
            .ok_or_else(|| eyre!("email is not a string"))?;

        Ok(Navigator {
            alias: Id(String::from(alias)),
            name: String::from(name),
            email: String::from(email),
        })
    }

    let cfg = serde_json::from_str::<Value>(&content)?;

    let mut navigators = Vec::new();
    let mut drivers = Vec::new();

    for nav in cfg["navigators"]
        .as_array()
        .ok_or_else(|| eyre!("navigators is not an array"))?
    {
        let nav = read_nav(nav)?;
        navigators.push(nav);
    }

    for drv in cfg["drivers"]
        .as_array()
        .ok_or_else(|| eyre!("drivers is not an array"))?
    {
        let key = &drv["key"];
        let key = match key {
            Value::Null => None,
            Value::String(key) => Some(key),
            _ => bail!("key is not a string"),
        };
        let nav = read_nav(drv)?;
        let drv = Driver {
            navigator: nav,
            key: key.map(String::from),
        };

        drivers.push(drv);
    }

    Ok(Config {
        navigators,
        drivers,
    })
}

pub fn store(config: &Config) -> Result<()> {
    fn write_nav(content: &mut String, nav: &Navigator) {
        content.push_str("Co-Authored-By: ");
        content.push_str(&nav.name);
        content.push_str(" <");
        content.push_str(&nav.email);
        content.push_str(">\n");
    }

    let file = match config_file(Mode::Write)? {
        ConfigFile::New(path) => path,
        ConfigFile::Old(path) => path,
        ConfigFile::Missing => bail!("The configuration directoy could not be found"),
    };

    let mut content = String::with_capacity(8192);
    content.push_str("version: 1\n");
    for nav in &config.navigators {
        content.push_str("navigator: ");
        content.push_str(&nav.alias);
        content.push('\n');
        write_nav(&mut content, nav);
    }
    for drv in &config.drivers {
        content.push_str("driver: ");
        content.push_str(&drv.navigator.alias);
        content.push('\n');
        content.push_str("key:");
        if let Some(key) = drv.key.as_deref() {
            content.push(' ');
            content.push_str(key);
        }
        content.push('\n');
        write_nav(&mut content, &drv.navigator);
    }

    store_to(&file, content.as_bytes(), true).wrap_err_with(|| {
        format!(
            concat!(
                "The file `{}` could not be written.\n",
                "Please make sure that the config file is accessible and writable.\n",
                "File content:\n{}"
            ),
            file.display(),
            content
        )
    })
}

fn store_to(file: &Path, data: &[u8], create_parent: bool) -> Result<()> {
    let mut f = match File::create(file) {
        Ok(f) => f,
        Err(e) => {
            return match e.kind() {
                ErrorKind::NotFound if create_parent => {
                    if let Some(parent) = file.parent() {
                        fs::create_dir_all(parent)
                            .wrap_err("Could not write the configuration file")?;
                    }
                    store_to(file, data, false)
                }
                ErrorKind::PermissionDenied => Err(eyre!("No permission to write to the file")),
                _ => Err(e.into()),
            }
        }
    };
    f.write_all(data)?;
    f.flush()?;
    Ok(())
}

fn config_file(mode: Mode) -> Result<ConfigFile> {
    fn try_find(path: PathBuf) -> Result<Option<PathBuf>> {
        match path.symlink_metadata() {
            Ok(meta) if meta.is_file() => Ok(Some(path)),
            Ok(meta) => Err(eyre!(
                "Found the config file at {:?}, but it's not a file. It's a {:?}",
                path,
                meta.file_type()
            )),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    let dirs = match ProjectDirs::from("de", "knutwalker", APPLICATION) {
        Some(dirs) => dirs,
        None => return Ok(ConfigFile::Missing),
    };

    let cfg_dir = dirs.config_dir();

    match mode {
        Mode::Read => match try_find(cfg_dir.join(CONFIG_FILE))? {
            Some(cfg) => Ok(ConfigFile::New(cfg)),
            None => Ok(match try_find(cfg_dir.join(OLD_CONFIG_FILE))? {
                Some(cfg) => ConfigFile::Old(cfg),
                None => ConfigFile::Missing,
            }),
        },
        Mode::Write => Ok(ConfigFile::New(cfg_dir.join(CONFIG_FILE))),
    }
}

enum Mode {
    Read,
    Write,
}

enum ConfigFile {
    New(PathBuf),
    Old(PathBuf),
    Missing,
}

impl std::fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigFile::New(path) => path.display().fmt(f),
            ConfigFile::Old(path) => path.display().fmt(f),
            ConfigFile::Missing => f.write_str("<missing>"),
        }
    }
}
