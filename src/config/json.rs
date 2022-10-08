use super::Config;
use crate::data::{Driver, Field, Id, Navigator, PartialNav};
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
    Err as IErr, Finish, IResult, Parser,
};
use std::{borrow::Cow, convert::identity, fs, path::Path, str};

pub fn load_from(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    deserialize_config_json(&content)
        .with_context(|| format!("Reading config json from {}", path.display()))
}

fn deserialize_config_json(content: &str) -> Result<Config> {
    let (_, config) = config(content)
        .map_err(IErr::<Error<&str>>::to_owned)
        .finish()?;
    Ok(config)
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
                Unicode::HighSurrogate(_) | Unicode::Char(_) => (rest, char::REPLACEMENT_CHARACTER),
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

    let nav = fields
        .into_iter()
        .fold(PartialNav::default(), |nav, (field, value)| {
            nav.with(field, value.into_owned())
        });

    match (nav.id, nav.name, nav.email) {
        (Some(id), Some(name), Some(email)) => {
            let navigator = Navigator {
                alias: Id(id),
                name,
                email,
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
