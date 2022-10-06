use core::fmt;

// use super::*;
use super::{PromptAlias, PromptText, SelectMany, SelectOne, Selectable};
use crate::{data::Kind, ui::validation::Validator};
use console::{style, Style, StyledObject};
use dialoguer::{theme::ColorfulTheme, theme::Theme, Input, MultiSelect, Select};
use eyre::Result;
use once_cell::sync::Lazy;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConsoleUi;

impl SelectOne for ConsoleUi {
    fn select_one(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<usize> {
        let mut select = Select::with_theme(&*THEME);
        select.with_prompt(format!(
            "Select one {}\n  Use arrows to select, [return] to confirm",
            kind
        ));

        for item in items {
            select.item(item.item);
        }

        let chosen = select.interact()?;

        Ok(chosen)
    }
}

impl SelectMany for ConsoleUi {
    fn select_many(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<Vec<usize>> {
        let mut ms = MultiSelect::with_theme(&*THEME);

        ms.with_prompt(format!(
            "Select any number {}(s)\n  Use [space] to select, [return] to confirm",
            kind
        ));
        for item in items {
            ms.item_checked(item.item, item.checked);
        }

        let chosen = ms.interact()?;
        Ok(chosen)
    }
}

impl PromptText for ConsoleUi {
    fn prompt_for_text<V: Validator>(
        &mut self,
        thing: &'static str,
        id: &str,
        initial: Option<String>,
        validator: V,
    ) -> Result<String> {
        let mut input = Input::<String>::with_theme(&*THEME);
        input
            .with_prompt(format!("The {} for {}\n", thing, style(id).cyan()))
            .allow_empty(true)
            .validate_with(adapt(validator));

        if let Some(initial) = initial {
            input.with_initial_text(initial);
        }
        let name = input.interact()?;
        Ok(name)
    }
}

impl PromptAlias for ConsoleUi {
    fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String> {
        let input = Input::<String>::with_theme(&*THEME)
        .with_prompt(format!("Please enter the alias for the {}.\n  The alias will be used as identifier for all other commands.\n", kind))
        .allow_empty(true)
        .validate_with(adapt(validator))
        .interact_text()?;

        Ok(input)
    }
}

fn adapt<V: Validator>(mut validator: V) -> impl FnMut(&String) -> Result<()> {
    move |input| validator.validate(input.as_str())
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
