use crate::{
    data::{Action, Command, Id, New, Provided},
    Result,
};
use clap::{AppSettings::DeriveDisplayOrder, Args, IntoApp, Parser, Subcommand};

pub(crate) fn print_help_stderr() -> Result<()> {
    let mut out = std::io::stderr();
    let mut app = AppArgs::command();
    app.write_long_help(&mut out)?;
    Ok(())
}

pub(crate) fn command() -> Command {
    parse_args()
}

fn parse_args() -> Command {
    let args = AppArgs::parse();
    match args.cmd {
        None => Command::nav(Action::Drive(Provided(None))),
        Some(cmd) => match cmd {
            NavigatorCommand::With { ids } => Command::nav(Action::Drive(ids.into())),
            NavigatorCommand::Delete { ids } => Command::nav(Action::Delete(ids.into())),
            NavigatorCommand::As { ids } => Command::drv(Action::Change(ids.into())),
            NavigatorCommand::Alone => Command::nav(Action::Drive(Provided(Some(Vec::new())))),
            NavigatorCommand::Show {
                color,
                fail_if_empty,
            } => {
                let color = color
                    .map(|value| value.unwrap_or_else(|| "cyan".to_string()))
                    .unwrap_or_else(|| "none".to_string());
                Command::nav(Action::Show(color, fail_if_empty))
            }
            NavigatorCommand::List { cmd } => {
                cmd.map_or(Command::nav(Action::List), |_| Command::drv(Action::List))
            }
            NavigatorCommand::New { id } => Command::nav(Action::New((id, None).into())),
            NavigatorCommand::Edit { id } => Command::nav(Action::Edit((id, None).into())),
            NavigatorCommand::Me { cmd } => match cmd {
                DriverCommand::List => Command::drv(Action::List),
                DriverCommand::New { id, key } => Command::drv(Action::New((id, Some(key)).into())),
                DriverCommand::Edit { id, key } => {
                    Command::drv(Action::Edit((id, Some(key)).into()))
                }
                DriverCommand::Delete { ids } => Command::drv(Action::Delete(ids.into())),
            },
        },
    }
}

#[derive(Parser, Clone, Debug)]
#[clap(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    propagate_version = true,
    infer_long_args = true,
    infer_subcommands = true,
    subcommand_required = false,
    global_setting = DeriveDisplayOrder
)]
struct AppArgs {
    #[clap(subcommand)]
    cmd: Option<NavigatorCommand>,
}

#[derive(Subcommand, Clone, Debug)]
enum NavigatorCommand {
    /// Start driving with the specified navigator(s)
    With {
        /// The navigators
        #[clap(min_values = 1, multiple_values = true, required = true)]
        ids: Vec<String>,
    },

    /// Deletes navigator(s), either prompted for, or specified
    Delete {
        /// The navigators
        #[clap(min_values = 0)]
        ids: Vec<String>,
    },

    /// Change driver seat
    As {
        /// The navigators
        #[clap(min_values = 0)]
        ids: Vec<String>,
    },

    /// Start driving alone
    Alone,

    /// Show current navigators
    Show {
        /// The color in which to print the current navigators
        #[clap(short, long, visible_alias = "colour")]
        color: Option<Option<String>>,

        /// If set, fail the process if there are no current navigators
        #[clap(long)]
        fail_if_empty: bool,
    },

    /// List known navigators
    List {
        #[clap(subcommand)]
        cmd: Option<ListCommand>,
    },

    /// Add a new navigator, either prompted for, or specified
    New {
        #[clap(flatten)]
        id: IdentityArgs,
    },

    ///Edit navigator(s), either prompted for, or specified
    Edit {
        #[clap(flatten)]
        id: IdentityArgs,
    },

    /// Operate on the driver instead of the navigator
    Me {
        #[clap(subcommand)]
        cmd: DriverCommand,
    },
}

#[derive(Args, Clone, Debug)]
struct IdentityArgs {
    /// The identifier to use for the author's entry
    #[clap(long, conflicts_with = "alias")]
    r#as: Option<String>,

    /// The author's name
    #[clap(long)]
    name: Option<String>,

    /// The author's email
    #[clap(long)]
    email: Option<String>,

    /// The identifier to use for the author's entry
    #[clap(conflicts_with = "as")]
    alias: Option<String>,
}

#[derive(Args, Clone, Debug)]
struct KeyArgs {
    /// The signing key to use
    #[clap(long, visible_alias = "signingkey")]
    key: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
enum DriverCommand {
    /// List known drivers
    List,

    /// Add a new driver, either prompted for, or specified
    New {
        #[clap(flatten)]
        id: IdentityArgs,

        #[clap(flatten)]
        key: KeyArgs,
    },
    /// Edit driver(s), either prompted for, or specified
    Edit {
        #[clap(flatten)]
        id: IdentityArgs,

        #[clap(flatten)]
        key: KeyArgs,
    },
    /// Deletes driver(s), either prompted for, or specified
    Delete {
        #[clap(min_values = 0)]
        ids: Vec<String>,
    },
}

#[derive(Subcommand, Clone, Debug)]
enum ListCommand {
    /// It's me
    Me,
}

impl From<Vec<String>> for Provided {
    fn from(ids: Vec<String>) -> Self {
        let ids = ids.into_iter().map(Id).collect::<Vec<_>>();
        let ids = if ids.is_empty() { None } else { Some(ids) };
        Provided(ids)
    }
}

impl From<(IdentityArgs, Option<KeyArgs>)> for New {
    fn from(
        (
            IdentityArgs {
                r#as,
                name,
                email,
                alias,
            },
            key,
        ): (IdentityArgs, Option<KeyArgs>),
    ) -> Self {
        let id = r#as.or(alias);
        let key = key.and_then(|k| k.key);

        New {
            id,
            name,
            email,
            key,
        }
    }
}
