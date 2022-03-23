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
#![warn(unused_crate_dependencies)]

use config::Config;
use console::style;
use data::{Action, Command, Provided};
use eyre::{bail, Result};

mod args;
mod config;
mod data;
mod drive;
mod io;

fn main() -> Result<()> {
    let command = args::command();
    let mut config = config::load()?;

    let Command { kind, action } = command;
    let changed = match action {
        Action::Drive(Provided(None)) => select_drive(&config)?,
        Action::Drive(Provided(Some(ids))) => drive::run(ids, &config)?,
        Action::Change(Provided(None)) => bail!("Switching seats not yet implemented"),
        Action::Change(Provided(Some(_))) => bail!("Switching seats not yet implemented"),
        Action::Show(color, fail) => drive::current(color, fail),
        Action::List => list::run(kind, &config),
        Action::New(new) => new::run(kind, &mut config, new)?,
        Action::Edit(new) => edit::run(kind, &mut config, new)?,
        Action::Delete(Provided(Some(ids))) => delete::run(kind, &mut config, ids),
        Action::Delete(_) => delete::select(kind, &mut config)?,
    };

    if changed {
        config::store(config)?;
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

    pub(crate) fn run(kind: Kind, config: &Config) -> bool {
        match kind {
            Kind::Navigator => {
                for nav in &config.navigators {
                    print_nav(nav)
                }
            }
            Kind::Driver => {
                for drv in &config.drivers {
                    print_nav(&drv.navigator)
                }
            }
        };
        false
    }

    fn print_nav(nav: &Navigator) {
        println!("{}: {} <{}>", &*nav.alias, nav.name, nav.email)
    }
}

mod new {
    use crate::{
        config::Config,
        data::{Kind, New},
        io, Result,
    };

    pub(crate) fn run(kind: Kind, config: &mut Config, new: New) -> Result<bool> {
        match kind {
            Kind::Navigator => {
                let navigator = io::complete_new_nav(new, config)?;
                config.navigators.push(navigator);
            }
            Kind::Driver => {
                let driver = io::complete_new_drv(new, config)?;
                config.drivers.push(driver);
            }
        }
        Ok(true)
    }
}

mod edit {
    use crate::{
        config::Config,
        data::{Kind, New},
        io, Result,
    };
    use eyre::bail;

    pub(crate) fn run(kind: Kind, config: &mut Config, mut new: New) -> Result<bool> {
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
            let id = io::select_id_from(kind, config)?;
            new.id = Some(id.0);
        }
        match kind {
            Kind::Navigator => {
                let navigator = io::complete_existing_nav(new, config)?;
                let nav = config
                    .navigators
                    .iter_mut()
                    .find(|n| navigator.alias.same_as_nav(n))
                    .expect("validated during complete_existing_nav");
                *nav = navigator;
            }
            Kind::Driver => {
                let driver = io::complete_existing_drv(new, config)?;
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
        data::{IdRef, Kind},
        io, Result,
    };

    pub(crate) fn select(kind: Kind, config: &mut Config) -> Result<bool> {
        let ids = io::select_ids_from(kind, config, Vec::new())?
            .into_iter()
            .cloned()
            .collect();
        Ok(run(kind, config, ids))
    }

    pub(crate) fn run<A: IdRef>(kind: Kind, config: &mut Config, ids: Vec<A>) -> bool {
        match kind {
            Kind::Navigator => do_delete(&mut config.navigators, ids),
            Kind::Driver => do_delete(&mut config.drivers, ids),
        }
    }

    fn do_delete<A: IdRef, T: IdRef>(data: &mut Vec<T>, ids: Vec<A>) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i != data.len() {
            if ids.iter().any(|id| id.id() == data[i].id()) {
                let _ = data.remove(i);
                changed = true;
            } else {
                i += 1;
            }
        }
        changed
    }
}
