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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub navigators: Vec<Navigator>,
    pub drivers: Vec<Driver>,
}

pub fn load() -> Result<Config> {
    let file = config_file(Mode::Read)?;

    match &file {
        ConfigFile::New(path) => load_from(path),
        ConfigFile::Old(path) => {
            let cfg = load_json_from(path)?;
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

fn load_from(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    deserialize_config(&content).with_context(|| format!("Reading config from {}", path.display()))
}

fn load_json_from(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    deserialize_config_json(&content)
        .with_context(|| format!("Reading config json from {}", path.display()))
}

fn deserialize_config(content: &str) -> Result<Config> {
    fn read_nav(alias: &str, line: &str, line_number: usize) -> Result<Navigator> {
        let co_author = co_authors::CoAuthor::try_from(line).map_err(|e| {
            eyre!(
                "Expected `Co-Authored-By: $name <$email>` in line {}, but got: {}",
                line_number,
                e
            )
        })?;
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
    ensure!(version == "1", "Unknown version: {}.", version);

    let mut navigators = Vec::new();
    let mut drivers = Vec::new();

    while let Some((line_number, line)) = lines.next() {
        let (kind, alias) = line.split_once(": ").ok_or_else(|| {
            eyre!("Expected `$type: $alias`, but got `{line}` in line {line_number}.")
        })?;

        match kind {
            "navigator" => {
                let (line_number, nav) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected `Co-Authored-By: $name <$email>` in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let nav = read_nav(alias, nav, line_number)?;
                navigators.push(nav);
            }
            "driver" => {
                let (line_number, key) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected `key: $key` in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let key = match key.split_once(':') {
                    Some(("key", key)) => Some(key).map(str::trim).filter(|k| !k.is_empty()),
                    Some(_) | None => {
                        bail!("Expected `key: $key` in line {line_number}, but got {key}.")
                    }
                };

                let (line_number, nav) = lines.next().ok_or_else(|| {
                    eyre!(
                        "Expected a Co-Authored-By in line {}, but reached the end of the file.",
                        line_number + 1
                    )
                })?;
                let nav = read_nav(alias, nav, line_number)?;
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

fn deserialize_config_json(content: &str) -> Result<Config> {
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

    let cfg = serde_json::from_str::<Value>(content)?;

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
    let file = match config_file(Mode::Write)? {
        ConfigFile::New(path) | ConfigFile::Old(path) => path,
        ConfigFile::Missing => bail!("The configuration directoy could not be found"),
    };

    store_in(config, &file)
}

fn store_in(config: &Config, path: &Path) -> Result<()> {
    let content = serialize_config(config);

    store_to(path, content.as_bytes(), true).wrap_err_with(|| {
        format!(
            concat!(
                "The file `{}` could not be written.\n",
                "Please make sure that the config file is accessible and writable.\n",
                "File content:\n{}"
            ),
            path.display(),
            content
        )
    })
}

fn serialize_config(config: &Config) -> String {
    fn write_nav(content: &mut String, nav: &Navigator) {
        content.push_str("Co-Authored-By: ");
        content.push_str(&nav.name);
        content.push_str(" <");
        content.push_str(&nav.email);
        content.push_str(">\n");
    }

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

    content
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
    let dirs = match ProjectDirs::from("de", "knutwalker", APPLICATION) {
        Some(dirs) => dirs,
        None => return Ok(ConfigFile::Missing),
    };

    config_file_in(mode, dirs.config_dir())
}

fn config_file_in(mode: Mode, dir: &Path) -> Result<ConfigFile> {
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

    match mode {
        Mode::Read => match try_find(dir.join(CONFIG_FILE))? {
            Some(cfg) => Ok(ConfigFile::New(cfg)),
            None => Ok(match try_find(dir.join(OLD_CONFIG_FILE))? {
                Some(cfg) => ConfigFile::Old(cfg),
                None => ConfigFile::Missing,
            }),
        },
        Mode::Write => Ok(ConfigFile::New(dir.join(CONFIG_FILE))),
    }
}

#[derive(Copy, Clone, Debug)]
enum Mode {
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConfigFile {
    New(PathBuf),
    Old(PathBuf),
    Missing,
}

impl std::fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New(path) | Self::Old(path) => path.display().fmt(f),
            Self::Missing => f.write_str("<missing>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::{
        prelude::{FileTouch, PathChild},
        TempDir,
    };

    fn nav1() -> Navigator {
        Navigator {
            alias: Id::from("nav1"),
            name: String::from("bernd"),
            email: String::from("foo@bar.org"),
        }
    }

    fn nav2() -> Navigator {
        Navigator {
            alias: Id::from("nav2"),
            name: String::from("ronny"),
            email: String::from("baz@bar.org"),
        }
    }

    fn drv1(key: impl Into<Option<&'static str>>) -> Driver {
        Driver {
            navigator: Navigator {
                alias: Id::from("drv1"),
                name: String::from("ralle"),
                email: String::from("qux@bar.org"),
            },
            key: key.into().map(String::from),
        }
    }

    #[test]
    fn serialize_empty_config() {
        let config = Config {
            navigators: vec![],
            drivers: vec![],
        };

        let config = serialize_config(&config);

        assert_eq!(config, "version: 1\n");
    }

    #[test]
    fn serialize_one_nav() {
        let config = Config {
            navigators: vec![nav1()],
            drivers: vec![],
        };

        let config = serialize_config(&config);

        assert_eq!(
            config,
            concat!(
                "version: 1\n",
                "navigator: nav1\n",
                "Co-Authored-By: bernd <foo@bar.org>\n",
            )
        );
    }

    #[test]
    fn serialize_multiple_navs() {
        let config = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![],
        };

        let config = serialize_config(&config);

        assert_eq!(
            config,
            concat!(
                "version: 1\n",
                "navigator: nav1\n",
                "Co-Authored-By: bernd <foo@bar.org>\n",
                "navigator: nav2\n",
                "Co-Authored-By: ronny <baz@bar.org>\n",
            )
        );
    }

    #[test]
    fn serialize_one_driver_without_key() {
        let config = Config {
            navigators: vec![],
            drivers: vec![drv1(None)],
        };

        let config = serialize_config(&config);

        assert_eq!(
            config,
            concat!(
                "version: 1\n",
                "driver: drv1\n",
                "key:\n",
                "Co-Authored-By: ralle <qux@bar.org>\n",
            )
        );
    }

    #[test]
    fn serialize_one_driver_with_key() {
        let config = Config {
            navigators: vec![],
            drivers: vec![drv1("my-key.pub")],
        };

        let config = serialize_config(&config);

        assert_eq!(
            config,
            concat!(
                "version: 1\n",
                "driver: drv1\n",
                "key: my-key.pub\n",
                "Co-Authored-By: ralle <qux@bar.org>\n",
            )
        );
    }

    #[test]
    fn serialize_all() {
        let config = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![drv1("my-key.pub")],
        };

        let config = serialize_config(&config);

        assert_eq!(
            config,
            concat!(
                "version: 1\n",
                "navigator: nav1\n",
                "Co-Authored-By: bernd <foo@bar.org>\n",
                "navigator: nav2\n",
                "Co-Authored-By: ronny <baz@bar.org>\n",
                "driver: drv1\n",
                "key: my-key.pub\n",
                "Co-Authored-By: ralle <qux@bar.org>\n",
            )
        );
    }

    #[test]
    fn deserialize_empty_config() {
        let config = "version: 1\n";
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![],
            drivers: vec![],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_one_nav() {
        let config = concat!(
            "version: 1\n",
            "navigator: nav1\n",
            "Co-Authored-By: bernd <foo@bar.org>\n",
        );
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![nav1()],
            drivers: vec![],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_multiple_navs() {
        let config = concat!(
            "version: 1\n",
            "navigator: nav1\n",
            "Co-Authored-By: bernd <foo@bar.org>\n",
            "navigator: nav2\n",
            "Co-Authored-By: ronny <baz@bar.org>\n",
        );
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_one_driver_without_key() {
        let config = concat!(
            "version: 1\n",
            "driver: drv1\n",
            "key:\n",
            "Co-Authored-By: ralle <qux@bar.org>\n",
        );
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![],
            drivers: vec![drv1(None)],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_one_driver_with_key() {
        let config = concat!(
            "version: 1\n",
            "driver: drv1\n",
            "key: my-key.pub\n",
            "Co-Authored-By: ralle <qux@bar.org>\n",
        );
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![],
            drivers: vec![drv1("my-key.pub")],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_all() {
        let config = concat!(
            "version: 1\n",
            "navigator: nav1\n",
            "Co-Authored-By: bernd <foo@bar.org>\n",
            "navigator: nav2\n",
            "Co-Authored-By: ronny <baz@bar.org>\n",
            "driver: drv1\n",
            "key: my-key.pub\n",
            "Co-Authored-By: ralle <qux@bar.org>\n",
        );
        let config = deserialize_config(config).unwrap();

        let expected = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![drv1("my-key.pub")],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn deserialize_empty() {
        let config = "";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "The config file is empty, expected at least a version key."
        );
    }

    #[test]
    fn deserialize_missing_version() {
        let config = "navigator: bernd";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Expected `version: $version`, but got `navigator: bernd` in line 1."
        );
    }

    #[test]
    fn deserialize_unexpected_version() {
        let config = "version: 2";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(config.to_string(), "Unknown version: 2.");
    }

    #[test]
    fn deserialize_unexpected_key() {
        let config = "version: 1\nfoo: bar";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Unexpted type `foo in line 2, expected either `navigator` or `driver`."
        );
    }

    #[test]
    fn deserialize_missing_coauthors_line() {
        let config = "version: 1\nnavigator: foo\n";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Expected `Co-Authored-By: $name <$email>` in line 3, but reached the end of the file."
        );
    }

    #[test]
    fn deserialize_skipped_coauthors_line() {
        let config = "version: 1\nnavigator: foo\nnavigator: bar";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Expected `Co-Authored-By: $name <$email>` in line 3, but got: The trailer is missing the `Co-Authored-By:` key."
        );
    }

    #[test]
    fn deserialize_wrong_coauthors_line() {
        let config = "version: 1\nnavigator: foo\nco-authored-by bernd <foo@bar.org>";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Expected `Co-Authored-By: $name <$email>` in line 3, but got: The trailer is missing the `Co-Authored-By:` key."
        );
    }

    #[test]
    fn deserialize_wrong_coauthors_line_missing_name() {
        let config = "version: 1\nnavigator: foo\nco-authored-by: <foo@bar.org>";
        let config = deserialize_config(config).unwrap_err();
        assert_eq!(
            config.to_string(),
            "Expected `Co-Authored-By: $name <$email>` in line 3, but got: The name of the co-author is missing."
        );
    }

    #[test]
    fn deserialize_json() {
        let config = r#"{
            "navigators": [{
                "alias": "nav1",
                "name": "bernd",
                "email": "foo@bar.org"
            }, {
                "alias": "nav2",
                "name": "ronny",
                "email": "baz@bar.org"
            }],
            "drivers": [{
                "alias": "drv1",
                "name": "ralle",
                "email": "qux@bar.org",
                "key": "my-key.pub"
            }]
        }"#;
        let config = deserialize_config_json(config).unwrap();

        let expected = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![drv1("my-key.pub")],
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn find_config_file_for_reading_in_empty_dir() {
        let dir = TempDir::new().unwrap();

        let cfg = config_file_in(Mode::Read, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::Missing);

        dir.close().unwrap();
    }

    #[test]
    fn find_old_config_file_for_reading_in_dir_with_json_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(OLD_CONFIG_FILE);
        file.touch().unwrap();

        let cfg = config_file_in(Mode::Read, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::Old(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_reading_in_dir_with_config_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(CONFIG_FILE);
        file.touch().unwrap();

        let cfg = config_file_in(Mode::Read, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_reading_in_dir_with_both_files() {
        let dir = TempDir::new().unwrap();
        dir.child(OLD_CONFIG_FILE).touch().unwrap();
        let file = dir.child(CONFIG_FILE);
        file.touch().unwrap();

        let cfg = config_file_in(Mode::Read, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_writing_in_empty_dir() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(CONFIG_FILE);

        let cfg = config_file_in(Mode::Write, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_writing_in_dir_with_json_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(OLD_CONFIG_FILE);
        file.touch().unwrap();
        let file = dir.child(CONFIG_FILE);

        let cfg = config_file_in(Mode::Write, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_writing_in_dir_with_config_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(CONFIG_FILE);
        file.touch().unwrap();

        let cfg = config_file_in(Mode::Write, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn find_new_config_file_for_writing_in_dir_with_both_files() {
        let dir = TempDir::new().unwrap();
        let file = dir.child(CONFIG_FILE);
        file.touch().unwrap();
        dir.child(OLD_CONFIG_FILE).touch().unwrap();

        let cfg = config_file_in(Mode::Write, dir.path()).unwrap();
        assert_eq!(cfg, ConfigFile::New(file.path().to_path_buf()));

        dir.close().unwrap();
    }

    #[test]
    fn config_roundtrip() {
        use assert_fs::prelude::*;
        let dir = TempDir::new().unwrap();
        let out = dir.child("out.txt");

        let config = Config {
            navigators: vec![nav1(), nav2()],
            drivers: vec![drv1("my-key.pub")],
        };

        store_in(&config, out.path()).unwrap();

        let expected = concat!(
            "version: 1\n",
            "navigator: nav1\n",
            "Co-Authored-By: bernd <foo@bar.org>\n",
            "navigator: nav2\n",
            "Co-Authored-By: ronny <baz@bar.org>\n",
            "driver: drv1\n",
            "key: my-key.pub\n",
            "Co-Authored-By: ralle <qux@bar.org>\n",
        );

        out.assert(expected);

        let roundtrip = load_from(out.path()).unwrap();

        assert_eq!(roundtrip, config);

        dir.close().unwrap();
    }
}
