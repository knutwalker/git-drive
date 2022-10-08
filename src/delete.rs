use crate::{
    config::Config,
    data::{IdRef, Kind, Modification},
    ui::{self, SelectMany},
};
use eyre::Result;

pub fn select(ui: impl SelectMany, kind: Kind, config: &mut Config) -> Result<Modification> {
    let ids = ui::select_ids_from(ui, kind, config, &[])?;
    let ids = ids.into_iter().cloned().collect::<Vec<_>>();
    Ok(run(kind, config, &ids))
}

pub fn run<I: IdRef>(kind: Kind, config: &mut Config, ids: &[I]) -> Modification {
    match kind {
        Kind::Navigator => do_delete(&mut config.navigators, ids),
        Kind::Driver => do_delete(&mut config.drivers, ids),
    }
}

fn do_delete<T: IdRef, I: IdRef>(data: &mut Vec<T>, ids: &[I]) -> Modification {
    let mut changed = Modification::Unchanged;
    let mut i = 0;
    while i != data.len() {
        if ids.iter().any(|id| id.id() == data[i].id()) {
            drop(data.remove(i));
            changed = Modification::Changed;
        } else {
            i += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data::{
            tests::{drv1, nav1, nav2},
            Id,
        },
        ui::{
            util::{select_many, NoUi},
            Selectable,
        },
    };

    #[test]
    fn test_delete_existing() {
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = run(Kind::Navigator, &mut config, &[Id::from("nav1")]);
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[nav2()]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_delete_non_existing() {
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = run(Kind::Navigator, &mut config, &[Id::from("non existing")]);
        assert_eq!(modified, Modification::Unchanged);
        assert_eq!(&config.navigators, &[nav1(), nav2()]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_delete_from_empty_config() {
        let mut config = Config::default();
        let modified = select(NoUi, Kind::Navigator, &mut config).unwrap();
        assert_eq!(modified, Modification::Unchanged);
        assert_eq!(&config.navigators, &[]);
        assert_eq!(&config.drivers, &[]);
    }

    #[test]
    fn test_delete_from_empty_selection() {
        let ui = select_many(|kind, items| {
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
            Ok(Vec::new())
        });
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = select(ui, Kind::Navigator, &mut config).unwrap();
        assert_eq!(modified, Modification::Unchanged);
        assert_eq!(&config.navigators, &[nav1(), nav2()]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_delete_from_single_selection() {
        let ui = select_many(|kind, items| {
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
            Ok(vec![0])
        });
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = select(ui, Kind::Navigator, &mut config).unwrap();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[nav2()]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_delete_driver_from_single_selection() {
        let ui = select_many(|kind, items| {
            assert_eq!(kind, Kind::Driver);
            assert_eq!(
                items,
                &[Selectable {
                    item: "drv1",
                    checked: false
                },]
            );
            Ok(vec![0])
        });
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = select(ui, Kind::Driver, &mut config).unwrap();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[nav1(), nav2()]);
        assert_eq!(&config.drivers, &[]);
    }

    #[test]
    fn test_delete_from_multiple_selection() {
        let ui = select_many(|kind, items| {
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
            Ok(vec![0, 1])
        });
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = select(ui, Kind::Navigator, &mut config).unwrap();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }

    #[test]
    fn test_delete_from_multiple_out_of_order_selection() {
        let ui = select_many(|kind, items| {
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
            Ok(vec![1, 0])
        });
        let mut config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let modified = select(ui, Kind::Navigator, &mut config).unwrap();
        assert_eq!(modified, Modification::Changed);
        assert_eq!(&config.navigators, &[]);
        assert_eq!(&config.drivers, &[drv1(None)]);
    }
}
