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
#![allow(clippy::module_name_repetitions)]

use crate::{args::Action, config::Config, data::Kind};
use console::style;
use eyre::{bail, Result};
use std::slice::from_ref;

mod args;
mod config;
mod data;
mod drive;
mod ui;

fn main() -> Result<()> {
    let action = args::action();
    let mut config = config::load()?;

    let changed = match action {
        Action::DriveFromSelection => select_drive(&config)?,
        Action::DriveWith(id) => drive::run(from_ref(&id), &config)?,
        Action::DriveWithAll(ids) => drive::run(&ids, &config)?,
        Action::DriveAlone => drive::drive_alone()?,
        Action::ListNavigators => list::run(Kind::Navigator, &config),
        Action::ListDrivers => list::run(Kind::Driver, &config),
        Action::ShowCurrentNavigator(show) => drive::current(show),
        Action::NewNavigator(partial) => new::run(Kind::Navigator, &mut config, partial)?,
        Action::EditNavigator(partial) => edit::run(Kind::Navigator, &mut config, partial)?,
        Action::DeleteNavigatorFromSelection => delete::select(Kind::Navigator, &mut config)?,
        Action::DeleteNavigator(id) => delete::run(Kind::Navigator, &mut config, from_ref(&id)),
        Action::DeleteAllNavigators(ids) => delete::run(Kind::Navigator, &mut config, &ids),
        Action::NewDriver(partial) => new::run(Kind::Driver, &mut config, partial)?,
        Action::EditDriver(partial) => edit::run(Kind::Driver, &mut config, partial)?,
        Action::DeleteDriverFromSelection => delete::select(Kind::Driver, &mut config)?,
        Action::DeleteDriver(id) => delete::run(Kind::Driver, &mut config, from_ref(&id)),
        Action::DeleteAllDrivers(ids) => delete::run(Kind::Driver, &mut config, &ids),
        Action::DriveAsFromSelection | Action::DriveAs(_) => {
            bail!("Switching seats not yet implemented")
        }
    };

    if changed {
        config::store(&config)?;
    }

    Ok(())
}

fn select_drive(config: &Config) -> Result<bool> {
    if config.navigators.is_empty() {
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

        return Ok(false);
    }
    drive::select(config)
}

mod list {
    use crate::{
        config::Config,
        data::{Kind, Navigator},
    };

    pub fn run(kind: Kind, config: &Config) -> bool {
        match kind {
            Kind::Navigator => {
                for nav in &config.navigators {
                    print_nav(nav);
                }
            }
            Kind::Driver => {
                for drv in &config.drivers {
                    print_nav(&drv.navigator);
                }
            }
        };
        false
    }

    fn print_nav(nav: &Navigator) {
        println!("{}: {} <{}>", &*nav.alias, nav.name, nav.email);
    }
}

mod new {
    use crate::{
        config::Config,
        data::{Kind, PartialNav},
        ui, Result,
    };

    pub fn run(kind: Kind, config: &mut Config, partial: PartialNav) -> Result<bool> {
        match kind {
            Kind::Navigator => {
                let navigator = ui::complete_new_nav(partial, config)?;
                config.navigators.push(navigator);
            }
            Kind::Driver => {
                let driver = ui::complete_new_drv(partial, config)?;
                config.drivers.push(driver);
            }
        }
        Ok(true)
    }
}

mod edit {
    use crate::{
        config::Config,
        data::{Kind, PartialNav},
        ui, Result,
    };
    use eyre::bail;

    pub fn run(kind: Kind, config: &mut Config, mut new: PartialNav) -> Result<bool> {
        if new.id.is_none() {
            match kind {
                Kind::Navigator => {
                    if config.navigators.is_empty() {
                        bail!("No navigators to edit")
                    }
                }
                Kind::Driver => {
                    if config.drivers.is_empty() {
                        bail!("No drivers to edit")
                    }
                }
            }
            let id = ui::select_id_from(kind, config)?;
            new.id = Some(id.0);
        }
        match kind {
            Kind::Navigator => {
                let navigator = ui::complete_existing_nav(new, config)?;
                let nav = config
                    .navigators
                    .iter_mut()
                    .find(|n| navigator.alias.same_as_nav(n))
                    .expect("validated during complete_existing_nav");
                *nav = navigator;
            }
            Kind::Driver => {
                let driver = ui::complete_existing_drv(new, config)?;
                let drv = config
                    .drivers
                    .iter_mut()
                    .find(|d| driver.navigator.alias.same_as_drv(d))
                    .expect("validated during complete_existing_nav");
                *drv = driver;
            }
        }
        Ok(true)
    }
}

mod delete {
    use crate::{
        config::Config,
        data::{Id, IdRef, Kind},
        ui, Result,
    };

    pub fn select(kind: Kind, config: &mut Config) -> Result<bool> {
        let ids: Vec<Id> = ui::select_ids_from(kind, config, &[])?
            .into_iter()
            .cloned()
            .collect();
        Ok(run(kind, config, &ids))
    }

    pub fn run<I: IdRef>(kind: Kind, config: &mut Config, ids: &[I]) -> bool {
        match kind {
            Kind::Navigator => do_delete(&mut config.navigators, ids),
            Kind::Driver => do_delete(&mut config.drivers, ids),
        }
    }

    fn do_delete<T: IdRef, I: IdRef>(data: &mut Vec<T>, ids: &[I]) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i != data.len() {
            if ids.iter().any(|id| id.id() == data[i].id()) {
                drop(data.remove(i));
                changed = true;
            } else {
                i += 1;
            }
        }
        changed
    }
}
