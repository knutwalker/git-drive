#![allow(clippy::needless_pass_by_value)]

use crate::{
    config::Config,
    data::{Driver, Id, IdRef, Kind, Navigator, PartialNav},
    Result,
};
use validation::{AsciiOnly, CheckForEmpty, Lookup, Validator};

mod tui;
mod validation;

pub trait SelectOne {
    fn select_one(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<usize>;
}

pub trait SelectMany {
    fn select_many(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<Vec<usize>>;
}

pub trait PromptText {
    fn prompt_for_text<V: Validator>(
        &mut self,
        thing: &'static str,
        id: &str,
        initial: Option<String>,
        validator: V,
    ) -> Result<String>;
}

pub trait PromptAlias {
    fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String>;
}

#[derive(Copy, Clone, Debug)]
pub struct Selectable<'a> {
    item: &'a str,
    checked: bool,
}

pub fn complete_new_nav(
    mut ui: impl PromptAlias + PromptText + Sized,
    partial: PartialNav,
    config: &Config,
) -> Result<Navigator> {
    ui.complete_new_nav(partial, config)
}

pub fn complete_existing_nav(
    mut ui: impl PromptAlias + PromptText + Sized,
    partial: PartialNav,
    config: &Config,
) -> Result<Navigator> {
    ui.complete_existing_nav(partial, config)
}

pub fn complete_new_drv(
    mut ui: impl PromptAlias + PromptText + Sized,
    partial: PartialNav,
    config: &Config,
) -> Result<Driver> {
    ui.complete_new_drv(partial, config)
}

pub fn complete_existing_drv(
    mut ui: impl PromptAlias + PromptText + Sized,
    partial: PartialNav,
    config: &Config,
) -> Result<Driver> {
    ui.complete_existing_drv(partial, config)
}

pub fn select_id_from(
    mut ui: impl SelectOne,
    kind: Kind,
    config: &Config,
) -> Result<Option<&'_ Id>> {
    ui.select_id_from(kind, config)
}

pub fn select_ids_from<'config>(
    mut ui: impl SelectMany,
    kind: Kind,
    config: &'config Config,
    pre_selected: &[Id],
) -> Result<Vec<&'config Id>> {
    ui.select_ids_from(kind, config, pre_selected)
}

pub fn ui() -> impl SelectOne + SelectMany + PromptText + PromptAlias + Sized {
    tui::ConsoleUi
}

impl<'a, T: SelectOne> SelectOne for &'a mut T {
    fn select_one(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<usize> {
        T::select_one(self, kind, items)
    }
}

impl<'a, T: SelectMany> SelectMany for &'a mut T {
    fn select_many(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<Vec<usize>> {
        T::select_many(self, kind, items)
    }
}

impl<'a, T: PromptText> PromptText for &'a mut T {
    fn prompt_for_text<V: Validator>(
        &mut self,
        thing: &'static str,
        id: &str,
        initial: Option<String>,
        validator: V,
    ) -> Result<String> {
        T::prompt_for_text(self, thing, id, initial, validator)
    }
}

impl<'a, T: PromptAlias> PromptAlias for &'a mut T {
    fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String> {
        T::prompt_for_alias(self, kind, validator)
    }
}

trait SelectOneExt: SelectOne {
    fn select_id_from<'config>(
        &mut self,
        kind: Kind,
        config: &'config Config,
    ) -> Result<Option<&'config Id>> {
        let selectables = selectable_items(kind, config, &[]);
        if selectables.is_empty() {
            return Ok(None);
        }

        let chosen = self.select_one(kind, &selectables)?;

        let nav = match kind {
            Kind::Navigator => config.navigators.get(chosen).map(IdRef::id),
            Kind::Driver => config.drivers.get(chosen).map(IdRef::id),
        };
        Ok(nav)
    }
}

impl<T: SelectOne> SelectOneExt for T {}

