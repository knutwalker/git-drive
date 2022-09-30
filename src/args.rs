use crate::{
    data::{Id, PartialNav, ShowNav},
    Result,
};
use clap::{builder::ValueParser, error::ErrorKind, Arg, ArgAction, ArgMatches, Command};
use std::{ffi::OsString, io::Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    DriveFromSelection,
    DriveWith(Id),
    DriveWithAll(Vec<Id>),
    DriveAlone,
    ListNavigators,
    ListDrivers,
    ShowCurrentNavigator(ShowNav),
    NewNavigator(PartialNav),
    EditNavigator(PartialNav),
    DeleteNavigatorFromSelection,
    DeleteNavigator(Id),
    DeleteAllNavigators(Vec<Id>),
    DriveAsFromSelection,
    DriveAs(Id),
    NewDriver(PartialNav),
    EditDriver(PartialNav),
    DeleteDriverFromSelection,
    DeleteDriver(Id),
    DeleteAllDrivers(Vec<Id>),
}

pub(crate) fn action() -> Action {
    Action::parse()
}

pub(crate) fn print_help_stderr() -> std::io::Result<()> {
    let mut app = Action::app();
    let help = app.render_long_help();
    drop(app);

    let out = std::io::stderr();
    let mut out = out.lock();
    write!(out, "{}", help)?;
    out.flush()
}

impl Action {
    fn parse() -> Self {
        Self::parse_from(std::env::args_os())
    }

