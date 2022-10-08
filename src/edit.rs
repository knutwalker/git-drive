use crate::{
    config::Config,
    data::{Kind, Modification, PartialNav},
    ui::{self, PromptAlias, PromptText, SelectOne},
};
use eyre::{bail, Result};

pub fn run(
    mut ui: impl SelectOne + PromptText + PromptAlias,
    kind: Kind,
    config: &mut Config,
    mut new: PartialNav,
) -> Result<Modification> {
    if new.id.is_none() {
        new.id = Some(select_existing_id(&mut ui, kind, config)?);
    }
    edit(ui, kind, config, new)
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
    ui: impl PromptAlias + PromptText,
    kind: Kind,
    config: &mut Config,
    new: PartialNav,
) -> Result<Modification> {
    debug_assert!(new.id.is_some());

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
            Field, Id, Navigator,
        },
        ui::{
            util::{prompt_alias, prompt_text, select_one, NoUi},
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
    fn test_edit_existing_navigator() {
        let text = prompt_text(|field, id, initial| {
            assert_eq!(id, "nav1");

            Ok(match field {
                Field::Name => {
                    assert_eq!(initial.as_deref(), Some("bernd"));
                    String::from("new name")
                }
                Field::Email => {
                    assert_eq!(initial.as_deref(), Some("foo@bar.org"));
                    String::from("new email")
                }
                _ => panic!("Unexpected field: {}", field),
            })
        });

        let alias = prompt_alias(|kind| {
            assert_eq!(kind, Kind::Navigator);
            Ok(String::from("nav1"))
        });

        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = edit(
            (text, alias),
            Kind::Navigator,
            &mut config,
            PartialNav::default().with_id(String::from("nav1")),
        )
        .unwrap();

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

    // #[test]
    // fn test_delete_non_existing() {
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = run(Kind::Navigator, &mut config, &[Id::from("non existing")]);
    //     assert_eq!(modified, Modification::Unchanged);
    //     assert_eq!(&config.navigators, &[nav1(), nav2()]);
    //     assert_eq!(&config.drivers, &[drv1(None)]);
    // }

    // #[test]
    // fn test_delete_from_empty_config() {
    //     let mut config = Config::default();
    //     let modified = select(NoUi, Kind::Navigator, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Unchanged);
    //     assert_eq!(&config.navigators, &[]);
    //     assert_eq!(&config.drivers, &[]);
    // }

    // #[test]
    // fn test_delete_from_empty_selection() {
    //     let ui = select_many(|kind, items| {
    //         assert_eq!(kind, Kind::Navigator);
    //         assert_eq!(
    //             items,
    //             &[
    //                 Selectable {
    //                     item: "nav1",
    //                     checked: false
    //                 },
    //                 Selectable {
    //                     item: "nav2",
    //                     checked: false
    //                 }
    //             ]
    //         );
    //         Ok(Vec::new())
    //     });
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = select(ui, Kind::Navigator, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Unchanged);
    //     assert_eq!(&config.navigators, &[nav1(), nav2()]);
    //     assert_eq!(&config.drivers, &[drv1(None)]);
    // }

    // #[test]
    // fn test_delete_from_single_selection() {
    //     let ui = select_many(|kind, items| {
    //         assert_eq!(kind, Kind::Navigator);
    //         assert_eq!(
    //             items,
    //             &[
    //                 Selectable {
    //                     item: "nav1",
    //                     checked: false
    //                 },
    //                 Selectable {
    //                     item: "nav2",
    //                     checked: false
    //                 }
    //             ]
    //         );
    //         Ok(vec![0])
    //     });
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = select(ui, Kind::Navigator, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Changed);
    //     assert_eq!(&config.navigators, &[nav2()]);
    //     assert_eq!(&config.drivers, &[drv1(None)]);
    // }

    // #[test]
    // fn test_delete_driver_from_single_selection() {
    //     let ui = select_many(|kind, items| {
    //         assert_eq!(kind, Kind::Driver);
    //         assert_eq!(
    //             items,
    //             &[Selectable {
    //                 item: "drv1",
    //                 checked: false
    //             },]
    //         );
    //         Ok(vec![0])
    //     });
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = select(ui, Kind::Driver, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Changed);
    //     assert_eq!(&config.navigators, &[nav1(), nav2()]);
    //     assert_eq!(&config.drivers, &[]);
    // }

    // #[test]
    // fn test_delete_from_multiple_selection() {
    //     let ui = select_many(|kind, items| {
    //         assert_eq!(kind, Kind::Navigator);
    //         assert_eq!(
    //             items,
    //             &[
    //                 Selectable {
    //                     item: "nav1",
    //                     checked: false
    //                 },
    //                 Selectable {
    //                     item: "nav2",
    //                     checked: false
    //                 }
    //             ]
    //         );
    //         Ok(vec![0, 1])
    //     });
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = select(ui, Kind::Navigator, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Changed);
    //     assert_eq!(&config.navigators, &[]);
    //     assert_eq!(&config.drivers, &[drv1(None)]);
    // }

    // #[test]
    // fn test_delete_from_multiple_out_of_order_selection() {
    //     let ui = select_many(|kind, items| {
    //         assert_eq!(kind, Kind::Navigator);
    //         assert_eq!(
    //             items,
    //             &[
    //                 Selectable {
    //                     item: "nav1",
    //                     checked: false
    //                 },
    //                 Selectable {
    //                     item: "nav2",
    //                     checked: false
    //                 }
    //             ]
    //         );
    //         Ok(vec![1, 0])
    //     });
    //     let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
    //     let modified = select(ui, Kind::Navigator, &mut config).unwrap();
    //     assert_eq!(modified, Modification::Changed);
    //     assert_eq!(&config.navigators, &[]);
    //     assert_eq!(&config.drivers, &[drv1(None)]);
    // }
}
