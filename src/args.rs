use crate::{
    data::{Action, Id, PartialNav, ShowNav},
    Result,
};
use clap::{
    builder::ValueParser, AppSettings::DeriveDisplayOrder, Arg, ArgAction, ArgMatches, Command,
};

pub(crate) fn print_help_stderr() -> std::io::Result<()> {
    let mut out = std::io::stderr();
    let mut app = Action::app();
    app.write_long_help(&mut out)
}

pub(crate) fn action() -> Action {
    Action::parse()
}

impl Action {
    fn parse() -> Self {
        let mut app = Self::app();
        let matches = app.get_matches_mut();
        match Self::action_from_matches(matches) {
            Ok(action) => action,
            Err(e) => {
                // Since this is more of a development-time error,
                // we aren't doing as fancy of a quit as `get_matches`
                e.format(&mut app).exit()
            }
        }
    }
    fn app() -> Command<'static> {
        Command::new(env!("CARGO_PKG_NAME"))
            .version(env!("CARGO_PKG_VERSION"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .propagate_version(true)
            .infer_long_args(true)
            .infer_subcommands(true)
            .subcommand_required(false)
            .global_setting(DeriveDisplayOrder)
            .subcommand(
                Command::new("with")
                    .arg(Self::ids_arg().min_values(1))
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
                            .default_missing_value("cyan")
                            .default_value("none")
                            .value_parser(ValueParser::string())
                            .action(ArgAction::StoreValue)
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
                    .arg(Self::ids_arg().min_values(0))
                    .about("Deletes navigator(s), either prompted for, or specified"),
            )
            .subcommand(
                Command::new("as")
                    .arg(Self::ids_arg().min_values(0).max_values(1))
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
                            .arg(
                                Self::ids_arg()
                                    // .action(ArgAction::StoreValue)
                                    // .multiple_values(false)
                                    // .required(false)
                                    .min_values(0)
                                    .help("The drivers"),
                            )
                            .about("Deletes driver(s), either prompted for, or specified"),
                    )
                    .setting(clap::AppSettings::SubcommandRequiredElseHelp)
                    .about("Operate on the driver instead of the navigator"),
            )
    }

    fn ids_arg() -> Arg<'static> {
        Arg::new("ids")
            .value_name("IDS")
            .takes_value(true)
            .multiple_values(true)
            .value_parser(ValueParser::string())
            .action(ArgAction::Append)
            .required(true)
            .help("The navigators")
    }

    fn key_arg() -> Arg<'static> {
        Arg::new("key")
            .long("key")
            .visible_alias("signingkey")
            .value_name("KEY")
            .takes_value(true)
            .value_parser(ValueParser::string())
            .action(ArgAction::StoreValue)
            .help("The signing key to use")
    }

    fn partial_nav_args() -> [Arg<'static>; 4] {
        [
            Arg::new("as")
                .long("as")
                .value_name("AS")
                .takes_value(true)
                .value_parser(ValueParser::string())
                .action(ArgAction::StoreValue)
                .help("The identifier to use for the author's entry")
                .conflicts_with("alias"),
            Arg::new("name")
                .long("name")
                .value_name("NAME")
                .takes_value(true)
                .value_parser(ValueParser::string())
                .action(ArgAction::StoreValue)
                .help("The author's name"),
            Arg::new("email")
                .long("email")
                .value_name("EMAIL")
                .takes_value(true)
                .value_parser(ValueParser::string())
                .action(ArgAction::StoreValue)
                .help("The author's email"),
            Arg::new("alias")
                .value_name("ALIAS")
                .takes_value(true)
                .value_parser(ValueParser::string())
                .action(ArgAction::StoreValue)
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
                        clap::ErrorKind::MissingSubcommand,
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
            clap::ErrorKind::UnrecognizedSubcommand,
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
