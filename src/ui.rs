use crate::{
    config::Config,
    data::{Driver, Id, Kind, Navigator, PartialNav},
    Result,
};
use console::{style, Style, StyledObject};
use dialoguer::{theme::ColorfulTheme, theme::Theme, Input, MultiSelect, Select, Validator};
use eyre::eyre;
use once_cell::sync::Lazy;
use std::{fmt, marker::PhantomData, option::Option};

pub fn select_ids_from<'config>(
    kind: Kind,
    config: &'config Config,
    pre_selected: &[Id],
) -> Result<Vec<&'config Id>> {
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

fn select_from(kind: Kind, config: &Config, pre_select: &[Id]) -> Result<Vec<usize>> {
    let mut ms = MultiSelect::with_theme(&*THEME);
    ms.with_prompt(format!(
        "Select any number {}(s)\n  Use [space] to select, [return] to confirm",
        kind
    ));
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

pub fn select_id_from(kind: Kind, config: &Config) -> Result<Id> {
    let mut select = Select::with_theme(&*THEME);
    select.with_prompt(format!(
        "Select one {}\n  Use arrows to select, [return] to confirm",
        kind
    ));

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

pub fn prompt_for_empty(
    thing: &'static str,
    id: &str,
    initial: Option<String>,
    allow_empty: bool,
) -> Result<String> {
    let mut input = Input::<String>::with_theme(&*THEME);
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

pub fn prompt_for(thing: &'static str, id: &str, initial: Option<String>) -> Result<String> {
    prompt_for_empty(thing, id, initial, false)
}

pub fn complete_new_nav(partial: PartialNav, config: &Config) -> Result<Navigator> {
    complete_nav(CheckMode::MustNotExist, partial, config)
}

pub fn complete_existing_nav(partial: PartialNav, config: &Config) -> Result<Navigator> {
    complete_nav(CheckMode::MustExist, partial, config)
}

pub fn complete_new_drv(partial: PartialNav, config: &Config) -> Result<Driver> {
    complete_drv(CheckMode::MustNotExist, partial, config)
}

pub fn complete_existing_drv(partial: PartialNav, config: &Config) -> Result<Driver> {
    complete_drv(CheckMode::MustExist, partial, config)
}

fn prompt_alias<T: Seat + Copy>(
    check: CheckMode,
    config: &Config,
    existing: Option<String>,
) -> Result<(Id, Option<&'_ T::Entity>)> {
    let mut lookup = Lookup::<T>::new(config, check);
    let id = if let Some(id) = existing {
        lookup.validate(&id)?;
        Id(id)
    } else {
        let input = Input::<String>::with_theme(&*THEME)
                .with_prompt(format!("Please enter the alias for the {}.\n  The alias will be used as identifier for all other commands.\n", T::kind()))
                .allow_empty(true)
                .validate_with(CheckForEmpty::new("alias", false))
                .validate_with(lookup)
                .interact_text()?;
        Id(input)
    };

    let existing = lookup.matching_navigator(&id);
    Ok((id, existing))
}

fn complete_nav(check: CheckMode, new: PartialNav, config: &Config) -> Result<Navigator> {
    let PartialNav {
        id,
        name,
        email,
        key: _,
    } = new;

    let (alias, existing) = prompt_alias::<NavigatorSeat>(check, config, id)?;
    finish_nav(alias, name, email, existing)
}

fn finish_nav(
    alias: Id,
    name: Option<String>,
    email: Option<String>,
    existing: Option<&Navigator>,
) -> Result<Navigator> {
    let name = prompt_for(
        "name",
        &alias,
        name.or_else(|| existing.map(|n| n.name.clone())),
    )?;
    let email = prompt_for(
        "email",
        &alias,
        email.or_else(|| existing.map(|n| n.email.clone())),
    )?;

    Ok(Navigator { alias, name, email })
}

fn complete_drv(check: CheckMode, new: PartialNav, config: &Config) -> Result<Driver> {
    let PartialNav {
        id,
        name,
        email,
        key,
    } = new;

    let (alias, existing) = prompt_alias::<DriverSeat>(check, config, id)?;
    let navigator = finish_nav(alias, name, email, existing.map(|d| &d.navigator))?;

    let key = prompt_for_empty(
        "signing key",
        &navigator.alias,
        key.or_else(|| existing.and_then(|d| d.key.clone())),
        true,
    )?;
    let key = if key.is_empty() { None } else { Some(key) };

    Ok(Driver { navigator, key })
}

#[derive(Copy, Clone, Debug)]
enum MsgTemplate {
    MustNotBeEmpty,
    EnterANonEmptyName,
}

#[derive(Copy, Clone, Debug)]
struct CheckForEmpty {
    messages: [MsgTemplate; 2],
    entity: &'static str,
    allow_empty: bool,
}

impl CheckForEmpty {
    const fn new(entity: &'static str, allow_empty: bool) -> Self {
        Self {
            messages: [MsgTemplate::MustNotBeEmpty, MsgTemplate::EnterANonEmptyName],
            entity,
            allow_empty,
        }
    }
}

impl Validator<String> for CheckForEmpty {
    type Err = String;

    fn validate(&mut self, input: &String) -> Result<(), Self::Err> {
        if self.allow_empty || !input.trim().is_empty() {
            return Ok(());
        }

        let tpl = self.messages[0];
        self.messages.rotate_left(1);

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
    const fn new(config: &'a Config, check: CheckMode) -> Self {
        Self {
            config,
            check,
            _kind: PhantomData,
        }
    }

    fn matching_navigator(&self, id: &str) -> Option<&'a T::Entity> {
        T::find(self.config, id)
    }
}

impl<T: Seat> Validator<String> for Lookup<'_, T> {
    type Err = eyre::Report;

    fn validate(&mut self, input: &String) -> Result<(), Self::Err> {
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

static THEME: Lazy<PrettyTheme> = Lazy::new(|| PrettyTheme {
    theme: ColorfulTheme {
        defaults_style: Style::new().for_stderr().cyan(),
        prompt_style: Style::new().for_stderr().bold(),
        prompt_prefix: style("?".to_string()).for_stderr().yellow(),
        prompt_suffix: style("❯".to_string()).for_stderr().cyan(),
        success_prefix: style("✔".to_string()).for_stderr().green(),
        success_suffix: style(String::new()).for_stderr().green().dim(),
        error_prefix: style("✘ ".to_string()).for_stderr().red(),
        error_style: Style::new().for_stderr().red(),
        hint_style: Style::new().for_stderr().cyan().dim(),
        values_style: Style::new().for_stderr().green(),
        active_item_style: Style::new().for_stderr().cyan(),
        inactive_item_style: Style::new().for_stderr(),
        active_item_prefix: style("❯".to_string()).for_stderr().cyan(),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        checked_item_prefix: style("[✔]".to_string()).for_stderr().green(),
        unchecked_item_prefix: style("[ ]".to_string()).for_stderr(),
        picked_item_prefix: style(" ❯".to_string()).for_stderr().green(),
        unpicked_item_prefix: style("  ".to_string()).for_stderr(),
        inline_selections: true,
    },
    active_and_checked_item_prefix: style("[✔]".to_string()).for_stderr().cyan(),
});

struct PrettyTheme {
    theme: ColorfulTheme,
    active_and_checked_item_prefix: StyledObject<String>,
}

impl Theme for PrettyTheme {
    fn format_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        Theme::format_prompt(&self.theme, f, prompt)
    }

    fn format_error(&self, f: &mut dyn fmt::Write, err: &str) -> fmt::Result {
        Theme::format_error(&self.theme, f, err)
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> fmt::Result {
        Theme::format_confirm_prompt(&self.theme, f, prompt, default)
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> fmt::Result {
        Theme::format_confirm_prompt_selection(&self.theme, f, prompt, selection)
    }

    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        Theme::format_input_prompt(&self.theme, f, prompt, default)
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> fmt::Result {
        Theme::format_input_prompt_selection(&self.theme, f, prompt, sel)
    }

    fn format_select_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        write!(f, "  {}", self.theme.prompt_style.apply_to(prompt))
    }

    fn format_select_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> fmt::Result {
        Theme::format_select_prompt_selection(&self.theme, f, prompt, sel)
    }

    fn format_multi_select_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        write!(f, "  {}", self.theme.prompt_style.apply_to(prompt))
    }

    fn format_sort_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        Theme::format_sort_prompt(&self.theme, f, prompt)
    }

    fn format_multi_select_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selections: &[&str],
    ) -> fmt::Result {
        Theme::format_multi_select_prompt_selection(&self.theme, f, prompt, selections)
    }

    fn format_sort_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selections: &[&str],
    ) -> fmt::Result {
        Theme::format_sort_prompt_selection(&self.theme, f, prompt, selections)
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        Theme::format_select_prompt_item(&self.theme, f, text, active)
    }

    fn format_multi_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        checked: bool,
        active: bool,
    ) -> fmt::Result {
        let (active, checked, text) = match (active, checked) {
            (true, true) => (
                &self.theme.active_item_prefix,
                &self.active_and_checked_item_prefix,
                self.theme.active_item_style.apply_to(text),
            ),
            (true, false) => (
                &self.theme.active_item_prefix,
                &self.theme.unchecked_item_prefix,
                self.theme.active_item_style.apply_to(text),
            ),
            (false, true) => (
                &self.theme.inactive_item_prefix,
                &self.theme.checked_item_prefix,
                self.theme.values_style.apply_to(text),
            ),
            (false, false) => (
                &self.theme.inactive_item_prefix,
                &self.theme.unchecked_item_prefix,
                self.theme.inactive_item_style.apply_to(text),
            ),
        };
        write!(f, "{} {} {}", active, checked, text)
    }

    fn format_sort_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        picked: bool,
        active: bool,
    ) -> fmt::Result {
        Theme::format_sort_prompt_item(&self.theme, f, text, picked, active)
    }
}
