use crate::{
    config::Config,
    data::{Id, IdRef, Kind, Modification, Navigator, ShowNav},
    ui::{self, SelectMany},
};
use console::{style, Style};
use eyre::{eyre, Result, WrapErr};
use std::{
    borrow::Borrow,
    fs::File,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command as Proc,
};

pub fn current(
    ShowNav {
        color,
        fail_if_empty,
    }: ShowNav,
) -> Modification {
    let current = current_fallible(&color);
    if fail_if_empty && matches!(current, Ok(false) | Err(_)) {
        std::process::exit(1);
    }

    Modification::Unchanged
}

pub fn select(ui: impl SelectMany, config: &Config) -> Result<Option<Modification>> {
    // TODO: proper error type
    if config.navigators.is_empty() {
        Ok(None)
    } else {
        select_navigators(ui, config).map(Some)
    }
}

fn select_navigators(ui: impl SelectMany, config: &Config) -> Result<Modification> {
    let currently = get_current().unwrap_or_default();
    let ids = ui::select_ids_from(ui, Kind::Navigator, config, &currently)?;
    run(&ids, config)
}

pub fn run<I>(ids: &[I], config: &Config) -> Result<Modification>
where
    I: IdRef,
{
    let navigators = ids
        .iter()
        .map(|id| match_navigator(id.id(), config))
        .collect::<Result<Vec<_>>>()?;

    if navigators.is_empty() {
        return alone();
    }

    drive_with(navigators.into_iter())?;

    Ok(Modification::Unchanged)
}

fn match_navigator<'config>(query: &Id, config: &'config Config) -> Result<&'config Navigator> {
    let direct_matches = config.navigators.iter().filter(|n| query.same_as_nav(n));
    if let Some(direct) = validate_matches(query, direct_matches) {
        return direct;
    }
    let partial_matches = config
        .navigators
        .iter()
        .filter(|n| match_caseless(query, n.borrow()));
    if let Some(direct) = validate_matches(query, partial_matches) {
        return direct;
    }

    Err(eyre!("No navigator found for `{}`", query.id().as_ref()))
}

fn validate_matches<'config>(
    query: &Id,
    mut matches: impl Iterator<Item = &'config Navigator>,
) -> Option<Result<&'config Navigator>> {
    matches.next().map(|direct| {
        let conflicting = matches.collect::<Vec<_>>();
        if conflicting.is_empty() {
            Ok(direct)
        } else {
            Err(eyre!(
                "The query `{}` is ambiguous, possible candidates: [{}, {}]",
                query.as_ref(),
                direct.id().as_ref(),
                conflicting.join(", ")
            ))
        }
    })
}

fn match_caseless(query: &str, navigator: &str) -> bool {
    use caseless::Caseless;
    use unicode_normalization::UnicodeNormalization;

    fn iter_starts_with<L: Iterator<Item = char>, R: Iterator<Item = char>>(
        mut a: L,
        mut b: R,
    ) -> bool {
        loop {
            match (a.next(), b.next()) {
                (_, None) => return true,
                (None, _) => return false,
                (Some(x), Some(y)) => {
                    if !x.eq(&y) {
                        return false;
                    }
                }
            }
        }
    }

    iter_starts_with(
        navigator
            .chars()
            .nfd()
            .default_case_fold()
            .filter(char::is_ascii),
        query
            .chars()
            .nfd()
            .default_case_fold()
            .filter(char::is_ascii),
    )
}

pub fn alone() -> Result<Modification> {
    let sc = Proc::new("git")
        .args(&["config", "--unset", "commit.template"])
        .spawn()?
        .wait()?;

    match sc.code() {
        Some(0 | 5) => {}
        Some(c) => std::process::exit(c),
        None => std::process::exit(127),
    }

    let git_dir = git_dir()?;
    let mut current_navigators_file = git_dir;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));

    if let Err(e) = std::fs::remove_file(&current_navigators_file) {
        if e.kind() != ErrorKind::NotFound {
            return Err(eyre!(e).wrap_err(format!("File: {}", current_navigators_file.display())));
        }
    }

    Ok(Modification::Unchanged)
}

/// U+001F - Information Separator One
const SEPARATOR: u8 = 0x1F_u8;

fn drive_with<'a>(navigators: impl ExactSizeIterator<Item = &'a Navigator>) -> Result<()> {
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
        .wrap_err_with(|| format!("File: {}", template_file.display()))?;

    let navigators = navigators.join([SEPARATOR].as_ref());
    let mut current_navigators_file = git_dir;
    current_navigators_file.push(concat!(".", env!("CARGO_PKG_NAME"), "_current_navigators"));
    write_data(&current_navigators_file, &navigators)
        .wrap_err_with(|| format!("File: {}", current_navigators_file.display()))?;
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

fn write_data(file: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut f = File::create(file)?;
    f.write_all(data)?;
    f.flush()?;
    Ok(())
}

fn current_fallible(color: &str) -> Result<bool> {
    let ids = get_current()?;
    let style = Style::from_dotted_str(color);
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
        .wrap_err_with(|| format!("File: {}", current_navigators_file.display()))?;

    let ids = data
        .split(|b| *b == SEPARATOR)
        .map(std::str::from_utf8)
        .map(|s| s.map(String::from).map(Id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ids)
}

fn git_dir() -> Result<PathBuf> {
    let git_dir = Proc::new("git")
        .args(&["rev-parse", "--absolute-git-dir"])
        .output()?;
    if !git_dir.status.success() {
        return Err(eyre!(
            concat!(
                "Could not get current git dir\n",
                "Stderr: {}\n",
                "\n",
                "Try calling ",
                env!("CARGO_PKG_NAME"),
                " from a working directory of a git repository."
            ),
            String::from_utf8_lossy(&git_dir.stdout[..])
        ));
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
