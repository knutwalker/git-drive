use crate::{
    config::Config,
    data::{Kind, Modification, PartialIdNav, PartialNav},
    ui::{self, PromptAlias, PromptText, SelectOne},
};
use eyre::{bail, Result};

pub fn run(
    mut ui: impl SelectOne + PromptText + PromptAlias,
    kind: Kind,
    config: &mut Config,
    mut new: PartialNav,
) -> Result<Modification> {
    let id = match new.id.take() {
        Some(id) => id,
        None => select_existing_id(&mut ui, kind, config)?,
    };

    let partial = PartialIdNav::new(id).merge(new);

    edit(ui, kind, config, partial)
}

fn select_existing_id(ui: impl SelectOne, kind: Kind, config: &Config) -> Result<String> {
    let id = ui::select_id_from(ui, kind, config)?;
    match id {
        Some(id) => Ok(id.0.clone()),
        None => {
            // TODO: proper error type
            bail!("No {}s to edit", kind)
        }
    }
}

fn edit(
    ui: impl PromptText,
    kind: Kind,
    config: &mut Config,
    new: PartialIdNav,
) -> Result<Modification> {
    match kind {
        Kind::Navigator => {
            let navigator = ui::complete_existing_nav(ui, new, config)?;
            let nav = config
                .navigators
                .iter_mut()
                .find(|n| navigator.alias.same_as_nav(n))
                .expect("validated during complete_existing_nav");

            if navigator != *nav {
                *nav = navigator;
                return Ok(Modification::Changed);
            }
        }
        Kind::Driver => {
            let driver = ui::complete_existing_drv(ui, new, config)?;
            let drv = config
                .drivers
                .iter_mut()
                .find(|d| driver.navigator.alias.same_as_drv(d))
                .expect("validated during complete_existing_drv");

            if driver != *drv {
                *drv = driver;
                return Ok(Modification::Changed);
            }
        }
    };

    Ok(Modification::Unchanged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data::{
            tests::{drv1, nav1, nav2},
            Driver, Field, Id, Navigator,
        },
        ui::{
            util::{disable_colors, prompt_text, select_one, AssertPromptText, Initial, NoUi},
            Selectable,
        },
    };

    #[test]
    fn test_select_existing_navigator_from_empty_config() {
        let config = Config::default();
        let selected = select_existing_id(NoUi, Kind::Navigator, &config).unwrap_err();
        assert_eq!(selected.to_string(), "No navigators to edit");
    }

    #[test]
    fn test_select_existing_driver_from_empty_config() {
        let config = Config::default();
        let selected = select_existing_id(NoUi, Kind::Driver, &config).unwrap_err();
        assert_eq!(selected.to_string(), "No drivers to edit");
    }

    #[test]
    fn test_select_existing_navigator() {
        let ui = select_one(|kind, items| {
            assert_eq!(kind, Kind::Navigator);
            assert_eq!(
                items,
                &[
                    Selectable {
                        item: "nav1",
                        checked: false
                    },
                    Selectable {
                        item: "nav2",
                        checked: false
                    }
                ]
            );
            Ok(0)
        });
        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let selected = select_existing_id(ui, Kind::Navigator, &config).unwrap();
        assert_eq!(selected, "nav1");
    }

    #[test]
    fn test_select_existing_driver() {
        let ui = select_one(|kind, items| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(
                items,
                &[Selectable {
                    item: "drv1",
                    checked: false
                }]
            );
            Ok(0)
        });
        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let selected = select_existing_id(ui, Kind::Driver, &config).unwrap();
        assert_eq!(selected, "drv1");
    }

    #[test]
    fn test_select_out_of_bounds() {
        let ui = select_one(|_kind, _items| Ok(usize::MAX));
        let config = Config::from_iter([nav1(), nav2()]);
        let selected = select_existing_id(ui, Kind::Navigator, &config).unwrap_err();
        assert_eq!(selected.to_string(), "No navigators to edit");
    }

    #[test]
    fn test_edit_existing_navigator() {
        let mut text = AssertPromptText::start("nav1")
            .expect(Field::Name)
            .with_initial_value("bernd")
            .returns("new name")
            .expect(Field::Email)
            .with_initial_value("foo@bar.org")
            .returns("new email")
            .done();

        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = edit(
            text.as_ui(),
            Kind::Navigator,
            &mut config,
            PartialIdNav::new("nav1"),
        )
        .unwrap();

        text.expect_done();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(
            &config.navigators,
            &[
                Navigator {
                    alias: Id(String::from("nav1")),
                    name: String::from("new name"),
                    email: String::from("new email"),
                },
                nav2()
            ]
        );
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_edit_non_existing_navigator() {
        let _guard = disable_colors();

        let mut config = Config::from_iter([nav1()]);
        let result = edit(
            NoUi,
            Kind::Navigator,
            &mut config,
            PartialIdNav::new("does not exist"),
        )
        .unwrap_err();

        assert_eq!(result.to_string(), "Alias does not exist does not exist.");
        assert_eq!(config, Config::from_iter([nav1()]));
    }

    #[test]
    fn test_edit_existing_navigator_without_change() {
        let text = prompt_text(|_field, _id, initial| Ok(initial.unwrap()));

        let mut config = Config::from_iter([nav1()]);
        let modified = edit(
            text,
            Kind::Navigator,
            &mut config,
            PartialIdNav::new("nav1"),
        )
        .unwrap();

        assert_eq!(modified, Modification::Unchanged);
        assert_eq!(config, Config::from_iter([nav1()]));
    }

    #[test]
    fn test_edit_existing_navigator_with_provided_values() {
        let partial = PartialIdNav::new("nav1")
            .with_name("new name")
            .with_email("new email");

        let mut text = AssertPromptText::start("nav1")
            .expect(Field::Name)
            .with_initial_value(partial.name.as_deref().unwrap())
            .returns(Initial)
            .expect(Field::Email)
            .with_initial_value(partial.email.as_deref().unwrap())
            .returns(Initial)
            .done();

        let mut config = Config::from_iter([nav1()]);
        let modified = edit(text.as_ui(), Kind::Navigator, &mut config, partial.clone()).unwrap();

        text.expect_done();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(
            config,
            Config::from_iter([Navigator {
                alias: Id::from("nav1"),
                name: partial.name.unwrap(),
                email: partial.email.unwrap()
            }])
        );
    }

    #[test]
    fn test_edit_existing_driver() {
        let mut text = AssertPromptText::start("drv1")
            .expect(Field::Name)
            .with_initial_value("ralle")
            .returns("new name")
            .expect(Field::Email)
            .with_initial_value("qux@bar.org")
            .returns("new email")
            .expect(Field::Key)
            .with_initial_value("initial key")
            .returns("new key")
            .done();

        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1("initial key").ent()]);
        let modified = edit(
            text.as_ui(),
            Kind::Driver,
            &mut config,
            PartialIdNav::new("drv1"),
        )
        .unwrap();

        text.expect_done();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[nav1(), nav2()]);
        assert_eq!(
            &config.drivers,
            &[Driver {
                navigator: Navigator {
                    alias: Id(String::from("drv1")),
                    name: String::from("new name"),
                    email: String::from("new email"),
                },
                key: Some(String::from("new key"))
            }]
        );
    }

    #[test]
    fn test_edit_non_existing_driver() {
        let _guard = disable_colors();

        let mut config = Config::from_iter([drv1(None)]);
        let result = edit(
            NoUi,
            Kind::Driver,
            &mut config,
            PartialIdNav::new("does not exist"),
        )
        .unwrap_err();

        assert_eq!(result.to_string(), "Alias does not exist does not exist.");
        assert_eq!(config, Config::from_iter([drv1(None)]));
    }

    #[test]
    fn test_edit_existing_driver_without_change() {
        let text = prompt_text(|_field, _id, initial| Ok(initial.unwrap()));

        let mut config = Config::from_iter([drv1("key")]);
        let modified = edit(text, Kind::Driver, &mut config, PartialIdNav::new("drv1")).unwrap();

        assert_eq!(modified, Modification::Unchanged);
        assert_eq!(config, Config::from_iter([drv1("key")]));
    }

    #[test]
    fn test_edit_existing_driver_with_provided_values() {
        let partial = PartialIdNav::new("drv1")
            .with_name("new name")
            .with_email("new email")
            .with_key("new key");

        let mut text = AssertPromptText::start("drv1")
            .expect(Field::Name)
            .with_initial_value(partial.name.as_deref().unwrap())
            .returns(Initial)
            .expect(Field::Email)
            .with_initial_value(partial.email.as_deref().unwrap())
            .returns(Initial)
            .expect(Field::Key)
            .with_initial_value(partial.key.as_deref().unwrap())
            .returns(Initial)
            .done();

        let mut config = Config::from_iter([drv1("key")]);
        let modified = edit(text.as_ui(), Kind::Driver, &mut config, partial.clone()).unwrap();

        text.expect_done();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(
            config,
            Config::from_iter([Driver {
                navigator: Navigator {
                    alias: Id::from("drv1"),
                    name: partial.name.unwrap(),
                    email: partial.email.unwrap()
                },
                key: partial.key
            }])
        );
    }
}
