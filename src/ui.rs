#![allow(clippy::needless_pass_by_value)]

use crate::{
    config::Config,
    data::{Driver, Field, Id, IdRef, Kind, Navigator, PartialNav},
};
use eyre::Result;
use validation::{CheckForEmpty, Lookup, Validator};

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
        field: Field,
        id: &str,
        initial: Option<String>,
        validator: V,
    ) -> Result<String>;
}

pub trait PromptAlias {
    fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String>;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Selectable<'a> {
    pub item: &'a str,
    pub checked: bool,
}

pub fn complete_new_nav(
    mut ui: impl PromptAlias + PromptText,
    partial: PartialNav,
    config: &Config,
) -> Result<Navigator> {
    ui.complete_new_nav(partial, config)
}

pub fn complete_existing_nav(
    mut ui: impl PromptAlias + PromptText,
    partial: PartialNav,
    config: &Config,
) -> Result<Navigator> {
    ui.complete_existing_nav(partial, config)
}

pub fn complete_new_drv(
    mut ui: impl PromptAlias + PromptText,
    partial: PartialNav,
    config: &Config,
) -> Result<Driver> {
    ui.complete_new_drv(partial, config)
}

pub fn complete_existing_drv(
    mut ui: impl PromptAlias + PromptText,
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
        field: Field,
        id: &str,
        initial: Option<String>,
        validator: V,
    ) -> Result<String> {
        T::prompt_for_text(self, field, id, initial, validator)
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
    fn prompt_for(&mut self, field: Field, id: &str, initial: Option<String>) -> Result<String> {
        let validator = CheckForEmpty::new(field);
        let result = self.prompt_for_text(field, id, initial, &validator)?;
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
            Field::Name,
            &alias,
            name.or_else(|| existing.map(|n| n.name.clone())),
        )?;
        let email = self.prompt_for(
            Field::Email,
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
            Field::Key,
            &navigator.alias,
            key.or_else(|| existing.and_then(|d| d.key.clone())),
            &CheckForEmpty::new(Field::Key).with_allow_empty(true),
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
        let check_empty = CheckForEmpty::new(Field::Alias);
        let lookup = Lookup::<T>::new(config, check);
        let mut validator = (&check_empty).and_then(lookup);

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
pub mod util {
    use std::cell::Cell;

    use crate::data::Field;

    use super::{
        Kind, PromptAlias, PromptText, Result, SelectMany, SelectOne, Selectable, Validator,
    };

    impl<A: PromptText, B> PromptText for (A, B) {
        fn prompt_for_text<V: Validator>(
            &mut self,
            field: Field,
            id: &str,
            initial: Option<String>,
            validator: V,
        ) -> Result<String> {
            self.0.prompt_for_text(field, id, initial, validator)
        }
    }

    impl<A, B: PromptAlias> PromptAlias for (A, B) {
        fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, validator: V) -> Result<String> {
            self.1.prompt_for_alias(kind, validator)
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct FromFn<F>(F);

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
        F: FnMut(Field, &str, Option<String>) -> Result<String>,
    {
        fn prompt_for_text<V: Validator>(
            &mut self,
            field: Field,
            id: &str,
            initial: Option<String>,
            _validator: V,
        ) -> Result<String> {
            (self.0)(field, id, initial)
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

    pub fn select_one<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<usize>,
    {
        FromFn(f)
    }

    pub fn select_many<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind, &[Selectable<'_>]) -> Result<Vec<usize>>,
    {
        FromFn(f)
    }

    pub fn prompt_text<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Field, &str, Option<String>) -> Result<String>,
    {
        FromFn(f)
    }

    pub fn prompt_alias<F>(f: F) -> FromFn<F>
    where
        F: FnMut(Kind) -> Result<String>,
    {
        FromFn(f)
    }

    pub struct NoUi;

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
            field: Field,
            id: &str,
            initial: Option<String>,
            _validator: V,
        ) -> Result<String> {
            panic!(
                "Unexpected call to prompt_for_text with field={field} id={id} initial={initial:?}"
            )
        }
    }

    impl PromptAlias for NoUi {
        fn prompt_for_alias<V: Validator>(&mut self, kind: Kind, _validator: V) -> Result<String> {
            panic!("Unexpected call to prompt_for_text with kind={kind}")
        }
    }

    impl<T> std::ops::Shr<T> for Field {
        type Output = (Self, T);

        fn shr(self, rhs: T) -> Self::Output {
            (self, rhs)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct ExpectField(Cell<Field>);

    impl ExpectField {
        pub fn new(field: Field) -> Self {
            Self(Cell::new(field))
        }

        pub fn expect(&self, field: Field) {
            assert_eq!(self.0.get(), field);
        }

        pub fn expect_done(&self) {
            assert_eq!(self.0.get(), Field::Done);
        }

        pub fn next(&self, expect: Field, next: Field) {
            match self.0.get() {
                f if f == expect => self.0.set(next),
                f => panic!("Expected field {} but got {}", expect, f),
            }
        }

        pub fn with(&self, mut check: impl FnMut(Field) -> Field) {
            let current = self.0.get();
            match check(current) {
                Field::Unexpected => panic!("Unexpected field: {}", current),
                otherwise => self.0.set(otherwise),
            }
        }

        pub fn get<T>(&self, mut fun: impl FnMut(Field) -> Option<T>) -> T {
            let current = self.0.get();
            match fun(current) {
                Some(value) => value,
                None => panic!("Unexpected field: {}", current),
            }
        }
    }

    #[macro_export]
    macro_rules! next {
        ($($pat:pat => $expr:expr),+ $(,)?) => {
            |field| match field {
                $($pat => $expr,)+
                _ => Field::Unexpected,
            }
        };
    }

    #[macro_export]
    macro_rules! partial {
        ($($pat:pat => $expr:expr),+ $(,)?) => {
            |field| match field {
                $($pat => Some($expr),)+
                _ => None,
            }
        };
    }

    pub struct ColorGuard(bool);

    impl Drop for ColorGuard {
        fn drop(&mut self) {
            console::set_colors_enabled(self.0);
        }
    }

    pub fn disable_colors() -> impl Drop {
        let colors = console::colors_enabled();
        console::set_colors_enabled(false);
        ColorGuard(colors)
    }
}

#[cfg(test)]
mod tests {
    use super::util::{disable_colors, prompt_alias, prompt_text, select_many, select_one, NoUi};
    use super::*;
    use crate::{
        data::tests::{drv1, nav1, nav2},
        next, partial,
        ui::util::ExpectField,
    };
    use std::cell::Cell;

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
        let ask_for = ExpectField::new(Field::Alias);

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, "alias");
            assert_eq!(initial.as_deref(), None);

            ask_for.expect(field);
            ask_for.with(next! {
                Field::Name => Field::Email,
                Field::Email => Field::Done,
            });

            Ok(field.to_string())
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            ask_for.next(Field::Alias, Field::Name);
            Ok(String::from("alias"))
        });

        let nav =
            complete_new_nav((text, alias), PartialNav::default(), &Config::default()).unwrap();

        ask_for.expect_done();
        assert_eq!(nav.alias.0, "alias");
        assert_eq!(nav.name, "name");
        assert_eq!(nav.email, "email");
    }

    #[test]
    fn complete_new_nav_with_name() {
        let ask_for = ExpectField::new(Field::Alias);

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, "alias");

            ask_for.expect(field);
            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), None);
                    Field::Done
                },
            });

            Ok(field.to_string())
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            ask_for.next(Field::Alias, Field::Name);
            Ok(String::from("alias"))
        });

        let nav = complete_new_nav(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &Config::default(),
        )
        .unwrap();
        ask_for.expect_done();
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
    }

    #[test]
    fn complete_existing_nav_with_no_input() {
        let ask_for = ExpectField::new(Field::Alias);
        let navigator = nav1();

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, navigator.id().as_ref());
            ask_for.expect(field);

            let value = ask_for.get(partial! {
                Field::Name => navigator.name.clone(),
                Field::Email => navigator.email.clone(),
            });

            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some(navigator.name.as_str()));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), Some(navigator.email.as_str()));
                    Field::Done
                },
            });

            Ok(value)
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            ask_for.next(Field::Alias, Field::Name);
            Ok(navigator.alias.0.clone())
        });

        let config = Config::from_iter([nav1()]);

        let nav = complete_existing_nav((text, alias), PartialNav::default(), &config).unwrap();
        ask_for.expect_done();
        assert_eq!(nav, navigator);
    }

    #[test]
    fn complete_existing_nav_with_name() {
        let ask_for = ExpectField::new(Field::Alias);
        let navigator = nav1();

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, navigator.id().as_ref());
            ask_for.expect(field);

            let value = ask_for.get(partial! {
                Field::Name => navigator.name.clone(),
                Field::Email => navigator.email.clone(),
            });

            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), Some(navigator.email.as_str()));
                    Field::Done
                },
            });

            Ok(value)
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            ask_for.next(Field::Alias, Field::Name);
            Ok(navigator.alias.0.clone())
        });

        let config = Config::from_iter([nav1()]);

        let nav = complete_existing_nav(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &config,
        )
        .unwrap();
        ask_for.expect_done();
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
    }

    #[test]
    fn complete_new_drv_with_no_input() {
        let ask_for = ExpectField::new(Field::Alias);

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, "alias");
            assert_eq!(initial.as_deref(), None);

            ask_for.expect(field);
            ask_for.with(next! {
                Field::Name => Field::Email,
                Field::Email => Field::Key,
                Field::Key => Field::Done,
            });

            Ok(field.to_string())
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            ask_for.next(Field::Alias, Field::Name);
            Ok(String::from("alias"))
        });

        let drv =
            complete_new_drv((text, alias), PartialNav::default(), &Config::default()).unwrap();
        ask_for.expect_done();
        assert_eq!(drv.navigator.alias.0, "alias");
        assert_eq!(drv.navigator.name, "name");
        assert_eq!(drv.navigator.email, "email");
        assert_eq!(drv.key.as_deref(), Some("signing key"));
    }

    #[test]
    fn complete_new_drv_with_name() {
        let ask_for = ExpectField::new(Field::Alias);

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, "alias");

            ask_for.expect(field);
            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), None);
                    Field::Key
                },
                Field::Key => {
                    assert_eq!(initial.as_deref(), None);
                    Field::Done
                },
            });

            Ok(field.to_string())
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            ask_for.next(Field::Alias, Field::Name);
            Ok(String::from("alias"))
        });

        let drv = complete_new_drv(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &Config::default(),
        )
        .unwrap();
        ask_for.expect_done();
        assert_eq!(drv.navigator.alias.0, "alias");
        assert_eq!(drv.navigator.name, "name");
        assert_eq!(drv.navigator.email, "email");
        assert_eq!(drv.key.as_deref(), Some("signing key"));
    }

    #[test]
    fn complete_new_drv_empty_key_is_none() {
        let text = prompt_text(|field, _id, _initial| match field {
            Field::Key => Ok(String::new()),
            otherwise => Ok(otherwise.to_string()),
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
    }

    #[test]
    fn complete_existing_drv_with_no_input() {
        let ask_for = ExpectField::new(Field::Alias);
        let driver = drv1("a key");

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, driver.id().as_ref());
            ask_for.expect(field);

            let value = ask_for.get(partial! {
                Field::Name => driver.navigator.name.clone(),
                Field::Email => driver.navigator.email.clone(),
                Field::Key => driver.key.clone().unwrap_or_default(),
            });

            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.name.as_str()));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.email.as_str()));
                    Field::Key
                },
                Field::Key => {
                    assert_eq!(initial.as_deref(), driver.key.as_deref());
                    Field::Done
                },
            });

            Ok(value)
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            ask_for.next(Field::Alias, Field::Name);
            Ok(driver.navigator.alias.0.clone())
        });

        let config = Config::from_iter([driver.clone()]);

        let drv = complete_existing_drv((text, alias), PartialNav::default(), &config).unwrap();
        ask_for.expect_done();
        assert_eq!(drv, driver);
    }

    #[test]
    fn complete_existing_drv_with_name() {
        let ask_for = ExpectField::new(Field::Alias);
        let driver = drv1("a key");

        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, driver.id().as_ref());
            ask_for.expect(field);

            let value = ask_for.get(partial! {
                Field::Name => driver.navigator.name.clone(),
                Field::Email => driver.navigator.email.clone(),
                Field::Key => driver.key.clone().unwrap_or_default(),
            });

            ask_for.with(next! {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some("some name"));
                    Field::Email
                },
                Field::Email => {
                    assert_eq!(initial.as_deref(), Some(driver.navigator.email.as_str()));
                    Field::Key
                },
                Field::Key => {
                    assert_eq!(initial.as_deref(), driver.key.as_deref());
                    Field::Done
                },
            });

            Ok(value)
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Driver);
            ask_for.next(Field::Alias, Field::Name);
            Ok(driver.navigator.alias.0.clone())
        });

        let config = Config::from_iter([driver.clone()]);

        let drv = complete_existing_drv(
            (text, alias),
            PartialNav::default().with_name(String::from("some name")),
            &config,
        )
        .unwrap();
        ask_for.expect_done();
        assert_eq!(drv, driver);
    }

    #[test]
    fn complete_existing_drv_empty_key_is_none() {
        let text = prompt_text(|field, _id, _initial| match field {
            Field::Key => Ok(String::new()),
            otherwise => Ok(otherwise.to_string()),
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
    }
}
