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

mod json {
    use super::Config;
    use crate::data::{Driver, Id, Navigator};
    use eyre::{Context, Result};
    use nom::{
        branch::alt,
        bytes::{
            complete::{tag, take, take_while},
            streaming::is_not,
        },
        character::complete::{char, hex_digit1},
        combinator::{all_consuming, complete, cut, map, map_parser, map_res, value, verify},
        error::{make_error, Error, ErrorKind},
        multi::{fold_many0, separated_list0, separated_list1},
        sequence::{delimited, preceded},
        Err as IErr, IResult, Parser,
    };
    use std::{borrow::Cow, convert::identity, fs, path::Path, str};

    pub fn load_from(path: &Path) -> Result<Config> {
        let content = fs::read_to_string(path)?;
        deserialize_config_json(&content)
            .with_context(|| format!("Reading config json from {}", path.display()))
    }

    fn deserialize_config_json(content: &str) -> Result<Config> {
        let config = config(content);
        match config {
            Ok((_, config)) => Ok(config),
            Err(e) => match e.to_owned() {
                IErr::Error(e) | IErr::Failure(e) => Err(e.into()),
                IErr::Incomplete(_) => unreachable!("config parser is complete"),
            },
        }
    }

    fn sp(input: &str) -> IResult<&str, &str> {
        let chars = " \t\r\n";
        take_while(move |c| chars.contains(c))(input)
    }

    fn array<'a, I, O>(inner: I) -> impl Parser<&'a str, Vec<O>, Error<&'a str>>
    where
        I: Parser<&'a str, O, Error<&'a str>>,
    {
        let opening = char('[');
        let opening = preceded(sp, opening);

        let closing = char(']');
        let closing = preceded(sp, closing);

        let separator = char(',');
        let separator = preceded(sp, separator);

        let inner = preceded(sp, inner);
        let inner = separated_list0(separator, inner);
        let inner = cut(inner);

        delimited(opening, inner, closing)
    }

    fn obj<'a, I, O>(inner: I) -> impl Parser<&'a str, O, Error<&'a str>>
    where
        I: Parser<&'a str, O, Error<&'a str>>,
    {
        let opening = char('{');
        let opening = preceded(sp, opening);

        let closing = char('}');
        let closing = preceded(sp, closing);

        let inner = preceded(sp, inner);
        let inner = cut(inner);

        delimited(opening, inner, closing)
    }

    #[derive(Copy, Clone, Debug)]
    enum Unicode {
        Char(u16),
        LowSurrogate(u16),
        HighSurrogate(u16),
    }

    /// Parse a unicode sequence, of the form `\uXXXX`.
    fn parse_unicode_code(input: &str) -> IResult<&str, Unicode> {
        // 4 hex digits
        let parse_hex = map_parser(complete(take(4_usize)), hex_digit1);
        // preceeded by an u: uXXXX
        let parse_hex = preceded(char('u'), cut(parse_hex));
        // parse into u16
        let parse_hex = map_res(parse_hex, move |hex| u16::from_str_radix(hex, 16));
        // split into char or surrogate pairs
        let parse_hex = map(parse_hex, |n| match n {
            0xDC00..=0xDFFF => Unicode::LowSurrogate(n),
            0xD800..=0xDBFF => Unicode::HighSurrogate(n),
            n => Unicode::Char(n),
        });

        identity(parse_hex)(input)
    }

    /// Parse a unicode character, either a single char or a surrogate pair.
    fn parse_unicode_char(input: &str) -> IResult<&str, char> {
        let (rest, code) = parse_unicode_code(input)?;
        Ok(match code {
            Unicode::Char(c) => (
                rest,
                char::from_u32(u32::from(c)).unwrap_or(char::REPLACEMENT_CHARACTER),
            ),
            Unicode::HighSurrogate(n1) => {
                let next = cut(preceded(char('\\'), parse_unicode_code));
                let (next_rest, code) = identity(next)(rest)?;
                match code {
                    Unicode::LowSurrogate(n2) => {
                        let n1 = u32::from(n1 - 0xD800) << 10;
                        let n2 = u32::from(n2 - 0xDC00) + 0x1_0000;
                        let n = n1 | n2;
                        (
                            next_rest,
                            char::from_u32(n).unwrap_or(char::REPLACEMENT_CHARACTER),
                        )
                    }
                    Unicode::HighSurrogate(_) | Unicode::Char(_) => {
                        (rest, char::REPLACEMENT_CHARACTER)
                    }
                }
            }
            Unicode::LowSurrogate(_) => (rest, char::REPLACEMENT_CHARACTER),
        })
    }

    /// Parse an escaped character: `\n`, `\t`, `\r`, `\u00AC`, etc.
    fn parse_escaped_char(input: &str) -> IResult<&str, char> {
        preceded(
            char('\\'),
            cut(alt((
                value('"', char('"')),
                value('\\', char('\\')),
                value('/', char('/')),
                value('\x08', char('b')),
                value('\x0C', char('f')),
                value('\n', char('n')),
                value('\r', char('r')),
                value('\t', char('t')),
                parse_unicode_char,
            ))),
        )(input)
    }

