use crate::{
    data::{Action, Command, Id, Kind, New, Provided},
    Result,
};
use console::{style, Term};

pub(crate) fn help(to: &Term) -> String {
    let style = to.style();

    let help = format!(
                        r#"
    {name} {version}
{description}

{usage}:
    git-drive [COMMAND]

{commands}:
    {with}        Start driving with the specified navigator(s)
    {alone}       Start driving alone
    {show}        Show current navigators
    {list}        List known navigators
    {new}         Add a new navigator, either prompted for, or specified
    {edit}        Edit navigator(s), either prompted for, or specified
    {delete}      Deletes navigator(s), either prompted for, or specified
    {me} <CMD>    Operate on the driver instead of the navigator
      {me} {list}   List known drivers
      {me} {new}    Add a new driver, either prompted for, or specified
      {me} {edit}   Edit driver(s), either prompted for, or specified
      {me} {delete} Deletes driver(s), either prompted for, or specified
    {as}          Change driver seat
    {help}        Prints this message or the help of the given subcommand(s)

FLAGS:
    {h_short}, {help_long}       Prints help information
    {v_short}, {version_long}    Prints version information

"#,
        name = style.apply_to(env!("CARGO_PKG_NAME")).green().for_stderr(),
        version = env!("CARGO_PKG_VERSION"),
        description = env!("CARGO_PKG_DESCRIPTION"),
        usage = style.apply_to("USAGE").yellow(),
        commands = style.apply_to("COMMANDS").yellow(),
        with = style.apply_to("with").green(),
        alone = style.apply_to("alone").green(),
        show = style.apply_to("show").green(),
        list = style.apply_to("list").green(),
        new = style.apply_to("new").green(),
        edit = style.apply_to("edit").green(),
        delete = style.apply_to("delete").green(),
        me = style.apply_to("me").green(),
        as = style.apply_to("as").green(),
        help = style.apply_to("help").green(),
        h_short = style.apply_to("-h").green(),
        help_long = style.apply_to("--help").green(),
        v_short = style.apply_to("-V").green(),
        version_long = style.apply_to("--version").green(),
    );

    help
}

pub(crate) fn print_help_stderr() -> Result<()> {
    print_help(Term::stderr())
}

pub(crate) fn print_help(to: Term) -> Result<()> {
    to.write_str(help(&to).as_str())?;
    Ok(())
}

pub(crate) fn command() -> Command {
    match parse_args() {
        Ok(c) => c,
        Err(e) => e.handle(),
    }
}

fn parse_args() -> Result<Command, ArgsErr> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        return Err(ArgsErr::HelpRequested);
    }
    if args.contains(["-V", "--version"]) {
        return Err(ArgsErr::VersionRequested);
    }

    let cmd = match args.subcommand()? {
        Some(cmd) => cmd,
        None => {
            args.finish()?;
            return Ok(Command::nav(Action::Drive(Provided(None))));
        }
    };

    let command = match cmd.as_str() {
        "w" | "with" => Command::nav(Action::Drive(must_ids(args.free()?)?)),
        "d" | "delete" => Command::nav(Action::Delete(ids(args.free()?))),
        "as" => Command::drv(Action::Change(ids(args.free()?))),
        "h" | "help" => return Err(ArgsErr::HelpRequested),

        other => {
            let mut command = match other {
                "a" | "alone" => Command::nav(Action::Drive(Provided(Some(Vec::new())))),
                "s" | "show" => {
                    let color = match args.opt_value_from_str(["-c", "--color"]) {
                        Ok(Some(c)) => c,
                        Ok(None) => String::from("none"),
                        Err(pico_args::Error::OptionWithoutAValue(_)) => String::from("cyan"),
                        Err(e) => return Err(e.into()),
                    };
                    Command::nav(Action::Show(color))
                }
                "l" | "list" => match args.subcommand()?.as_deref() {
                    None => Command::nav(Action::List),
                    Some("me") => Command::drv(Action::List),
                    Some(other) => return Err(ArgsErr::UnknownCommand(format!("list {}", other))),
                },
                "n" | "new" => Command::nav(Action::New(new_from(&mut args, false)?)),
                "e" | "edit" => Command::nav(Action::Edit(new_from(&mut args, false)?)),

                "me" => match args.subcommand()?.ok_or(ArgsErr::MissingCommand)?.as_str() {
                    "l" | "list" => Command::drv(Action::List),
                    "n" | "new" => Command::drv(Action::New(new_from(&mut args, true)?)),
                    "e" | "edit" => Command::drv(Action::Edit(new_from(&mut args, true)?)),
                    "d" | "delete" => Command::drv(Action::Delete(Provided(None))),
                    other => return Err(ArgsErr::UnknownCommand(format!("me {}", other))),
                },
                _ => return Err(ArgsErr::UnknownCommand(cmd)),
            };
            if let Command {
                kind: Kind::Driver,
                action: Action::Delete(provided),
            } = &mut command
            {
                *provided = ids(args.free()?);
            } else {
                args.finish()?;
            }

            command
        }
    };

    Ok(command)
}

fn ids(ids: Vec<String>) -> Provided {
    provided(ids, true).unwrap()
}

fn must_ids(ids: Vec<String>) -> Result<Provided, ArgsErr> {
    provided(ids, false)
}

