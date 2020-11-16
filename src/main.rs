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

use color_eyre::{
    eyre::{bail, eyre},
    Result, Section, SectionExt,
};
use config::Config;
use console::{style, Style};
use data::{Action, Command, Driver, Id, Kind, Navigator, New, Provided};
use dialoguer::{
    theme::{ColorfulTheme, Theme},
    Input, MultiSelect, Select,
};
use std::{
    fs::File,
    io::ErrorKind,
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
        Action::Show(color) => run_show(color),
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

        let mut app = args::app(config).before_help(pre_help.as_str());
        app.print_help()?;
        return Ok(false);
    }
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

    match sc.code() {
        Some(0) | Some(5) => {}
        Some(c) => std::process::exit(c),
        None => std::process::exit(127),
    }

    let git_dir = git_dir()?;
    let mut current_navigators_file = git_dir;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));

    if let Err(e) = std::fs::remove_file(&current_navigators_file) {
        if e.kind() != ErrorKind::NotFound {
            return Err(eyre!(e).with_section(|| {
                format!("{}", current_navigators_file.display()).header("File:")
            }));
        }
    }

    Ok(false)
}

/// U+001F - Information Separator One
const SEPARATOR: u8 = 0x1F_u8;

fn drive_with<'a>(navigators: impl Iterator<Item = &'a Navigator>) -> Result<()> {
    let git_dir = git_dir()?;

    let (co_authored_lines, navigators): (Vec<_>, Vec<_>) = navigators
        .map(|n| {
            let co_authored_line = format!("Co-Authored-By: {} <{}>", n.name, n.email);
            let navigator = n.alias.as_bytes().to_vec();
            (co_authored_line, navigator)
        })
        .unzip();

    let template_file = git_dir.join(concat!(env!("CARGO_PKG_NAME"), "_commit_template"));
    write_template(&template_file, co_authored_lines.into_iter())
        .with_section(|| format!("{}", template_file.display()).header("File:"))?;

    let navigators = navigators.join([SEPARATOR].as_ref());
    let mut current_navigators_file = git_dir;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));
    write_data(&current_navigators_file, navigators)
        .with_section(|| format!("{}", current_navigators_file.display()).header("File:"))?;
    println!(
        "git-commit set template to {}. Use {} to unset and drive alone.",
        style(template_file.display()).cyan(),
        style(concat!(env!("CARGO_PKG_NAME"), " alone")).yellow()
    );

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

fn write_data(file: &Path, data: Vec<u8>) -> Result<()> {
    use std::io::Write;
    let mut f = File::create(file)?;
    f.write_all(&data)?;
    f.flush()?;
    Ok(())
}

fn run_show(color: String) -> bool {
    let _ = run_show_fallible(color);
    false
}

fn run_show_fallible(color: String) -> Result<()> {
    let mut current_navigators_file = git_dir()?;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));

    let data = read_data(&current_navigators_file)
        .with_section(|| format!("{}", current_navigators_file.display()).header("File:"))?;

    let style = Style::from_dotted_str(&color);

    let s = data
        .split(|b| *b == SEPARATOR)
        .map(|s| String::from_utf8_lossy(s))
        .map(|id| format!("{} ", style.apply_to(id)))
        .collect::<String>();

    println!("{}", s.trim_end());
    Ok(())
}

fn git_dir() -> Result<PathBuf> {
    let git_dir = Proc::new("git")
        .args(&["rev-parse", "--absolute-git-dir"])
        .output()?;
    if !git_dir.status.success() {
        return Err(eyre!("Could not get current git dir")
            .with_section(|| {
                String::from_utf8_lossy(&git_dir.stdout[..])
                    .into_owned()
                    .header("Stderr:")
            })
            .with_suggestion(|| {
                concat!(
                    "Try calling ",
                    env!("CARGO_PKG_NAME"),
                    " from a working directory of a git repository."
                )
            }))?;
    }

    let git_dir = git_dir.stdout;
    let git_dir = String::from_utf8(git_dir)?;
    let git_dir = PathBuf::from(git_dir.trim());
    Ok(git_dir)
}

