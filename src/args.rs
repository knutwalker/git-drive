use crate::{
    config::Config,
    data::{Action, Command, Id, Kind, New, Provided},
};
use clap::{
    App,
    AppSettings::SubcommandRequiredElseHelp,
    AppSettings::{
        ColoredHelp, DeriveDisplayOrder, GlobalVersion, InferSubcommands, VersionlessSubcommands,
    },
    Arg, ArgGroup, ArgMatches, Error, ErrorKind,
};

macro_rules! list_app {
    ($kind:literal) => {
        App::new("list").about(concat!("List known ", $kind, "s"))
    };
}

macro_rules! delete_app {
    ($kind:literal, $ids:ident) => {
        App::new("delete")
            .about(concat!(
                "Deletes ",
                $kind,
                "(s), either prompted for, or specified"
            ))
            .arg(
                Arg::new("DELETE")
                    .value_name("ID")
                    .about(concat!("Delete the specified ", $kind, "(s)"))
                    .required(false)
                    .multiple_values(true)
                    .min_values(1)
                    .possible_values(&$ids),
            )
    };
}

macro_rules! nav_args {
    ($kind:literal, $app:expr) => {
        $app.arg(
            Arg::new("as_pos")
                .about(concat!("The id of the ", $kind))
                .value_name("ID")
                .takes_value(true),
        )
        .arg(
            Arg::new("as_opt")
                .about(concat!("The id of the ", $kind))
                .value_name("ID")
                .long("as")
                .takes_value(true),
        )
        .arg(
            Arg::new("name")
                .about(concat!("The name of the ", $kind))
                .value_name("NAME")
                .long("name")
                .takes_value(true),
        )
        .arg(
            Arg::new("email")
                .about(concat!("The email of the ", $kind))
                .value_name("EMAIL")
                .long("email")
                .takes_value(true),
        )
        .group(
            ArgGroup::new("as")
                .args(["as_opt", "as_pos"].as_ref())
                .multiple(false)
                .required(false),
        )
    };
}

macro_rules! edit_app {
    ($kind:literal, $ids:ident) => {
        nav_args!(
            $kind,
            App::new("edit").about(concat!(
                "Edit ",
                $kind,
                "(s), either prompted for, or specified"
            ))
        )
    };
}

macro_rules! new_app {
    ($kind:literal) => {
        nav_args!(
            $kind,
            App::new("new").about(concat!(
                "Add a new ",
                $kind,
                ", either prompted for, or specified"
            ))
        )
    };
}

pub(crate) fn command(config: &Config) -> Command {
    let drivers = config
        .drivers
        .iter()
        .map(|s| s.navigator.alias.as_ref())
        .collect::<Vec<_>>();

    let navigators = config
        .navigators
        .iter()
        .map(|s| s.alias.as_ref())
        .collect::<Vec<_>>();

    let args = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .global_setting(ColoredHelp)
        .global_setting(DeriveDisplayOrder)
        .global_setting(GlobalVersion)
        .global_setting(InferSubcommands)
        .global_setting(VersionlessSubcommands)
        .subcommand(
            App::new("with").arg(
                Arg::new("WITH")
                    .value_name("NAVIGATOR")
                    .about("Start driving with the specified navigator(s)")
                    .required(true)
                    .multiple_values(true)
                    .min_values(1)
                    .possible_values(&navigators),
            ),
        )
        .subcommand(App::new("alone").about("Start driving alone"))
        .subcommand(
            App::new("show").about("Show current navigators").arg(
                Arg::new("color")
                    .short('c')
                    .long("color")
                    .visible_alias("colour")
                    .default_value("none")
                    .overrides_with("color")
                    .min_values(0)
                    .require_equals(true)
                    .default_missing_value("cyan")
                    .about("How to colorize each id"),
            ),
        )
        .subcommand(list_app!("navigator").subcommand(App::new("me")))
        .subcommand(new_app!("navigator"))
        .subcommand(edit_app!("navigator", navigators))
        .subcommand(delete_app!("navigator", navigators))
        .subcommand(
            App::new("me")
                .about("Operate on the driver instead of the navigator")
                .setting(SubcommandRequiredElseHelp)
                .subcommand(list_app!("driver"))
                .subcommand(
                    new_app!("driver").arg(
                        Arg::new("signingkey")
                            .about("The signingkey of the driver")
                            .value_name("signingkey")
                            .long("key")
                            .visible_alias("signingkey")
                            .takes_value(true),
                    ),
                )
                .subcommand(edit_app!("driver", drivers))
                .subcommand(delete_app!("driver", drivers)),
        )
        .subcommand(
            App::new("as").about("Change driver seat").arg(
                Arg::new("AS")
                    .value_name("DRIVER")
                    .about("Start driving as one of the specified drivers")
                    .min_values(1)
                    .max_values(1)
                    .required(true)
                    .multiple_values(false)
                    .possible_values(&drivers),
            ),
        )
        .get_matches();

    fn ids(matches: &ArgMatches, var: &str, empty_is_none: bool) -> Provided {
        let ids = matches.values_of(var);
        let mut ids = ids.map(|v| v.map(Id::from).collect::<Vec<_>>());
        if empty_is_none {
            ids = ids.filter(|v| !v.is_empty())
        }
        Provided(ids)
    }

    fn new(matches: &ArgMatches) -> New {
        New {
            id: matches.value_of("as").map(String::from),
            name: matches.value_of("name").map(String::from),
            email: matches.value_of("email").map(String::from),
            key: matches.value_of("signingkey").map(String::from),
        }
    }

    fn invalid_sub(c: &str) -> ! {
        Error::with_description(
            format!("Unknown command: {}", c),
            ErrorKind::InvalidSubcommand,
        )
        .exit()
    }

    fn missing_sub() -> ! {
        Error::with_description("Missing command".to_string(), ErrorKind::MissingSubcommand).exit()
    }

    let command = match args.subcommand() {
        Some(("with", m)) => Command::nav(Action::Drive(ids(m, "WITH", false))),
        Some(("alone", _)) => Command::nav(Action::Drive(Provided(Some(Vec::new())))),
        Some(("show", m)) => Command::nav(Action::Show(String::from(
            m.value_of("color").expect("has a default value"),
        ))),
        Some(("list", m)) => Command::new(
            if m.subcommand_matches("me").is_none() {
                Kind::Navigator
            } else {
                Kind::Driver
            },
            Action::List,
        ),
        Some(("new", m)) => Command::nav(Action::New(new(m))),
        Some(("edit", m)) => Command::nav(Action::Edit(new(m))),
        Some(("delete", m)) => Command::nav(Action::Delete(ids(m, "DELETE", true))),
        Some(("me", m)) => match m.subcommand() {
            Some(("list", _)) => Command::drv(Action::List),
            Some(("new", m)) => Command::drv(Action::New(new(m))),
            Some(("edit", m)) => Command::drv(Action::Edit(new(m))),
            Some(("delete", m)) => Command::drv(Action::Delete(ids(m, "DELETE", true))),
            Some((c, _)) => invalid_sub(c),
            None => missing_sub(),
        },
        Some(("as", m)) => Command::drv(Action::Change(ids(m, "AS", true))),
        Some((c, _)) => invalid_sub(c),
        None => Command::nav(Action::Drive(Provided(None))),
    };

    command
}
