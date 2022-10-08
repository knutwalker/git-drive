/*!

Support for switching git authors and co-authors

# Usage

```bash
# Prompt for a navigator / co-author, or a list thereof, and prepare a new drive
git drive

# Start driving with the specified navigator(s)
git drive with user1 [user2...]

# Start driving alone
git drive alone

# Show current navigators
git drive show [--color[=<color>]]

# List known navigators
git drive list

# Edit navigator(s), either prompted for, or specified
git drive edit [user1 [user2...]]

# Add new navigator, either prompted for, or specified
git drive new [[--as] user --name User --email Email]

# Delets navigator(s), either prompted for, or specified
git drive delete [user1 [user2...]]

# List known aliases for the driver
git drive me list

# Edit driver, either prompted for, or specified
git drive me edit [user1 [user2...]]

# Add new driver, either prompted for, or specified
git drive me new [[--as] user --name User --email Email --key GPGSigningKey]

# Delets a driver, either prompted for, or specified
git drive me delete [user1 [user2...]]

# Change identity while driving
git drive as alias
```

*/
#![warn(clippy::all, clippy::nursery)]
#![warn(clippy::cargo, clippy::pedantic)]
#![warn(
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    missing_copy_implementations,
    missing_debug_implementations,
    no_mangle_generic_items,
    non_shorthand_field_patterns,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    rust_2018_idioms,
    rust_2021_compatibility,
    rust_2021_incompatible_or_patterns,
    rust_2021_incompatible_closure_captures,
    rust_2021_prefixes_incompatible_syntax,
    rust_2021_prelude_collisions,
    trivial_casts,
    trivial_numeric_casts,
    unconditional_recursion,
    unsafe_op_in_unsafe_fn,
    unused_allocation,
    unused_comparisons,
    unused_crate_dependencies,
    unused_extern_crates,
    unused_import_braces,
    unused_parens,
    unused_qualifications,
    unused,
    while_true
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::bool_assert_comparison,
    clippy::missing_const_for_fn
)]

use crate::{
    args::Action,
    config::Config,
    data::{Kind, Modification},
};
use console::style;
use eyre::{bail, Result};
use std::slice::from_ref;

mod args;
mod config;
mod data;
mod delete;
mod drive;
mod edit;
mod list;
mod new;
mod ui;

fn main() -> Result<()> {
    let action = args::action();
    let mut config = config::load()?;
    let ui = ui::ui();

    let changed = match action {
        Action::DriveFromSelection => select_drive(&config)?,
        Action::DriveWith(id) => drive::run(from_ref(&id), &config)?,
        Action::DriveWithAll(ids) => drive::run(&ids, &config)?,
        Action::DriveAlone => drive::alone()?,
        Action::ListNavigators => list::run(Kind::Navigator, &config),
        Action::ListDrivers => list::run(Kind::Driver, &config),
        Action::ShowCurrentNavigator(show) => drive::current(show),
        Action::NewNavigator(partial) => new::run(ui, Kind::Navigator, &mut config, partial)?,
        Action::EditNavigator(partial) => edit::run(ui, Kind::Navigator, &mut config, partial)?,
        Action::DeleteNavigatorFromSelection => delete::select(ui, Kind::Navigator, &mut config)?,
        Action::DeleteNavigator(id) => delete::run(Kind::Navigator, &mut config, from_ref(&id)),
        Action::DeleteAllNavigators(ids) => delete::run(Kind::Navigator, &mut config, &ids),
        Action::NewDriver(partial) => new::run(ui, Kind::Driver, &mut config, partial)?,
        Action::EditDriver(partial) => edit::run(ui, Kind::Driver, &mut config, partial)?,
        Action::DeleteDriverFromSelection => delete::select(ui, Kind::Driver, &mut config)?,
        Action::DeleteDriver(id) => delete::run(Kind::Driver, &mut config, from_ref(&id)),
        Action::DeleteAllDrivers(ids) => delete::run(Kind::Driver, &mut config, &ids),
        Action::DriveAsFromSelection | Action::DriveAs(_) => {
            bail!("Switching seats not yet implemented")
        }
    };

    if changed == Modification::Changed {
        config::store(&config)?;
    }

    Ok(())
}

fn select_drive(config: &Config) -> Result<Modification> {
    if let Some(changed) = drive::select(ui::ui(), config)? {
        Ok(changed)
    } else {
        use std::fmt::Write;
        let mut pre_help = String::with_capacity(128);
        writeln!(pre_help, "{}", style("No navigators found").yellow())?;
        writeln!(pre_help)?;
        writeln!(
            pre_help,
            "You haven't added any navigators to the system yet."
        )?;
        writeln!(
            pre_help,
            "Please add a new navigator with {}",
            style(concat!(env!("CARGO_PKG_NAME"), " new")).green()
        )?;
        eprintln!("{}", pre_help);
        eprintln!();
        eprintln!();
        args::print_help_stderr()?;
        Ok(Modification::Unchanged)
    }
}