trait SelectManyExt: SelectMany {
    fn select_ids_from<'config>(
        &mut self,
        kind: Kind,
        config: &'config Config,
        pre_selected: &[Id],
    ) -> Result<Vec<&'config Id>> {
        let selectable = selectable_items(kind, config, pre_selected);
        if selectable.is_empty() {
            return Ok(Vec::new());
        }

        let selection = self.select_many(kind, &selectable)?;

        let ids = match kind {
            Kind::Navigator => selection
                .into_iter()
                .filter_map(|idx| config.navigators.get(idx))
                .map(IdRef::id)
                .collect(),
            Kind::Driver => selection
                .into_iter()
                .filter_map(|idx| config.drivers.get(idx))
                .map(IdRef::id)
                .collect(),
        };
        Ok(ids)
    }
}

impl<T: SelectMany> SelectManyExt for T {}

fn selectable_items<'config>(
    kind: Kind,
    config: &'config Config,
    pre_select: &[Id],
) -> Vec<Selectable<'config>> {
    match kind {
        Kind::Navigator => config
            .navigators
            .iter()
            .map(|nav| Selectable {
                item: &nav.alias,
                checked: pre_select.contains(&nav.alias),
            })
            .collect(),
        Kind::Driver => config
            .drivers
            .iter()
            .map(|drv| Selectable {
                item: &drv.navigator.alias,
                checked: pre_select.contains(&drv.navigator.alias),
            })
            .collect(),
    }
}

trait PromptTextExt: PromptText + Sized {
    fn prompt_for(
        &mut self,
        thing: &'static str,
        id: &str,
        initial: Option<String>,
    ) -> Result<String> {
        let validator = CheckForEmpty::new(thing);
        let result = self.prompt_for_text(thing, id, initial, &validator)?;
        (&validator).validate(&result)?;

        Ok(result)
    }

    fn finish_nav(
        &mut self,
        alias: Id,
        name: Option<String>,
        email: Option<String>,
        existing: Option<&Navigator>,
    ) -> Result<Navigator> {
        let name = self.prompt_for(
            "name",
            &alias,
            name.or_else(|| existing.map(|n| n.name.clone())),
        )?;
        let email = self.prompt_for(
            "email",
            &alias,
            email.or_else(|| existing.map(|n| n.email.clone())),
        )?;

        Ok(Navigator { alias, name, email })
    }

    fn finish_drv(
        &mut self,
        alias: Id,
        name: Option<String>,
        email: Option<String>,
        key: Option<String>,
        existing: Option<&Driver>,
    ) -> Result<Driver> {
        let navigator = self.finish_nav(alias, name, email, existing.map(|d| &d.navigator))?;

        let key = self.prompt_for_text(
            "signing key",
            &navigator.alias,
            key.or_else(|| existing.and_then(|d| d.key.clone())),
            &CheckForEmpty::new("signing key").with_allow_empty(true),
        )?;

        let key = if key.is_empty() { None } else { Some(key) };

        Ok(Driver { navigator, key })
    }
}

impl<T: PromptText + Sized> PromptTextExt for T {}

trait PromptAliasExt: PromptAlias + PromptText + Sized {
    fn prompt_alias<'config, T: Seat + Copy>(
        &mut self,
        check: CheckMode,
        config: &'config Config,
        existing: Option<String>,
    ) -> Result<(Id, Option<&'config T::Entity>)> {
        let check_empty = CheckForEmpty::new("alias");
        let lookup = Lookup::<T>::new(config, check);
        let validator = AsciiOnly.and_then(lookup);
        let mut validator = (&check_empty).and_then(validator);

        let id = match existing {
            Some(id) => Id(id),
            None => Id(self.prompt_for_alias(T::kind(), validator)?),
        };

        validator.validate(&id.0)?;

        let existing = lookup.matching_navigator(&id);
        Ok((id, existing))
    }

    fn complete_nav(
        &mut self,
        check: CheckMode,
        new: PartialNav,
        config: &Config,
    ) -> Result<Navigator> {
        let PartialNav {
            id,
            name,
            email,
            key: _,
        } = new;

        let (alias, existing) = self.prompt_alias::<NavigatorSeat>(check, config, id)?;
        self.finish_nav(alias, name, email, existing)
    }

    fn complete_drv(
        &mut self,
        check: CheckMode,
        new: PartialNav,
        config: &Config,
    ) -> Result<Driver> {
        let PartialNav {
            id,
            name,
            email,
            key,
        } = new;

        let (alias, existing) = self.prompt_alias::<DriverSeat>(check, config, id)?;
        self.finish_drv(alias, name, email, key, existing)
    }

    fn complete_new_nav(&mut self, partial: PartialNav, config: &Config) -> Result<Navigator> {
        self.complete_nav(CheckMode::MustNotExist, partial, config)
    }

    fn complete_existing_nav(&mut self, partial: PartialNav, config: &Config) -> Result<Navigator> {
        self.complete_nav(CheckMode::MustExist, partial, config)
    }

    fn complete_new_drv(&mut self, partial: PartialNav, config: &Config) -> Result<Driver> {
        self.complete_drv(CheckMode::MustNotExist, partial, config)
    }

    fn complete_existing_drv(&mut self, partial: PartialNav, config: &Config) -> Result<Driver> {
        self.complete_drv(CheckMode::MustExist, partial, config)
    }
}

