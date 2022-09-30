use std::convert::TryInto;

use crate::{
    data::{Action, Id, PartialNav, ShowNav},
    Result,
};
use clap::{AppSettings::DeriveDisplayOrder, Args, IntoApp, Parser, Subcommand};

pub(crate) fn print_help_stderr() -> Result<()> {
    let mut out = std::io::stderr();
    let mut app = AppArgs::command();
    app.write_long_help(&mut out)?;
    Ok(())
}

pub(crate) fn action() -> Action {
    parse_args()
}

fn parse_args() -> Action {
    let args = AppArgs::parse();
    match args.cmd {
        None => Action::DriveFromSelection,
        Some(cmd) => match cmd {
            NavigatorCommand::With { ids } => fold_map_items(
                ids,
                Id,
                Action::DriveAlone,
                Action::DriveWith,
                Action::DriveWithAll,
            ),
            NavigatorCommand::Delete { ids } => fold_map_items(
                ids,
                Id,
                Action::DeleteNavigatorFromSelection,
                Action::DeleteNavigator,
                Action::DeleteAllNavigators,
            ),
            NavigatorCommand::As { ids } => fold_map_items(
                ids,
                Id,
                Action::DriveAsFromSelection,
                Action::DriveAs,
                |_| panic!("Cannot drive as multiple drivers"),
            ),
            NavigatorCommand::Alone => Action::DriveAlone,
            NavigatorCommand::Show {
                color,
                fail_if_empty,
            } => {
                let color = color
                    .map(|value| value.unwrap_or_else(|| "cyan".to_string()))
                    .unwrap_or_else(|| "none".to_string());
                Action::ShowCurrentNavigator(ShowNav {
                    color,
                    fail_if_empty,
                })
            }
            NavigatorCommand::List { cmd } => match cmd {
                Some(_) => Action::ListDrivers,
                None => Action::ListNavigators,
            },
            NavigatorCommand::New { id } => Action::NewNavigator((id, None).into()),
            NavigatorCommand::Edit { id } => Action::EditNavigator((id, None).into()),
            NavigatorCommand::Me { cmd } => match cmd {
                DriverCommand::List => Action::ListDrivers,
                DriverCommand::New { id, key } => Action::NewDriver((id, Some(key)).into()),
                DriverCommand::Edit { id, key } => Action::EditDriver((id, Some(key)).into()),
                DriverCommand::Delete { ids } => fold_map_items(
                    ids,
                    Id,
                    Action::DeleteDriverFromSelection,
                    Action::DeleteDriver,
                    Action::DeleteAllDrivers,
                ),
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
        #[clap(min_values = 0, max_values = 1)]
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

fn fold_map_items<T, R>(
    items: Vec<String>,
    conv: impl Fn(String) -> T,
    empty: R,
    one: impl FnOnce(T) -> R,
    many: impl FnOnce(Vec<T>) -> R,
) -> R {
    // one is the most common case
    match <Vec<String> as TryInto<[String; 1]>>::try_into(items) {
        Ok([item]) => one(conv(item)),
        Err(items) if items.is_empty() => empty,
        Err(items) => many(items.into_iter().map(&conv).collect()),
    }
}

impl From<(IdentityArgs, Option<KeyArgs>)> for PartialNav {
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

        PartialNav {
            id,
            name,
            email,
            key,
        }
    }
}
