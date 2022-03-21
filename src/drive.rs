use crate::{
    config::Config,
    data::{Id, IdRef, Kind, Navigator},
    io,
};
use color_eyre::{eyre::eyre, Result, Section, SectionExt};
use console::{style, Style};
use std::{
    fs::File,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command as Proc,
};

pub(crate) fn current(color: String, fail_if_empty: bool) -> bool {
    let current = current_fallible(color);
    if fail_if_empty {
        if matches!(current, Ok(false) | Err(_)) {
            std::process::exit(1);
        }
    }

    false
}

pub(crate) fn select(config: &Config) -> Result<bool> {
    let currently = get_current().unwrap_or_default();
    let ids = io::select_ids_from(Kind::Navigator, config, currently)?;
    run(ids, config)
}

pub(crate) fn run<A: IdRef>(ids: Vec<A>, config: &Config) -> Result<bool> {
    if ids.is_empty() {
        return drive_alone();
    }

    let navigators = ids
        .into_iter()
        .map(|id| {
            config
                .navigators
                .iter()
                .find(|n| id.id().same_as_nav(n))
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
        "git-commit set template to {}.",
        style(template_file.display()).cyan(),
    );

    let prog = (|| {
        let prog = std::env::args().next()?;
        let prog = Path::new(prog.as_str());
        let file = prog.file_name()?;
        let file = file.to_str()?;
        Some(file.to_string())
    })()
    .unwrap_or_else(|| String::from(env!("CARGO_PKG_NAME")));

    println!(
        "Use {} {} to unset and drive alone.",
        style(prog).yellow(),
        style("alone").yellow(),
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

fn current_fallible(color: String) -> Result<bool> {
    let ids = get_current()?;
    let style = Style::from_dotted_str(&color);
    let has_current = !ids.is_empty();
    let s = ids
        .into_iter()
        .map(|id| format!("{} ", style.apply_to(&*id)))
        .collect::<String>();

    println!("{}", s.trim_end());
    Ok(has_current)
}

fn get_current() -> Result<Vec<Id>> {
    let mut current_navigators_file = git_dir()?;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));

    let data = read_data(&current_navigators_file)
        .with_section(|| format!("{}", current_navigators_file.display()).header("File:"))?;

    let ids = data
        .split(|b| *b == SEPARATOR)
        .map(|s| Ok(Id(String::from_utf8(s.to_vec())?)))
        .collect::<Result<Vec<_>>>()?;

    Ok(ids)
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