impl<T: PromptAlias + PromptText + Sized> PromptAliasExt for T {}

#[derive(Copy, Clone)]
enum CheckMode {
    MustNotExist,
    MustExist,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tests::{drv1, nav1, nav2};
    use std::cell::Cell;

    impl<A: PromptText, B> PromptText for (A, B) {
        fn prompt_for_text<V: Validator>(
            &mut self,
            thing: &'static str,
            id: &str,
            initial: Option<String>,
            validator: V,
        ) -> Result<String> {
            self.0.prompt_for_text(thing, id, initial, validator)
        }
    }

    impl<A, B: PromptAlias> PromptAlias for (A, B) {
        fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String> {
            self.1.prompt_for_alias(kind, validator)
        }
    }

    #[derive(Copy, Clone, Debug)]
    struct FromFn<F>(F);

    impl<F> SelectOne for FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<usize>,
    {
        fn select_one(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<usize> {
            (self.0)(kind, items)
        }
    }

    impl<F> SelectMany for FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<Vec<usize>>,
    {
        fn select_many(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<Vec<usize>> {
            (self.0)(kind, items)
        }
    }

    impl<F> PromptText for FromFn<F>
    where
        F: FnMut(&'static str, &str, Option<String>) -> Result<String>,
    {
        fn prompt_for_text<V: Validator>(
            &mut self,
            thing: &'static str,
            id: &str,
            initial: Option<String>,
            _validator: V,
        ) -> Result<String> {
            (self.0)(thing, id, initial)
        }
    }

    impl<F> PromptAlias for FromFn<F>
    where
        F: FnMut(Kind) -> Result<String>,
    {
        fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, _validator: V) -> Result<String> {
            (self.0)(kind)
        }
    }

    const fn select_one<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<usize>,
    {
        FromFn(f)
    }

    const fn select_many<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<Vec<usize>>,
    {
        FromFn(f)
    }

    const fn prompt_text<F>(f: F) -> FromFn<F>
    where
        F: FnMut(&'static str, &str, Option<String>) -> Result<String>,
    {
        FromFn(f)
    }

    const fn prompt_alias<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind) -> Result<String>,
    {
        FromFn(f)
    }

    struct NoUi;

    #[allow(unused)]
    impl SelectOne for NoUi {
        fn select_one(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<usize> {
            panic!("Unexpected call to select_one kind={kind} items={items:?}")
        }
    }

    impl SelectMany for NoUi {
        fn select_many(&mut self, kind: Kind, items: &[Selectable<'_>]) -> Result<Vec<usize>> {
            panic!("Unexpected call to select_many kind={kind} items={items:?}")
        }
    }

    impl PromptText for NoUi {
        fn prompt_for_text<V: Validator>(
            &mut self,
            thing: &'static str,
            id: &str,
            initial: Option<String>,
            _validator: V,
        ) -> Result<String> {
            panic!(
                "Unexpected call to prompt_for_text with thing={thing} id={id} initial={initial:?}"
            )
        }
    }

    impl PromptAlias for NoUi {
        fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, _validator: V) -> Result<String> {
            panic!("Unexpected call to prompt_for_text with kind={kind}")
        }
    }

    struct ColorGuard(bool);

    impl Drop for ColorGuard {
        fn drop(&mut self) {
            console::set_colors_enabled(self.0);
        }
    }

    fn disable_colors() -> impl Drop {
        let colors = console::colors_enabled();
        console::set_colors_enabled(false);
        ColorGuard(colors)
    }