    /// Parse a non-empty block of text that doesn't include \ or "
    fn parse_literal(input: &str) -> IResult<&str, &str> {
        let literal = is_not(r#""\"#);
        let literal = verify(literal, |s: &str| !s.is_empty());

        identity(literal)(input)
    }

    #[derive(Copy, Clone, Debug)]
    enum Fragment<'a> {
        Literal(&'a str),
        Escaped(char),
    }

    /// Combine `parse_literal` and `parse_escaped_char` into a Fragment.
    fn parse_fragment(input: &str) -> IResult<&str, Fragment<'_>> {
        alt((
            map(parse_literal, Fragment::Literal),
            map(parse_escaped_char, Fragment::Escaped),
        ))(input)
    }

    type Str<'a> = Cow<'a, str>;

    fn combine_fragments<'a>(mut buf: Option<Str<'a>>, fragment: Fragment<'a>) -> Option<Str<'a>> {
        match fragment {
            Fragment::Literal(s) => match buf.as_mut() {
                Some(cow) => cow.to_mut().push_str(s),
                None => return Some(Cow::Borrowed(s)),
            },
            Fragment::Escaped(c) => buf.get_or_insert_with(Default::default).to_mut().push(c),
        };
        buf
    }

    /// Parse a string.
    /// Strings without escape sequence are returned as a borrowed str.
    /// Strings with an escape sequence are returned as an owned String.
    fn string(input: &str) -> IResult<&str, Str<'_>> {
        let string = fold_many0(parse_fragment, || None, combine_fragments);
        let string = map(string, Option::unwrap_or_default);
        let string = delimited(char('"'), cut(string), char('"'));

