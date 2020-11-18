use crate::{
    config::Config,
    data::{Driver, Id, Kind, Navigator, New},
    Result,
};
use color_eyre::eyre::eyre;
use console::{style, Style, StyledObject};
use dialoguer::{
    theme::{ColorfulTheme as PrettyTheme, Theme},
    Input, MultiSelect, Select, Validator,
};
use std::{cell::RefCell, fmt, marker::PhantomData};

pub(crate) fn select_ids_from(
    kind: Kind,
    config: &Config,
    pre_selected: Vec<Id>,
) -> Result<Vec<&Id>> {
    let selection = select_from(kind, config, pre_selected)?;
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

fn select_from(kind: Kind, config: &Config, pre_select: Vec<Id>) -> Result<Vec<usize>> {
    let theme = PrettyTheme::default();
    let mut ms = MultiSelect::with_theme(&theme);
    ms.with_prompt("Use [space] to select, [return] to confirm");
    match kind {
        Kind::Navigator => {
            if config.navigators.is_empty() {
                return Ok(Vec::new());
            }
            for nav in &config.navigators {
                ms.item_checked(&*nav.alias, pre_select.contains(&nav.alias));
            }
        }
        Kind::Driver => {
            if config.drivers.is_empty() {
                return Ok(Vec::new());
            }
            for drv in &config.drivers {
                ms.item_checked(
                    &*drv.navigator.alias,
                    pre_select.contains(&drv.navigator.alias),
                );
            }
        }
    }

    let chosen = ms.interact()?;
    Ok(chosen)
}

pub(crate) fn select_id_from(kind: Kind, config: &Config) -> Result<Id> {
    let theme = PrettyTheme::default();
    let mut select = Select::with_theme(&theme);
    select.with_prompt("Use [return] to select");

    match kind {
        Kind::Navigator => {
            for nav in &config.navigators {
                select.item(&*nav.alias);
            }
        }
        Kind::Driver => {
            for drv in &config.drivers {
                select.item(&*drv.navigator.alias);
            }
        }
    }
    let chosen = select.interact()?;
    let id = match kind {
        Kind::Navigator => &config.navigators[chosen],
        Kind::Driver => &config.drivers[chosen].navigator,
    };
    let id = id.alias.as_ref().to_string();
    Ok(Id(id))
}

pub(crate) fn prompt_for_empty(
    thing: &'static str,
    id: &str,
    initial: Option<String>,
    allow_empty: bool,
    theme: &dyn Theme,
) -> Result<String> {
    let mut input = Input::<String>::with_theme(theme);
    input
        .with_prompt(format!("The {} for {}\n", thing, style(id).cyan()))
        .allow_empty(true)
        .validate_with(CheckForEmpty::new(thing, allow_empty));
    if let Some(initial) = initial {
        input.with_initial_text(initial);
    }
    let name = input.interact()?;
    Ok(name)
}

pub(crate) fn prompt_for(
    thing: &'static str,
    id: &str,
    initial: Option<String>,
    theme: &dyn Theme,
) -> Result<String> {
    prompt_for_empty(thing, id, initial, false, theme)
}

pub(crate) fn complete_new_nav(new: New, config: &Config) -> Result<Navigator> {
    let theme = PrettyTheme::default();
    complete_nav(CheckMode::MustNotExist, new, config, &theme)
}

pub(crate) fn complete_existing_nav(new: New, config: &Config) -> Result<Navigator> {
    let theme = PrettyTheme::default();
    complete_nav(CheckMode::MustExist, new, config, &theme)
}

pub(crate) fn complete_new_drv(new: New, config: &Config) -> Result<Driver> {
    let theme = PrettyTheme::default();
    complete_drv(CheckMode::MustNotExist, new, config, &theme)
}

pub(crate) fn complete_existing_drv(new: New, config: &Config) -> Result<Driver> {
    let theme = PrettyTheme::default();
    complete_drv(CheckMode::MustExist, new, config, &theme)
}

fn prompt_alias<'a, T: Seat + Copy>(
    check: CheckMode,
    config: &'a Config,
    existing: Option<String>,
    theme: &dyn Theme,
) -> Result<(Id, Option<&'a T::Entity>)> {
    let lookup = Lookup::<T>::new(config, check);
    let id = match existing {
        Some(id) => {
            lookup.validate(&id)?;
            Id(id)
        }
        None => {
            let input = Input::<String>::with_theme(theme)
            .with_prompt(format!("Please enter the alias for the {}.\n  The alias will be used as identifier for all other commands.\n", T::kind()))
            .validate_with(CheckForEmpty::new("alias", false))
            .validate_with(lookup)
            .interact_text()?;
            Id(input)
        }
    };

    let existing = lookup.matching_navigator(&*id);
    Ok((id, existing))
}