    #[test]
    fn test_select_id_with_empty_options() {
        let config = Config::default();

        let selected = select_id_from(NoUi, Kind::Navigator, &config).unwrap();
        assert_eq!(selected, None);

        let selected = select_id_from(NoUi, Kind::Driver, &config).unwrap();
        assert_eq!(selected, None);
    }

    #[test]
    fn test_select_id() {
        let config = Config::from_iter([nav1(), nav2()]);
        let ui = select_one(|kind, items| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].item, "nav1");
            assert_eq!(items[0].checked, false);
            assert_eq!(items[1].item, "nav2");
            assert_eq!(items[1].checked, false);
            Ok(0)
        });

        let selected = select_id_from(ui, Kind::Navigator, &config).unwrap();

        assert_eq!(selected, Some(config.navigators[0].id()));
    }

    #[test]
    fn test_select_out_of_bounds() {
        let config = Config::from_iter([nav1()]);
        let ui = select_one(|_, _| Ok(usize::MAX));

        let selected = select_id_from(ui, Kind::Navigator, &config).unwrap();

        assert_eq!(selected, None);
    }

    #[test]
    fn test_select_ids_with_empty_options() {
        let config = Config::default();

        let selected = select_ids_from(NoUi, Kind::Navigator, &config, &[]).unwrap();
        assert_eq!(selected, Vec::<&Id>::new());

        let selected = select_ids_from(NoUi, Kind::Driver, &config, &[]).unwrap();
        assert_eq!(selected, Vec::<&Id>::new());
    }