fn provided(ids: Vec<String>, allow_empty: bool) -> Result<Provided, ArgsErr> {
    let ids = ids.into_iter().map(Id).collect::<Vec<_>>();
    let ids = if ids.is_empty() {
        if !allow_empty {
            return Err(ArgsErr::EmptyWith);
        }
        None
    } else {
        Some(ids)
    };
    Ok(Provided(ids))
}

fn new_from(args: &mut pico_args::Arguments, accept_key: bool) -> Result<New, ArgsErr> {
    let alias1 = args.opt_value_from_str("--as")?;
    let name = args.opt_value_from_str("--name")?;
    let email = args.opt_value_from_str("--email")?;

    let key = if accept_key {
        let key1 = args.opt_value_from_str("--key")?;
        let key2 = args.opt_value_from_str("--signingkey")?;
        match (key1, key2) {
            (None, None) => None,
            (Some(k), None) => Some(k),
            (None, Some(k)) => Some(k),
            (Some(_), Some(_)) => return Err(ArgsErr::DuplicateFlags("--key", "--signingkey")),
        }
    } else {
        None
    };

    let alias2 = args.subcommand()?;

    let id = match (alias1, alias2) {
        (None, None) => None,
        (Some(id), None) => Some(id),
        (None, Some(id)) => Some(id),
        (Some(_), Some(_)) => return Err(ArgsErr::DuplicateFlags("<as>", "--as")),
    };

    let new = New {
        id,
        name,
        email,
        key,
    };
    Ok(new)
}

#[derive(Debug)]
pub enum ArgsErr {
    HelpRequested,
    VersionRequested,
    MissingCommand,
    UnknownCommand(String),
    DuplicateFlags(&'static str, &'static str),
    EmptyWith,
    ParserErr(pico_args::Error),
}

impl ArgsErr {
    pub fn handle(self) -> ! {
        let term = if self.to_stderr() {
            Term::stderr()
        } else {
            Term::stdout()
        };
        let display = format!("{}", self);
        let _ = term.write_str(&display);
        if self.suggest_help() {
            let _ = term.write_line("");
            let suggestion = format!(
                "For more information try {help}",
                help = style("--help").green()
            );
            let _ = term.write_line(&suggestion);
        }

        std::process::exit(self.exit_code())
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            ArgsErr::HelpRequested => 0,
            ArgsErr::VersionRequested => 0,
            ArgsErr::MissingCommand => 1,
            ArgsErr::UnknownCommand(_) => 2,
            ArgsErr::DuplicateFlags(_, _) => 3,
            ArgsErr::EmptyWith => 4,
            ArgsErr::ParserErr(pico_args::Error::UnusedArgsLeft(_)) => 5,
            ArgsErr::ParserErr(pico_args::Error::OptionWithoutAValue(_)) => 6,
            ArgsErr::ParserErr(_) => 7,
        }
    }

    pub fn to_stderr(&self) -> bool {
        match self {
            ArgsErr::HelpRequested | ArgsErr::VersionRequested => false,
            _ => true,
        }
    }

    pub fn suggest_help(&self) -> bool {
        match self {
            ArgsErr::HelpRequested | ArgsErr::VersionRequested | ArgsErr::MissingCommand => false,
            _ => true,
        }
    }
}

impl std::fmt::Display for ArgsErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgsErr::HelpRequested => write!(f, "{}", help(&Term::stdout())),
            ArgsErr::VersionRequested => writeln!(
                f,
                concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
            ),
            ArgsErr::MissingCommand => write!(f, "{}", help(&Term::stderr())),
            ArgsErr::UnknownCommand(c) => writeln!(
                f,
                "{error} The command '{cmd}' was not recognized",
                error = style("error:").red(),
                cmd = style(c).yellow(),
            ),
            ArgsErr::DuplicateFlags(a, b) => writeln!(
                f,
                "{error} The argument '{a}' cannot be used with '{b}'",
                error = style("error:").red(),
                a = style(a).yellow(),
                b = style(b).yellow(),
            ),
            ArgsErr::EmptyWith => writeln!(
                f,
                "{error} The command '{with}' required at least one argument",
                error = style("error:").red(),
                with = style("with").yellow(),
            ),
            ArgsErr::ParserErr(pico_args::Error::UnusedArgsLeft(args)) => {
                write!(
                    f,
                    "{error} Unknown arguments in this context:",
                    error = style("error:").red()
                )?;
                for arg in args {
                    write!(f, " {}", style(arg).yellow())?;
                }
                writeln!(f)
            }
            ArgsErr::ParserErr(pico_args::Error::OptionWithoutAValue(arg)) => writeln!(
                f,
                "{error} The argument {arg} requires a value not non was supplied",
                error = style("error:").red(),
                arg = style(arg).yellow(),
            ),
            ArgsErr::ParserErr(e) => writeln!(f, "{error} {}", e, error = style("error:").red()),
        }
    }
}

impl From<pico_args::Error> for ArgsErr {
    fn from(v: pico_args::Error) -> Self {
        ArgsErr::ParserErr(v)
    }
}

impl std::error::Error for ArgsErr {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArgsErr::ParserErr(e) => Some(e),
            _ => None,
        }
    }
}
