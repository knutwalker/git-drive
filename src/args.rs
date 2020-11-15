use crate::{
    config::Config,
    data::{Command, Id, Kind, New, Provided},
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
        .arg(
            Arg::new("EDIT")
                .value_name("ID")
                .about(concat!("Edit the specified ", $kind, "(s)"))
                .required(false)
                .last(true)
                .multiple_values(true)
                .min_values(1)
                .possible_values(&$ids),
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
        Some(("with", m)) => Command::Drive(ids(m, "WITH", false)),
        Some(("alone", _)) => Command::Drive(Provided(Some(Vec::new()))),
        Some(("list", m)) => Command::List(if m.subcommand_matches("me").is_none() {
            Kind::Navigator
        } else {
            Kind::Driver
        }),
        Some(("new", m)) => Command::New(Kind::Navigator, new(m)),
        Some(("edit", m)) => Command::Edit(Kind::Navigator, ids(m, "EDIT", true), new(m)),
        Some(("delete", m)) => Command::Delete(Kind::Navigator, ids(m, "DELETE", true)),
        Some(("me", m)) => match m.subcommand() {
            Some(("list", _)) => Command::List(Kind::Driver),
            Some(("new", m)) => Command::New(Kind::Driver, new(m)),
            Some(("edit", m)) => Command::Edit(Kind::Driver, ids(m, "EDIT", true), new(m)),
            Some(("delete", m)) => Command::Delete(Kind::Driver, ids(m, "DELETE", true)),
            Some((c, _)) => invalid_sub(c),
            None => missing_sub(),
        },
        Some((c, _)) => invalid_sub(c),
        None => Command::Drive(Provided(None)),
    };

    command
}