        identity(string)(input)
    }

    enum Field {
        Alias,
        Name,
        Email,
    }

    fn str_val(input: &str) -> IResult<&str, Str<'_>> {
        let separator = value((), char(':'));
        let separator = preceded(sp, separator);
        let value = preceded(sp, string);
        let value = preceded(separator, value);
        let value = cut(value);

        identity(value)(input)
    }

    fn nav_field(input: &str) -> IResult<&str, (Field, Str<'_>)> {
        let field = cut(alt((
            map(preceded(tag(r#""alias""#), str_val), |s| (Field::Alias, s)),
            map(preceded(tag(r#""name""#), str_val), |s| (Field::Name, s)),
            map(preceded(tag(r#""email""#), str_val), |s| (Field::Email, s)),
        )));
        let field = preceded(sp, field);
        identity(field)(input)
    }

    fn inner_navigator(input: &str) -> IResult<&str, Navigator> {
        let delimiter = value((), char(','));
        let delimiter = preceded(sp, delimiter);

        let fields = preceded(sp, nav_field);
        let fields = separated_list1(delimiter, fields);

        let (rest, fields) = identity(fields)(input)?;

        let mut alias = None;
        let mut name = None;
        let mut email = None;

        for (field, value) in fields {
            match field {
                Field::Alias => alias = Some(value),
                Field::Name => name = Some(value),
                Field::Email => email = Some(value),
            }
        }

        match (alias, name, email) {
            (Some(id), Some(name), Some(email)) => {
                let navigator = Navigator {
                    alias: Id(id.into_owned()),
                    name: name.into_owned(),
                    email: email.into_owned(),
                };
                Ok((rest, navigator))
            }
            _ => Err(IErr::Failure(make_error(input, ErrorKind::ManyMN))),
        }
    }

    fn navigator(input: &str) -> IResult<&str, Navigator> {
        obj(inner_navigator).parse(input)
    }

    fn driver(input: &str) -> IResult<&str, Driver> {
        map(navigator, |navigator| Driver {
            navigator,
            key: None,
        })(input)
    }

    fn navigators(input: &str) -> IResult<&str, Vec<Navigator>> {
        let separator = value((), char(':'));
        let separator = preceded(sp, separator);
        let navigators = array(navigator);
        let navigators = preceded(separator, navigators);
        let navigators = cut(navigators);
        let navigators = preceded(tag(r#""navigators""#), navigators);

        identity(navigators)(input)
    }

    fn drivers(input: &str) -> IResult<&str, Vec<Driver>> {
        let separator = value((), char(':'));
        let separator = preceded(sp, separator);
        let drivers = array(driver);
        let drivers = preceded(separator, drivers);
        let drivers = cut(drivers);
        let drivers = preceded(tag(r#""drivers""#), drivers);

        identity(drivers)(input)
    }

    enum Items {
        Navigators(Vec<Navigator>),
        Drivers(Vec<Driver>),
    }

    fn config_items(input: &str) -> IResult<&str, Items> {
        alt((
            map(navigators, Items::Navigators),
            map(drivers, Items::Drivers),
        ))(input)
    }

    fn inner_config(input: &str) -> IResult<&str, Config> {
        let delimiter = value((), char(','));
        let delimiter = preceded(sp, delimiter);

        let config = preceded(sp, config_items);
        let config = separated_list1(delimiter, config);
        let config = map(config, |items| {
            items
                .into_iter()
                .fold(Config::default(), |mut config, item| {
                    match item {
                        Items::Navigators(navigators) => config.navigators = navigators,
                        Items::Drivers(drivers) => config.drivers = drivers,
                    }
                    config
                })
        });

        identity(config)(input)
    }

    fn config(input: &str) -> IResult<&str, Config> {
        let config = obj(inner_config);
        let config = complete(config);
        let config = all_consuming(config);

        identity(config)(input)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::data::tests::{drv1, nav1, nav2};

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
                    "email": "qux@bar.org"
                }]
            }"#;
            let config = deserialize_config_json(config).unwrap();

            let expected = Config {
                navigators: vec![nav1(), nav2()],
                drivers: vec![drv1(None)],
            };
            assert_eq!(config, expected);
        }

        #[test]
        fn deserialize_escapes() {
            let config = r#"{
                "navigators": [{
                    "alias": "nav",
                    "name": "foo \uD83D\uDE05 bar\tbaz \u03c0 ",
                    "email": "\"foo\"@\\\/bar.org\r\n"
                }]
            }"#;
            let config = deserialize_config_json(config).unwrap();

            let expected = Config::from_iter([Navigator {
                alias: Id::from("nav"),
                name: String::from("foo 😅 bar\tbaz π "),
                email: String::from("\"foo\"@\\/bar.org\r\n"),
            }]);
            assert_eq!(config, expected);
        }

        #[test]
        fn deserialize_broken_escapes() {
            let config = r#"{
                "navigators": [{
                    "alias": "nav",
                    "name": "\uD7FE\uDCFF",
                    "email": "foo"
                }]
            }"#;
            let config = deserialize_config_json(config).unwrap();

            let expected = Config::from_iter([Navigator {
                alias: Id::from("nav"),
                name: String::from("\u{d7fe}\u{fffd}"),
                email: String::from("foo"),
            }]);
            assert_eq!(config, expected);
        }
    }
}

const APPLICATION: &str = env!("CARGO_PKG_NAME");
const OLD_CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.json");
const CONFIG_FILE: &str = concat!(env!("CARGO_PKG_NAME"), "_config.gitdrive");

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub navigators: Vec<Navigator>,
    pub drivers: Vec<Driver>,
}

impl FromIterator<Navigator> for Config {
    fn from_iter<T: IntoIterator<Item = Navigator>>(iter: T) -> Self {
        Self {
            navigators: iter.into_iter().collect(),
            drivers: Vec::new(),
        }
    }
}

impl FromIterator<Driver> for Config {
    fn from_iter<T: IntoIterator<Item = Driver>>(iter: T) -> Self {
        Self {
            navigators: Vec::new(),
            drivers: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
enum Entity {
    Nav(Navigator),
    Drv(Driver),
}

#[cfg(test)]
impl Navigator {
    const fn ent(self) -> Entity {
        Entity::Nav(self)
    }
}

#[cfg(test)]
impl Driver {
    const fn ent(self) -> Entity {
        Entity::Drv(self)
    }
}

#[cfg(test)]
impl FromIterator<Entity> for Config {
    fn from_iter<T: IntoIterator<Item = Entity>>(iter: T) -> Self {
        let (mut navigators, mut drivers) = (Vec::new(), Vec::new());
        for ent in iter {
            match ent {
                Entity::Nav(nav) => navigators.push(nav),
                Entity::Drv(drv) => drivers.push(drv),
            }
        }

        Self {
            navigators,
            drivers,
        }
    }
}

pub fn load() -> Result<Config> {
    let file = config_file(Mode::Read)?;

    match &file {
        ConfigFile::New(path) => load_from(path),
        ConfigFile::Old(path) => {
            let cfg = json::load_from(path)?;
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
    use crate::data::tests::{drv1, nav1, nav2};
    use assert_fs::{
        prelude::{FileTouch, PathChild},
        TempDir,
    };

    #[test]
    fn serialize_empty_config() {
        let config = Config::default();
        let config = serialize_config(&config);

        assert_eq!(config, "version: 1\n");
    }

    #[test]
    fn serialize_one_nav() {
        let config = Config::from_iter(Some(nav1()));
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
        let config = Config::from_iter([nav1(), nav2()]);
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
        let config = Config::from_iter(Some(drv1(None)));
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
        let config = Config::from_iter(Some(drv1("my-key.pub")));
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
        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1("my-key.pub").ent()]);
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

        let expected = Config::default();
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

        let expected = Config::from_iter(Some(nav1()));
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

        let expected = Config::from_iter([nav1(), nav2()]);
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

        let expected = Config::from_iter(Some(drv1(None)));
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

        let expected = Config::from_iter(Some(drv1("my-key.pub")));
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

        let expected = Config::from_iter([nav1().ent(), nav2().ent(), drv1("my-key.pub").ent()]);
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

        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1("my-key.pub").ent()]);

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