fn read_data(file: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut f = File::open(file)?;
    let mut data = Vec::with_capacity(64);
    let read = f.read_to_end(&mut data)?;
    data.truncate(read);
    Ok(data)
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

fn complete_nav<'a, F, G>(
    new: New,
    valid: F,
    error_msg: &str,
    lookup: G,
    theme: &dyn Theme,
) -> Result<Navigator>
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
            let input = Input::<String>::with_theme(theme)
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

    let mut input = Input::<String>::with_theme(theme);
    input.with_prompt(format!("The name for [{}]", &*alias));
    if let Some(name) = name {
        input.with_initial_text(name);
    } else if let Some(ex) = existing {
        input.with_initial_text(ex.name.as_str());
    }
    let name = input.interact()?;

    let mut input = Input::<String>::with_theme(theme);
    input.with_prompt(format!("The email for [{}]", &*alias));
    if let Some(email) = email {
        input.with_initial_text(email);
    } else if let Some(ex) = existing {
        input.with_initial_text(ex.email.as_str());
    }
    let email = input.interact_text()?;

    Ok(Navigator { alias, name, email })
}

fn complete_drv<'a, F, G>(
    mut new: New,
    valid: F,
    error_msg: &str,
    lookup: G,
    theme: &dyn Theme,
) -> Result<Driver>
where
    F: for<'x> Fn(&'x str) -> bool,
    G: for<'x> Fn(&'x Id) -> Option<&'a Driver>,
{
    let key = new.key.take();
    let navigator = complete_nav(
        new,
        valid,
        error_msg,
        |id| lookup(id).map(|d| &d.navigator),
        theme,
    )?;

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

fn complete_new_nav(new: New, config: &Config, theme: &dyn Theme) -> Result<Navigator> {
    complete_nav(
        new,
        |input| !config.navigators.iter().any(|n| n.alias.as_ref() == input),
        "already exists",
        |_| None,
        theme,
    )
}

fn complete_existing_nav(new: New, config: &Config, theme: &dyn Theme) -> Result<Navigator> {
    complete_nav(
        new,
        |input| config.navigators.iter().any(|n| n.alias.as_ref() == input),
        "does not exist",
        |id| config.navigators.iter().find(|n| id.same_as(n)),
        theme,
    )
}

fn complete_new_drv(new: New, config: &Config, theme: &dyn Theme) -> Result<Driver> {
    complete_drv(
        new,
        |input| !config.drivers.iter().any(|n| n.alias.as_ref() == input),
        "already exists",
        |_| None,
        theme,
    )
}

fn complete_existing_drv(new: New, config: &Config, theme: &dyn Theme) -> Result<Driver> {
    complete_drv(
        new,
        |input| config.drivers.iter().any(|n| n.alias.as_ref() == input),
        "does not exist",
        |id| config.drivers.iter().find(|d| id.same_as(*d)),
        theme,
    )
}

fn run_new(kind: Kind, config: &mut Config, new: New) -> Result<bool> {
    let theme = ColorfulTheme::default();
    match kind {
        Kind::Navigator => {
            let navigator = complete_new_nav(new, config, &theme)?;
            config.navigators.push(navigator);
        }
        Kind::Driver => {
            let driver = complete_new_drv(new, config, &theme)?;
            config.drivers.push(driver);
        }
    }
    Ok(true)
}

fn run_edit(kind: Kind, config: &mut Config, mut new: New) -> Result<bool> {
    let theme = ColorfulTheme::default();
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
        let mut ms = Select::with_theme(&theme);
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
            let navigator = complete_existing_nav(new, config, &theme)?;
            let nav = config
                .navigators
                .iter_mut()
                .find(|n| navigator.alias.same_as(n))
                .expect("validated during complete_existing_nav");
            *nav = navigator;
        }
        Kind::Driver => {
            let driver = complete_existing_drv(new, config, &theme)?;
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
    let theme = ColorfulTheme::default();
    let mut ms = MultiSelect::with_theme(&theme);
    ms.with_prompt("Use [space] to select, [return] to confirm");
    match kind {
        Kind::Navigator => {
            if config.navigators.is_empty() {
                return Ok(Vec::new());
            }
            for nav in &config.navigators {
                ms.item(&*nav.alias);
            }
        }
        Kind::Driver => {
            if config.drivers.is_empty() {
                return Ok(Vec::new());
            }
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