    #[test]
    fn test_select_ids() {
        let config = Config::from_iter([nav1(), nav2()]);

        let ui = select_many(|kind, items| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].item, "nav1");
            assert_eq!(items[1].item, "nav2");
            assert_eq!(items[0].checked, false);
            assert_eq!(items[1].checked, true);
            Ok(vec![0, 1])
        });

        let selected =
            select_ids_from(ui, Kind::Navigator, &config, &[nav2().id().clone()]).unwrap();

        assert_eq!(selected, vec![nav1().id(), nav2().id()]);
    }

    #[test]
    fn test_select_ids_keeps_selection_order() {
        let config = Config::from_iter([nav1(), nav2()]);
        let ui = select_many(|_, _| Ok(vec![1, 0]));

        let selected = select_ids_from(ui, Kind::Navigator, &config, &[]).unwrap();

        assert_eq!(selected, vec![nav2().id(), nav1().id()]);
    }

    #[test]
    fn test_select_ids_out_of_bounds() {
        let config = Config::from_iter([nav1(), nav2()]);
        let ui = select_many(|_, _| Ok(vec![usize::MAX, usize::MAX - 1, usize::MAX - 2]));

        let selected = select_ids_from(ui, Kind::Navigator, &config, &[]).unwrap();

        assert_eq!(selected, Vec::<&Id>::new());
    }

    #[test]
    fn complete_new_nav_with_no_input() {
        let ask_for = Cell::new("alias");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, "alias");
            assert_eq!(initial.as_deref(), None);

            ask_for.set(match ask_for.get() {
                "name" => "email",
                "email" => "done",
                what => panic!("Unexpected thing: {what}"),
            });

            Ok(String::from(thing))
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(String::from("alias"))
        });

        let nav =
            complete_new_nav((text, alias), PartialNav::default(), &Config::default()).unwrap();

        assert_eq!(ask_for.get(), "done");
        assert_eq!(nav.alias.0, "alias");
        assert_eq!(nav.name, "name");
        assert_eq!(nav.email, "email");
    }

    #[test]
    fn complete_new_nav_with_name() {
        let ask_for = Cell::new("alias");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, "alias");

            ask_for.set(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    "email"
                }
                "email" => {
                    assert_eq!(initial.as_deref(), None);
                    "done"
                }
                what => panic!("Unexpected thing: {what}"),
            });

            Ok(String::from(thing))
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(String::from("alias"))
        });

        let nav = complete_new_nav(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &Config::default(),
        )
        .unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(nav.alias.0, "alias");
        assert_eq!(nav.name, "name");
        assert_eq!(nav.email, "email");
    }

    #[test]
    fn complete_new_nav_id_validation() {
        let alias = Cell::new("nav1");

        let prompt = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            Ok(String::from(alias.get()))
        });

        let _guard = disable_colors();

        let err = complete_new_nav(
            (NoUi, prompt),
            PartialNav::default(),
            &Config::from_iter([nav1()]),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(err, "Alias nav1 already exists.");

        alias.set("");

        let err = complete_new_nav((NoUi, prompt), PartialNav::default(), &Config::default())
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The alias must not be empty.");

        alias.set("Sörën");

        let err = complete_new_nav((NoUi, prompt), PartialNav::default(), &Config::default())
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The input must be ASCII only.");
    }

    #[test]
    fn complete_existing_nav_with_no_input() {
        let ask_for = Cell::new("alias");
        let navigator = nav1();

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, navigator.id().as_ref());

            Ok(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some(navigator.name.as_str()));
                    ask_for.set("email");
                    navigator.name.clone()
                }
                "email" => {
                    assert_eq!(initial.as_deref(), Some(navigator.email.as_str()));
                    ask_for.set("done");
                    navigator.email.clone()
                }
                what => panic!("Unexpected thing: {what}"),
            })
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(navigator.alias.0.clone())
        });

        let config = Config::from_iter([nav1()]);

        let nav = complete_existing_nav((text, alias), PartialNav::default(), &config).unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(nav, navigator);
    }

    #[test]
    fn complete_existing_nav_with_name() {
        let ask_for = Cell::new("alias");
        let navigator = nav1();

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, navigator.id().as_ref());

            Ok(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    ask_for.set("email");
                    navigator.name.clone()
                }
                "email" => {
                    assert_eq!(initial.as_deref(), Some(navigator.email.as_str()));
                    ask_for.set("done");
                    navigator.email.clone()
                }
                what => panic!("Unexpected thing: {what}"),
            })
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(navigator.alias.0.clone())
        });

        let config = Config::from_iter([nav1()]);

        let nav = complete_existing_nav(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &config,
        )
        .unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(nav, navigator);
    }

    #[test]
    fn complete_existing_nav_id_validation() {
        let alias = Cell::new("nav2");

        let prompt = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            Ok(String::from(alias.get()))
        });

        let _guard = disable_colors();

        let err = complete_existing_nav(
            (NoUi, prompt),
            PartialNav::default(),
            &Config::from_iter([nav1()]),
        )
        .unwrap_err()
        .to_string();

        assert_eq!(err, "Alias nav2 does not exist.");

        alias.set("");

        let err = complete_existing_nav((NoUi, prompt), PartialNav::default(), &Config::default())
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The alias must not be empty.");

        alias.set("Sörën");

        let err = complete_existing_nav((NoUi, prompt), PartialNav::default(), &Config::default())
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The input must be ASCII only.");
    }

    #[test]
    fn complete_new_drv_with_no_input() {
        let ask_for = Cell::new("alias");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(initial.as_deref(), None);
            assert_eq!(id, "alias");

            ask_for.set(match ask_for.get() {
                "name" => "email",
                "email" => "signing key",
                "signing key" => "done",
                what => panic!("Unexpected thing: {what}"),
            });

            Ok(String::from(thing))
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(String::from("alias"))
        });

        let drv =
            complete_new_drv((text, alias), PartialNav::default(), &Config::default()).unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(drv.navigator.alias.0, "alias");
        assert_eq!(drv.navigator.name, "name");
        assert_eq!(drv.navigator.email, "email");
        assert_eq!(drv.key.as_deref(), Some("signing key"));
    }

    #[test]
    fn complete_new_drv_with_name() {
        let ask_for = Cell::new("alias");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, "alias");

            ask_for.set(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    "email"
                }
                "email" => {
                    assert_eq!(initial.as_deref(), None);
                    "signing key"
                }
                "signing key" => {
                    assert_eq!(initial.as_deref(), None);
                    "done"
                }
                what => panic!("Unexpected thing: {what}"),
            });

            Ok(String::from(thing))
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(String::from("alias"))
        });

        let drv = complete_new_drv(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &Config::default(),
        )
        .unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(drv.navigator.alias.0, "alias");
        assert_eq!(drv.navigator.name, "name");
        assert_eq!(drv.navigator.email, "email");
        assert_eq!(drv.key.as_deref(), Some("signing key"));
    }

    #[test]
    fn complete_new_drv_empty_key_is_none() {
        let text = prompt_text(|thing, _id, _initial| match thing {
            "signing key" => Ok(String::new()),
            otherwise => Ok(String::from(otherwise)),
        });

        let alias = prompt_alias(|_kind| Ok(String::from("alias")));

        let drv =
            complete_new_drv((text, alias), PartialNav::default(), &Config::default()).unwrap();

        assert_eq!(drv.key, None);
    }

    #[test]
    fn complete_new_drv_id_validation() {
        let alias = Cell::new("drv1");

        let prompt = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            Ok(String::from(alias.get()))
        });

        let _guard = disable_colors();

        let config = Config::from_iter([drv1(None)]);

        let err = complete_new_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "Alias drv1 already exists.");

        alias.set("");

        let err = complete_new_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The alias must not be empty.");

        alias.set("Sörën");

        let err = complete_new_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The input must be ASCII only.");
    }

    #[test]
    fn complete_existing_drv_with_no_input() {
        let ask_for = Cell::new("alias");
        let driver = drv1("a key");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, driver.id().as_ref());

            Ok(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.name.as_str()));
                    ask_for.set("email");
                    driver.navigator.name.clone()
                }
                "email" => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.email.as_str()));
                    ask_for.set("signing key");
                    driver.navigator.email.clone()
                }
                "signing key" => {
                    assert_eq!(initial.as_deref(), driver.key.as_deref());
                    ask_for.set("done");
                    driver.key.clone().unwrap_or_default()
                }
                what => panic!("Unexpected thing: {what}"),
            })
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(driver.navigator.alias.0.clone())
        });

        let config = Config::from_iter([driver.clone()]);

        let drv = complete_existing_drv((text, alias), PartialNav::default(), &config).unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(drv, driver);
    }

    #[test]
    fn complete_existing_drv_with_name() {
        let ask_for = Cell::new("alias");
        let driver = drv1("a key");

        let text = prompt_text(|thing, id, initial| {
            assert_eq!(thing, ask_for.get());
            assert_eq!(id, driver.id().as_ref());

            Ok(match ask_for.get() {
                "name" => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    ask_for.set("email");
                    driver.navigator.name.clone()
                }
                "email" => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.email.as_str()));
                    ask_for.set("signing key");
                    driver.navigator.email.clone()
                }
                "signing key" => {
                    assert_eq!(initial.as_deref(), driver.key.as_deref());
                    ask_for.set("done");
                    driver.key.clone().unwrap_or_default()
                }
                what => panic!("Unexpected thing: {what}"),
            })
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(ask_for.get(), "alias");
            ask_for.set("name");
            Ok(driver.navigator.alias.0.clone())
        });

        let config = Config::from_iter([driver.clone()]);

        let drv = complete_existing_drv(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &config,
        )
        .unwrap();
        assert_eq!(ask_for.get(), "done");
        assert_eq!(drv, driver);
    }

    #[test]
    fn complete_existing_drv_empty_key_is_none() {
        let text = prompt_text(|thing, _id, _initial| match thing {
            "signing key" => Ok(String::new()),
            otherwise => Ok(String::from(otherwise)),
        });

        let alias = prompt_alias(|_kind| Ok(String::from("drv1")));

        let config = Config::from_iter([drv1(None)]);

        let drv = complete_existing_drv((text, alias), PartialNav::default(), &config).unwrap();

        assert_eq!(drv.key, None);
    }

    #[test]
    fn complete_existing_drv_id_validation() {
        let alias = Cell::new("drv2");

        let prompt = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            Ok(String::from(alias.get()))
        });

        let _guard = disable_colors();

        let config = Config::from_iter([drv1(None)]);

        let err = complete_existing_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "Alias drv2 does not exist.");

        alias.set("");

        let err = complete_existing_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The alias must not be empty.");

        alias.set("Sörën");

        let err = complete_existing_drv((NoUi, prompt), PartialNav::default(), &config)
            .unwrap_err()
            .to_string();

        assert_eq!(err, "The input must be ASCII only.");
    }
}