fn complete_nav(
    check: CheckMode,
    new: New,
    config: &Config,
    theme: &dyn Theme,
) -> Result<Navigator> {
    let New {
        id,
        name,
        email,
        key: _,
    } = new;

    let (alias, existing) = prompt_alias::<NavigatorSeat>(check, config, id, theme)?;
    finish_nav(alias, name, email, existing, theme)
}

fn finish_nav(
    alias: Id,
    name: Option<String>,
    email: Option<String>,
    existing: Option<&Navigator>,
    theme: &dyn Theme,
) -> Result<Navigator> {
    let name = prompt_for(
        "name",
        &*alias,
        name.or_else(|| existing.map(|n| n.name.clone())),
        theme,
    )?;
    let email = prompt_for(
        "email",
        &*alias,
        email.or_else(|| existing.map(|n| n.email.clone())),
        theme,
    )?;

    Ok(Navigator { alias, name, email })
}

fn complete_drv(check: CheckMode, new: New, config: &Config, theme: &dyn Theme) -> Result<Driver> {
    let New {
        id,
        name,
        email,
        key,
    } = new;

    let (alias, existing) = prompt_alias::<DriverSeat>(check, config, id, theme)?;
    let navigator = finish_nav(alias, name, email, existing.map(|d| &d.navigator), theme)?;

    let key = prompt_for_empty(
        "signing key",
        &*navigator.alias,
        key.or_else(|| existing.and_then(|d| d.key.clone())),
        true,
        theme,
    )?;
    let key = if key.is_empty() { None } else { Some(key) };

    Ok(Driver { navigator, key })
}

#[derive(Copy, Clone)]
enum MsgTemplate {
    MustNotBeEmpty,
    EnterANonEmptyName,
}
struct CheckForEmpty {
    messages: RefCell<Vec<MsgTemplate>>,
    entity: &'static str,
    allow_empty: bool,
}

impl CheckForEmpty {
    fn new(entity: &'static str, allow_empty: bool) -> Self {
        Self {
            messages: RefCell::new(vec![
                MsgTemplate::MustNotBeEmpty,
                MsgTemplate::EnterANonEmptyName,
            ]),
            entity,
            allow_empty,
        }
    }
}

impl Validator<String> for CheckForEmpty {
    type Err = String;

    fn validate(&self, input: &String) -> Result<(), Self::Err> {
        if self.allow_empty || !input.trim().is_empty() {
            return Ok(());
        }
        let mut msg = self.messages.borrow_mut();
        let tpl = msg[0];
        msg.rotate_left(1);

        let msg = match tpl {
            MsgTemplate::MustNotBeEmpty => format!("The {} must not be empty", self.entity),
            MsgTemplate::EnterANonEmptyName => format!("Please enter a non-empty {}", self.entity),
        };

        Err(msg)
    }
}

trait Seat {
    type Entity;

    fn kind() -> Kind;

    fn find<'a>(config: &'a Config, id: &str) -> Option<&'a Self::Entity>;
}

#[derive(Copy, Clone)]
struct DriverSeat;

impl Seat for DriverSeat {
    type Entity = Driver;

    fn kind() -> Kind {
        Kind::Driver
    }

    fn find<'a>(config: &'a Config, id: &str) -> Option<&'a Self::Entity> {
        config.drivers.iter().find(|n| &*n.navigator.alias == id)
    }
}

#[derive(Copy, Clone)]
struct NavigatorSeat;

impl Seat for NavigatorSeat {
    type Entity = Navigator;

    fn kind() -> Kind {
        Kind::Navigator
    }

    fn find<'a>(config: &'a Config, id: &str) -> Option<&'a Self::Entity> {
        config.navigators.iter().find(|n| &*n.alias == id)
    }
}

#[derive(Copy, Clone)]
enum CheckMode {
    MustNotExist,
    MustExist,
}

#[derive(Copy, Clone)]
struct Lookup<'a, T> {
    config: &'a Config,
    check: CheckMode,
    _kind: PhantomData<T>,
}

impl<'a, T: Seat> Lookup<'a, T> {
    fn new(config: &'a Config, check: CheckMode) -> Self {
        Self {
            config,
            check,
            _kind: PhantomData,
        }
    }

    fn matching_navigator(&self, id: &str) -> Option<&'a T::Entity> {
        T::find(&self.config, id)
    }
}

impl<T: Seat> Validator<String> for Lookup<'_, T> {
    type Err = color_eyre::eyre::Report;

    fn validate(&self, input: &String) -> Result<(), Self::Err> {
        let input = input.as_str();
        let id_exists = self.matching_navigator(input).is_some();
        match self.check {
            CheckMode::MustNotExist => {
                if id_exists {
                    return Err(eyre!("Alias {} already exists", style(input).cyan()));
                }
            }
            CheckMode::MustExist => {
                if !id_exists {
                    return Err(eyre!("Alias {} does not exist", style(input).cyan()));
                }
            }
        }

        Ok(())
    }
}
