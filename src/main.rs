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

# List known navigators
git drive list

# Edit navigator(s), either prompted for, or specified
git drive edit [user1 [user2...]]

# Add new navigator. Values not provided will be prompted
git drive new [[--as] user --name User --email Email]

# Delets navigator(s), either prompted for, or specified
git drive delete [user1 [user2...]]

# List known aliases for the driver
git drive me list

# Edit driver, either prompted for, or specified
git drive me edit [user1 [user2...]]

# Add new driver. Values not provided will be prompted
git drive me new [[--as] user --name User --email Email]

# Delets a driver, either prompted for, or specified
git drive me delete [user1 [user2...]]

# Change identity while driving
git drive as alias
```

*/

use color_eyre::{
    eyre::{bail, eyre},
    Result, Section, SectionExt,
};
use config::Config;
use data::{Action, Command, Driver, Id, Kind, Navigator, New, Provided};
use dialoguer::{Input, MultiSelect, Select};
use std::{
    fs::File,
    ops::Deref,
    path::{Path, PathBuf},
    process::Command as Proc,
};

mod args;
mod config;
mod data;

fn main() -> Result<()> {
    install_eyre()?;

    let mut config = config::load()?;
    let command = args::command(&config);

    let Command { kind, action } = command;
    let changed = match action {
        Action::Drive(Provided(None)) => select_drive(&config)?,
        Action::Drive(Provided(Some(ids))) => run_drive(ids, &config)?,
        Action::Change(Provided(None)) => bail!("Switching seats not yet implemented"),
        Action::Change(Provided(Some(_))) => bail!("Switching seats not yet implemented"),
        Action::List => run_list(kind, &config),
        Action::New(new) => run_new(kind, &mut config, new)?,
        Action::Edit(new) => run_edit(kind, &mut config, new)?,
        Action::Delete(Provided(Some(ids))) => run_delete(kind, &mut config, ids),
        Action::Delete(_) => select_delete(kind, &mut config)?,
    };

    if changed {
        config::store(config)?;
    }

    Ok(())
}

fn select_drive(config: &Config) -> Result<bool> {
    let ids = select_ids_from(Kind::Navigator, config)?;
    run_drive(ids, config)
}

fn select_delete(kind: Kind, config: &mut Config) -> Result<bool> {
    let ids = select_ids_from(kind, config)?
        .into_iter()
        .cloned()
        .collect();
    Ok(run_delete(kind, config, ids))
}

fn run_drive<A: IdRef>(ids: Vec<A>, config: &Config) -> Result<bool> {
    if ids.is_empty() {
        return drive_alone();
    }

    let navigators = ids
        .into_iter()
        .map(|id| {
            config
                .navigators
                .iter()
                .find(|n| id.id().same_as(n))
                .ok_or_else(|| eyre!("No navigator found for `{}`", id.id().as_ref()))
        })
        .collect::<Result<Vec<_>>>()?;

    drive_with(navigators.into_iter())?;

    Ok(false)
}

fn drive_alone() -> Result<bool> {
    let sc = Proc::new("git")
        .args(&["config", "--unset", "commit.template"])
        .spawn()?
        .wait()?;

    if !sc.success() {
        std::process::exit(sc.code().unwrap_or_default())
    }

    Ok(false)
}

fn drive_with<'a>(navigators: impl Iterator<Item = &'a Navigator>) -> Result<()> {
    let top_level = Proc::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()?;
    assert!(top_level.status.success());

    let top_level = top_level.stdout;
    let top_level = String::from_utf8(top_level)?;
    let mut template_file = PathBuf::from(top_level.trim());
    template_file.push(".git");
    template_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_commit_template"));

    let templates = navigators.map(|n| format!("Co-Authored-By: {} <{}>", n.name, n.email));

    write_template(&template_file, templates)
        .with_section(|| format!("{}", template_file.display()).header("File:"))?;

    let sc = Proc::new("git")
        .args(&["config", "commit.template"])
        .arg(template_file)
        .spawn()?
        .wait()?;

    if !sc.success() {
        std::process::exit(sc.code().unwrap_or_default())
    }

    Ok(())
}

fn write_template(file: &Path, data: impl Iterator<Item = String>) -> Result<()> {
    use std::io::Write;

    let mut f = File::create(file)?;
    writeln!(f)?;
    writeln!(f)?;
    for line in data {
        writeln!(f, "{}", line)?;
    }

    f.flush()?;
    Ok(())
}

fn run_list(kind: Kind, config: &Config) -> bool {
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

fn complete_nav<'a, F, G>(new: New, valid: F, error_msg: &str, lookup: G) -> Result<Navigator>
where
    F: for<'x> Fn(&'x str) -> bool,
    G: for<'x> Fn(&'x Id) -> Option<&'a Navigator>,
{
    let New {
        id,
        name,
        email,
        key: _,
    } = new;

    let alias = match id {
        Some(id) => Id(id),
        None => {
            let input = Input::<String>::new()
                .with_prompt("The alias?")
                .validate_with(|input: &String| -> Result<(), String> {
                    if valid(input.as_str()) {
                        Ok(())
                    } else {
                        Err(format!("Alias {} {}", input, error_msg))
                    }
                })
                .interact_text()?;
            Id(input)
        }
    };

    if !valid(&*alias) {
        bail!("Alias {} {}", &*alias, error_msg);
    }

    let existing = lookup(&alias);

    let mut input = Input::<String>::new();
    input.with_prompt(format!("The name for [{}]", &*alias));
    if let Some(name) = name {
        input.with_initial_text(name);
    } else if let Some(ex) = existing {
        input.with_initial_text(ex.name.as_str());
    }
    let name = input.interact()?;

    let mut input = Input::<String>::new();
    input.with_prompt(format!("The email for [{}]", &*alias));
    if let Some(email) = email {
        input.with_initial_text(email);
    } else if let Some(ex) = existing {
        input.with_initial_text(ex.email.as_str());
    }
    let email = input.interact_text()?;

    Ok(Navigator { alias, name, email })
}

fn complete_drv<'a, F, G>(mut new: New, valid: F, error_msg: &str, lookup: G) -> Result<Driver>
where
    F: for<'x> Fn(&'x str) -> bool,
    G: for<'x> Fn(&'x Id) -> Option<&'a Driver>,
{
    let key = new.key.take();
    let navigator = complete_nav(new, valid, error_msg, |id| lookup(id).map(|d| &d.navigator))?;

    let existing = lookup(&navigator.alias);

    let mut input = Input::<String>::new();
    input.with_prompt(format!(
        "The signing key for [{}]",
        navigator.alias.as_ref()
    ));
    input.allow_empty(true);
    if let Some(key) = key {
        input.with_initial_text(key);
    } else if let Some(key) = existing.and_then(|d| d.key.as_ref()) {
        input.with_initial_text(key.as_str());
    }
    let key = input.interact_text()?;
    let key = if key.is_empty() { None } else { Some(key) };

    Ok(Driver { navigator, key })
}

fn complete_new_nav(new: New, config: &Config) -> Result<Navigator> {
    complete_nav(
        new,
        |input| !config.navigators.iter().any(|n| n.alias.as_ref() == input),
        "already exists",
        |_| None,
    )
}

fn complete_existing_nav(new: New, config: &Config) -> Result<Navigator> {
    complete_nav(
        new,
        |input| config.navigators.iter().any(|n| n.alias.as_ref() == input),
        "does not exist",
        |id| config.navigators.iter().find(|n| id.same_as(n)),
    )
}

fn complete_new_drv(new: New, config: &Config) -> Result<Driver> {
    complete_drv(
        new,
        |input| !config.drivers.iter().any(|n| n.alias.as_ref() == input),
        "already exists",
        |_| None,
    )
}

fn complete_existing_drv(new: New, config: &Config) -> Result<Driver> {
    complete_drv(
        new,
        |input| config.drivers.iter().any(|n| n.alias.as_ref() == input),
        "does not exist",
        |id| config.drivers.iter().find(|d| id.same_as(*d)),
    )
}

fn run_new(kind: Kind, config: &mut Config, new: New) -> Result<bool> {
    match kind {
        Kind::Navigator => {
            let navigator = complete_new_nav(new, config)?;
            config.navigators.push(navigator);
        }
        Kind::Driver => {
            let driver = complete_new_drv(new, config)?;
            config.drivers.push(driver);
        }
    }
    Ok(true)
}

fn run_edit(kind: Kind, config: &mut Config, mut new: New) -> Result<bool> {
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
        let mut ms = Select::new();
        ms.with_prompt("Use [return] to select");
        match kind {
            Kind::Navigator => {
                for nav in &config.navigators {
                    ms.item(&*nav.alias);
                }
            }
            Kind::Driver => {
                for drv in &config.drivers {
                    ms.item(&*drv.navigator.alias);
                }
            }
        }
        let chosen = ms.interact()?;
        let id = match kind {
            Kind::Navigator => &config.navigators[chosen],
            Kind::Driver => &config.drivers[chosen].navigator,
        };
        new.id = Some(id.alias.as_ref().to_string());
    }
    match kind {
        Kind::Navigator => {
            let navigator = complete_existing_nav(new, config)?;
            let nav = config
                .navigators
                .iter_mut()
                .find(|n| navigator.alias.same_as(n))
                .expect("validated during complete_existing_nav");
            *nav = navigator;
        }
        Kind::Driver => {
            let driver = complete_existing_drv(new, config)?;
            let drv = config
                .drivers
                .iter_mut()
                .find(|d| driver.alias.same_as(&d.navigator))
                .expect("validated during complete_existing_nav");
            *drv = driver;
        }
    }
    Ok(true)
}

fn run_delete<A: IdRef>(kind: Kind, config: &mut Config, ids: Vec<A>) -> bool {
    match kind {
        Kind::Navigator => do_delete(&mut config.navigators, ids),
        Kind::Driver => do_delete(&mut config.drivers, ids),
    }
}

fn do_delete<A: IdRef, T: Deref<Target = Navigator>>(data: &mut Vec<T>, ids: Vec<A>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i != data.len() {
        if ids.iter().any(|id| id.id() == &data[i].alias) {
            let _ = data.remove(i);
            changed = true;
        } else {
            i += 1;
        }
    }
    changed
}

fn print_nav(nav: &Navigator) {
    println!("{}: {} <{}>", &*nav.alias, nav.name, nav.email)
}

fn select_ids_from(kind: Kind, config: &Config) -> Result<Vec<&Id>> {
    let selection = select_from(kind, config)?;
    let ids = match kind {
        Kind::Navigator => config
            .navigators
            .iter()
            .enumerate()
            .filter_map(|(idx, nav)| {
                if selection.contains(&idx) {
                    Some(&nav.alias)
                } else {
                    None
                }
            })
            .collect(),
        Kind::Driver => config
            .drivers
            .iter()
            .enumerate()
            .filter_map(|(idx, drv)| {
                if selection.contains(&idx) {
                    Some(&drv.navigator.alias)
                } else {
                    None
                }
            })
            .collect(),
    };
    Ok(ids)
}

fn select_from(kind: Kind, config: &Config) -> Result<Vec<usize>> {
    let mut ms = MultiSelect::new();
    ms.with_prompt("Use [space] to select, [return] to confirm");
    match kind {
        Kind::Navigator => {
            for nav in &config.navigators {
                ms.item(&*nav.alias);
            }
        }
        Kind::Driver => {
            for drv in &config.drivers {
                ms.item(&*drv.navigator.alias);
            }
        }
    }

    let chosen = ms.interact()?;
    Ok(chosen)
}

fn install_eyre() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .capture_span_trace_by_default(false)
        .display_env_section(false)
        .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
        .add_issue_metadata("version", env!("CARGO_PKG_VERSION"))
        .issue_filter(|kind| match kind {
            color_eyre::ErrorKind::NonRecoverable(_) => false,
            color_eyre::ErrorKind::Recoverable(_) => true,
        })
        .install()
}

trait IdRef {
    fn id(&self) -> &Id;
}

impl IdRef for Id {
    fn id(&self) -> &Id {
        self
    }
}

impl IdRef for &Id {
    fn id(&self) -> &Id {
        self
    }
}