    fn parse_from<I>(args: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<OsString> + Clone,
    {
        match Self::try_parse_from(args) {
            Ok(action) => action,
            Err((mut app, e)) => {
                let e = e.format(&mut app);
                drop(app);

                if cfg!(test) {
                    panic!("Clap returned an error:\n{0:#?}\n{0}", e);
                } else {
                    e.exit()
                }
            }
        }
    }

    fn try_parse_from<I>(args: I) -> Result<Self, (Command, clap::Error)>
    where
        I: IntoIterator,
        I::Item: Into<OsString> + Clone,
    {
        #[cfg(test)]
        let args =
            std::iter::once(OsString::from("test-prog")).chain(args.into_iter().map(|s| s.into()));

        let mut app = Self::app();
        let matches = match app.try_get_matches_from_mut(args) {
            Ok(matches) => matches,
            Err(e) => return Err((app, e)),
        };
        match Self::action_from_matches(matches) {
            Ok(action) => Ok(action),
            Err(e) => Err((app, e)),
        }
    }

    fn app() -> Command {
        Command::new(env!("CARGO_PKG_NAME"))
            .version(env!("CARGO_PKG_VERSION"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .propagate_version(true)
            .infer_long_args(true)
            .infer_subcommands(true)
            .subcommand_required(false)
            .subcommand(
                Command::new("with")
                    .arg(Self::ids_arg().required(true).num_args(1..))
                    .about("Start driving with the specified navigator(s)"),
            )
            .subcommand(Command::new("alone").about("Start driving alone"))
            .subcommand(
                Command::new("list")
                    .subcommand(Command::new("me").about("It's me"))
                    .about("List known navigators"),
            )
            .subcommand(
                Command::new("show")
                    .arg(
                        Arg::new("color")
                            .short('c')
                            .long("color")
                            .visible_alias("colour")
                            .value_name("COLOR")
                            .num_args(..=1)
                            .default_missing_value("cyan")
                            .default_value("none")
                            .value_parser(ValueParser::string())
                            .action(ArgAction::Set)
                            .help("The color in which to print the current navigators"),
                    )
                    .arg(
                        Arg::new("fail-if-empty")
                            .long("fail-if-empty")
                            .action(ArgAction::SetTrue)
                            .help("If set, fail the process if there are no current navigators"),
                    )
                    .about("Show current navigators"),
            )
            .subcommand(
                Command::new("new")
                    .args(Self::partial_nav_args())
                    .about("Add a new navigator, either prompted for, or specified"),
            )
            .subcommand(
                Command::new("edit")
                    .args(Self::partial_nav_args())
                    .about("Edit navigator(s), either prompted for, or specified"),
            )
            .subcommand(
                Command::new("delete")
                    .arg(Self::ids_arg())
                    .about("Deletes navigator(s), either prompted for, or specified"),
            )
            .subcommand(
                Command::new("as")
                    .arg(Self::ids_arg().num_args(..=1))
                    .about("Change driver seat"),
            )
            .subcommand(
                Command::new("me")
                    .subcommand(Command::new("list").about("List known drivers"))
                    .subcommand(
                        Command::new("new")
                            .args(Self::partial_nav_args())
                            .arg(Self::key_arg())
                            .about("Add a new driver, either prompted for, or specified"),
                    )
                    .subcommand(
                        Command::new("edit")
                            .args(Self::partial_nav_args())
                            .arg(Self::key_arg())
                            .about("Edit driver(s), either prompted for, or specified"),
                    )
                    .subcommand(
                        Command::new("delete")
                            .arg(Self::ids_arg().help("The drivers"))
                            .about("Deletes driver(s), either prompted for, or specified"),
                    )
                    .subcommand_required(true)
                    .arg_required_else_help(true)
                    .about("Operate on the driver instead of the navigator"),
            )
    }

    fn ids_arg() -> Arg {
        Arg::new("ids")
            .value_name("IDS")
            .value_parser(ValueParser::string())
            .action(ArgAction::Append)
            .help("The navigators")
    }

    fn key_arg() -> Arg {
        Arg::new("key")
            .long("key")
            .visible_alias("signingkey")
            .value_name("KEY")
            .value_parser(ValueParser::string())
            .action(ArgAction::Set)
            .help("The signing key to use")
    }

    fn partial_nav_args() -> [Arg; 4] {
        [
            Arg::new("as")
                .long("as")
                .value_name("AS")
                .value_parser(ValueParser::string())
                .action(ArgAction::Set)
                .help("The identifier to use for the author's entry")
                .conflicts_with("alias"),
            Arg::new("name")
                .long("name")
                .value_name("NAME")
                .value_parser(ValueParser::string())
                .action(ArgAction::Set)
                .help("The author's name"),
            Arg::new("email")
                .long("email")
                .value_name("EMAIL")
                .value_parser(ValueParser::string())
                .action(ArgAction::Set)
                .help("The author's email"),
            Arg::new("alias")
                .value_name("ALIAS")
                .value_parser(ValueParser::string())
                .action(ArgAction::Set)
                .help("The identifier to use for the author's entry")
                .conflicts_with("as"),
        ]
    }

    fn action_from_matches(mut matches: ArgMatches) -> Result<Action, clap::Error> {
        let (name, mut matches) = match matches.remove_subcommand() {
            None => return Ok(Action::DriveFromSelection),
            Some((name, matches)) => (name, matches),
        };
        match name.as_str() {
            "with" => Ok(fold_map(
                matches.remove_many::<String>("ids"),
                Action::DriveAlone,
                Action::DriveWith,
                Action::DriveWithAll,
            )),
            "alone" => Ok(Action::DriveAlone),
            "list" => Ok(matches
                .subcommand()
                .map(|_| Action::ListDrivers)
                .unwrap_or(Action::ListNavigators)),
            "show" => Ok(Action::ShowCurrentNavigator(ShowNav {
                color: matches.remove_one::<String>("color").expect("has default"),
                fail_if_empty: matches.get_flag("fail-if-empty"),
            })),
            "new" => Ok(Action::NewNavigator(Self::partial_nav(matches, false))),
            "edit" => Ok(Action::EditNavigator(Self::partial_nav(matches, false))),
            "delete" => Ok(fold_map(
                matches.remove_many::<String>("ids"),
                Action::DeleteNavigatorFromSelection,
                Action::DeleteNavigator,
                Action::DeleteAllNavigators,
            )),
            "as" => Ok(fold_map(
                matches.remove_many::<String>("ids"),
                Action::DriveAsFromSelection,
                Action::DriveAs,
                |_| panic!("Cannot drive as multiple drivers"),
            )),
            "me" => matches
                .remove_subcommand()
                .ok_or_else(|| {
                    clap::Error::raw(
                        ErrorKind::MissingSubcommand,
                        "A subcommand is required but one was not provided.",
                    )
                })
                .and_then(|(name, mut matches)| {
                    Ok(match name.as_str() {
                        "list" => Action::ListDrivers,
                        "new" => Action::NewDriver(Self::partial_nav(matches, true)),
                        "edit" => Action::EditDriver(Self::partial_nav(matches, true)),
                        "delete" => fold_map(
                            matches.remove_many::<String>("ids"),
                            Action::DeleteDriverFromSelection,
                            Action::DeleteDriver,
                            Action::DeleteAllDrivers,
                        ),
                        othwerise => return Err(Self::unknown_command(othwerise)),
                    })
                }),
            otherwise => Err(Self::unknown_command(otherwise)),
        }
    }

    fn partial_nav(mut matches: ArgMatches, key: bool) -> PartialNav {
        PartialNav {
            id: matches
                .remove_one::<String>("as")
                .or_else(|| matches.remove_one::<String>("alias")),
            name: matches.remove_one::<String>("name"),
            email: matches.remove_one::<String>("email"),
            key: key.then(|| matches.remove_one::<String>("key")).flatten(),
        }
    }

    fn unknown_command(name: &str) -> clap::Error {
        clap::Error::raw(
            ErrorKind::UnknownArgument,
            format!("The subcommand '{}' wasn't recognized", name),
        )
    }
}

fn fold_map<I: ExactSizeIterator<Item = String>, R>(
    items: Option<I>,
    empty: R,
    one: impl FnOnce(Id) -> R,
    many: impl FnOnce(Vec<Id>) -> R,
) -> R {
    match items {
        Some(mut items) if items.len() == 1 => one(Id(items.next().unwrap())),
        None => empty,
        Some(items) => many(items.map(Id).collect()),
    }
}

#[cfg(test)]
mod tests {
    use std::convert::identity;

    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn no_args_intiates_selection() {
        let action = Action::parse_from(identity::<[&str; 0]>([]));
        assert_eq!(action, Action::DriveFromSelection)
    }

    #[test]
    fn with() {
        let action = Action::parse_from(["with", "foo"]);
        assert_eq!(action, Action::DriveWith(Id::from("foo")));

        let action = Action::parse_from(["with", "foo", "bar"]);
        assert_eq!(
            action,
            Action::DriveWithAll(vec![Id::from("foo"), Id::from("bar")])
        );
    }

    #[test]
    fn with_requires_at_least_one_id() {
        let (_, err) = Action::try_parse_from(["with"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn alone() {
        let action = Action::parse_from(["alone"]);
        assert_eq!(action, Action::DriveAlone);
    }

    #[test]
    fn list_navigators() {
        let action = Action::parse_from(["list"]);
        assert_eq!(action, Action::ListNavigators);
    }

    #[test]
    fn list_drivers() {
        let action = Action::parse_from(["list", "me"]);
        assert_eq!(action, Action::ListDrivers);
    }

    #[test]
    fn show_current() {
        let action = Action::parse_from(["show"]);
        assert_eq!(
            action,
            Action::ShowCurrentNavigator(ShowNav {
                color: String::from("none"),
                fail_if_empty: false
            })
        );
    }

    #[test]
    fn show_current_color() {
        let action = Action::parse_from(["show", "--color"]);
        assert_eq!(
            action,
            Action::ShowCurrentNavigator(ShowNav {
                color: String::from("cyan"),
                fail_if_empty: false
            })
        );

        let action = Action::parse_from(["show", "--color", "bold.red"]);
        assert_eq!(
            action,
            Action::ShowCurrentNavigator(ShowNav {
                color: String::from("bold.red"),
                fail_if_empty: false
            })
        );
    }

    #[test]
    fn show_current_fail() {
        let action = Action::parse_from(["show", "--fail-if-empty"]);
        assert_eq!(
            action,
            Action::ShowCurrentNavigator(ShowNav {
                color: String::from("none"),
                fail_if_empty: true
            })
        );
    }

    #[test]
    fn new_navigator() {
        let action = Action::parse_from(["new"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: None,
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_as() {
        let action = Action::parse_from(["new", "--as", "bernd"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_alias() {
        let action = Action::parse_from(["new", "bernd"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_as_and_alias_are_mutually_exclusive() {
        let (_, err) = Action::try_parse_from(["new", "bernd", "--as", "ronny"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn new_navigator_name() {
        let action = Action::parse_from(["new", "--name", "foo"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: None,
                name: Some(String::from("foo")),
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_email() {
        let action = Action::parse_from(["new", "--email", "foo"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: None,
                name: None,
                email: Some(String::from("foo")),
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_all_with_as() {
        let action =
            Action::parse_from(["new", "--as", "bernd", "--name", "foo", "--email", "bar"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: None,
            })
        )
    }

    #[test]
    fn new_navigator_all_with_alias() {
        let action = Action::parse_from(["new", "bernd", "--name", "foo", "--email", "bar"]);
        assert_eq!(
            action,
            Action::NewNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: None,
            })
        )
    }

    #[test]
    fn edit_navigator_name() {
        let action = Action::parse_from(["edit", "--name", "foo"]);
        assert_eq!(
            action,
            Action::EditNavigator(PartialNav {
                id: None,
                name: Some(String::from("foo")),
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn edit_navigator_email() {
        let action = Action::parse_from(["edit", "--email", "foo"]);
        assert_eq!(
            action,
            Action::EditNavigator(PartialNav {
                id: None,
                name: None,
                email: Some(String::from("foo")),
                key: None,
            })
        )
    }

    #[test]
    fn edit_navigator_all_with_as() {
        let action =
            Action::parse_from(["edit", "--as", "bernd", "--name", "foo", "--email", "bar"]);
        assert_eq!(
            action,
            Action::EditNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: None,
            })
        )
    }

    #[test]
    fn edit_navigator_all_with_alias() {
        let action = Action::parse_from(["edit", "bernd", "--name", "foo", "--email", "bar"]);
        assert_eq!(
            action,
            Action::EditNavigator(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: None,
            })
        )
    }

    #[test]
    fn delete_initiates_navigator_selection() {
        let action = Action::parse_from(["delete"]);
        assert_eq!(action, Action::DeleteNavigatorFromSelection);
    }

    #[test]
    fn delete_one_navigator_provided() {
        let action = Action::parse_from(["delete", "foo"]);
        assert_eq!(action, Action::DeleteNavigator(Id::from("foo")));
    }

    #[test]
    fn delete_many_provided_navigators() {
        let action = Action::parse_from(["delete", "foo", "bar"]);
        assert_eq!(
            action,
            Action::DeleteAllNavigators(vec![Id::from("foo"), Id::from("bar")])
        );
    }

    #[test]
    fn as_initiates_selection() {
        let action = Action::parse_from(["as"]);
        assert_eq!(action, Action::DriveAsFromSelection);
    }

    #[test]
    fn as_with_one_arg() {
        let action = Action::parse_from(["as", "bernd"]);
        assert_eq!(action, Action::DriveAs(Id::from("bernd")));
    }

    #[test]
    fn as_only_accepts_one_arg() {
        let (_, err) = Action::try_parse_from(["as", "bernd", "ronny"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TooManyValues);
    }

    #[test]
    fn list_drivers_from_me() {
        let action = Action::parse_from(["me", "list"]);
        assert_eq!(action, Action::ListDrivers);
    }

    #[test]
    fn new_driver() {
        let action = Action::parse_from(["me", "new"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: None,
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_driver_as() {
        let action = Action::parse_from(["me", "new", "--as", "bernd"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_driver_alias() {
        let action = Action::parse_from(["me", "new", "bernd"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: None,
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_driver_as_and_alias_are_mutually_exclusive() {
        let (_, err) = Action::try_parse_from(["me", "new", "bernd", "--as", "ronny"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn new_driver_name() {
        let action = Action::parse_from(["me", "new", "--name", "foo"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: None,
                name: Some(String::from("foo")),
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn new_driver_email() {
        let action = Action::parse_from(["me", "new", "--email", "foo"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: None,
                name: None,
                email: Some(String::from("foo")),
                key: None,
            })
        )
    }

    #[test]
    fn new_driver_key() {
        let action = Action::parse_from(["me", "new", "--key", "foo"]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: None,
                name: None,
                email: None,
                key: Some(String::from("foo")),
            })
        )
    }

    #[test]
    fn new_driver_all_with_as() {
        let action = Action::parse_from([
            "me", "new", "--as", "bernd", "--name", "foo", "--email", "bar", "--key", "baz",
        ]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: Some(String::from("baz")),
            })
        )
    }

    #[test]
    fn new_driver_all_with_alias() {
        let action = Action::parse_from([
            "me", "new", "bernd", "--name", "foo", "--email", "bar", "--key", "baz",
        ]);
        assert_eq!(
            action,
            Action::NewDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: Some(String::from("baz")),
            })
        )
    }

    #[test]
    fn edit_driver_name() {
        let action = Action::parse_from(["me", "edit", "--name", "foo"]);
        assert_eq!(
            action,
            Action::EditDriver(PartialNav {
                id: None,
                name: Some(String::from("foo")),
                email: None,
                key: None,
            })
        )
    }

    #[test]
    fn edit_driver_email() {
        let action = Action::parse_from(["me", "edit", "--email", "foo"]);
        assert_eq!(
            action,
            Action::EditDriver(PartialNav {
                id: None,
                name: None,
                email: Some(String::from("foo")),
                key: None,
            })
        )
    }

    #[test]
    fn edit_driver_key() {
        let action = Action::parse_from(["me", "edit", "--key", "foo"]);
        assert_eq!(
            action,
            Action::EditDriver(PartialNav {
                id: None,
                name: None,
                email: None,
                key: Some(String::from("foo")),
            })
        )
    }

    #[test]
    fn edit_driver_all_with_as() {
        let action = Action::parse_from([
            "me", "edit", "--as", "bernd", "--name", "foo", "--email", "bar", "--key", "baz",
        ]);
        assert_eq!(
            action,
            Action::EditDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: Some(String::from("baz")),
            })
        )
    }

    #[test]
    fn edit_driver_all_with_alias() {
        let action = Action::parse_from([
            "me", "edit", "bernd", "--name", "foo", "--email", "bar", "--key", "baz",
        ]);
        assert_eq!(
            action,
            Action::EditDriver(PartialNav {
                id: Some(String::from("bernd")),
                name: Some(String::from("foo")),
                email: Some(String::from("bar")),
                key: Some(String::from("baz")),
            })
        )
    }

    #[test]
    fn delete_initiates_driver_selection() {
        let action = Action::parse_from(["me", "delete"]);
        assert_eq!(action, Action::DeleteDriverFromSelection);
    }

    #[test]
    fn delete_one_driver_provided() {
        let action = Action::parse_from(["me", "delete", "foo"]);
        assert_eq!(action, Action::DeleteDriver(Id::from("foo")));
    }

    #[test]
    fn delete_many_provided_drivers() {
        let action = Action::parse_from(["me", "delete", "foo", "bar"]);
        assert_eq!(
            action,
            Action::DeleteAllDrivers(vec![Id::from("foo"), Id::from("bar")])
        );
    }

    #[test]
    fn version_flag() {
        let (_, res) = Action::try_parse_from(["--version"]).unwrap_err();
        assert_eq!(res.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn short_version_flag() {
        let (_, res) = Action::try_parse_from(["-V"]).unwrap_err();
        assert_eq!(res.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn help_flag() {
        let (_, res) = Action::try_parse_from(["--help"]).unwrap_err();
        assert_eq!(res.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn short_help_flag() {
        let (_, res) = Action::try_parse_from(["-h"]).unwrap_err();
        assert_eq!(res.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn help_command() {
        let (_, res) = Action::try_parse_from(["help"]).unwrap_err();
        assert_eq!(res.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn verify_cli() {
        Action::app().debug_assert();
    }
}
