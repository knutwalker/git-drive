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
git driver list

# Edit driver, either prompted for, or specified
git drive me edit [user1 [user2...]]
git driver edit [user1 [user2...]]

# Add new driver. Values not provided will be prompted
git drive me new [[--as] user --name User --email Email]
git driver add [[--as] user --name User --email Email]

# Delets a driver, either prompted for, or specified
git drive me delete [user1 [user2...]]
git driver delete [user1 [user2...]]
```

*/

use color_eyre::{
    eyre::{bail, eyre},
    Result, Section, SectionExt,
};
use config::Config;
use data::{Command, Driver, Id, Kind, Navigator, New, Provided};
use std::{
    fs::File,
    ops::Deref,
    path::{Path, PathBuf},
    process::Command as Proc,
};

mod args;
mod config;
mod data;

static APPLICATION: &str = env!("CARGO_PKG_NAME");

fn main() -> Result<()> {
    install_eyre()?;

    let mut config = config::load()?;
    let command = args::command(&config);

    eprintln!("config = {:#?}", config);
    eprintln!("command = {:#?}", command);

    let changed = match command {
        Command::Drive(Provided(None)) => bail!("driving not yet implemented"),
        Command::Drive(Provided(Some(ids))) => run_drive(ids, &config)?,
        Command::List(kind) => run_list(kind, &config),
        Command::New(kind, new) => run_new(kind, &mut config, new)?,
        Command::Edit(kind, Provided(None), new) => run_edit(kind, &mut config, new)?,
        Command::Edit(_, _, _) => bail!("prompting or multi edit not yet implemented"),
        Command::Delete(kind, Provided(Some(ids))) => run_delete(kind, &mut config, ids),
        Command::Delete(_, _) => bail!("prompting for deleted not yet implemented"),
    };

    if changed {
        eprintln!("config = {:#?}", config);
        config::store(config)?;
    }

    Ok(())
}

fn run_drive(ids: Vec<Id>, config: &Config) -> Result<bool> {
    if ids.is_empty() {
        return drive_alone();
    }

    let navigators = ids
        .into_iter()
        .map(|id| {
            config
                .navigators
                .iter()
                .find(|n| id.same_as(n))
                .ok_or_else(|| eyre!("No navigator found for `{}`", &*id))
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

fn run_new(kind: Kind, config: &mut Config, new: New) -> Result<bool> {
    match new {
        New {
            id: Some(id),
            name: Some(name),
            email: Some(email),
            key,
        } => {
            let alias = Id(id);

            match kind {
                Kind::Navigator => {
                    if config.navigators.iter().any(|n| alias.same_as(&n)) {
                        bail!(
                            "Alias {} already exists, use `{} edit` to change it ",
                            &*alias,
                            APPLICATION,
                        );
                    }
                    let navigator = Navigator { alias, name, email };
                    config.navigators.push(navigator);
                }
                Kind::Driver => {
                    if config.drivers.iter().any(|d| alias.same_as(d)) {
                        bail!(
                            "Alias {} already exists, use `{} me edit` to change it ",
                            &*alias,
                            APPLICATION,
                        );
                    }
                    let navigator = Navigator { alias, name, email };
                    let driver = Driver { navigator, key };
                    config.drivers.push(driver)
                }
            }
            Ok(true)
        }
        _ => bail!("prompting not yet implemented"),
    }
}

fn run_edit(kind: Kind, config: &mut Config, new: New) -> Result<bool> {
    match new {
        New {
            id: Some(id),
            name,
            email,
            key,
        } => {
            let alias = Id(id);
            // let navigator = Navigator { alias, name, email };
            match kind {
                Kind::Navigator => match config.navigators.iter_mut().find(|n| alias.same_as(n)) {
                    Some(nav) => {
                        if let Some(name) = name {
                            nav.name = name;
                        }
                        if let Some(email) = email {
                            nav.email = email;
                        }
                    }
                    None => {
                        bail!(
                            "Alias {} does not exist, use `{} new` to add it ",
                            &*alias,
                            APPLICATION,
                        );
                    }
                },
                Kind::Driver => match config.drivers.iter_mut().find(|d| alias.same_as(&**d)) {
                    Some(drv) => {
                        if let Some(name) = name {
                            drv.navigator.name = name;
                        }
                        if let Some(email) = email {
                            drv.navigator.email = email;
                        }
                        if let Some(key) = key {
                            drv.key = Some(key);
                        }
                    }
                    None => {
                        bail!(
                            "Alias {} does not exist, use `{} me new` to add it ",
                            &*alias,
                            APPLICATION,
                        );
                    }
                },
            }
            Ok(true)
        }
        _ => bail!("prompting not yet implemented"),
    }
}

fn run_delete(kind: Kind, config: &mut Config, ids: Vec<Id>) -> bool {
    match kind {
        Kind::Navigator => do_delete(&mut config.navigators, ids),
        Kind::Driver => do_delete(&mut config.drivers, ids),
    }
}

fn do_delete<T: Deref<Target = Navigator>>(data: &mut Vec<T>, ids: Vec<Id>) -> bool {
    let mut changed = false;
    let mut i = 0;
    while i != data.len() {
        if ids.contains(&data[i].alias) {
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
