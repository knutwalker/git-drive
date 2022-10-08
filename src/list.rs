use crate::{
    config::Config,
    data::{Kind, Modification, Navigator},
};

pub fn run(kind: Kind, config: &Config) -> Modification {
    for line in list(kind, config) {
        println!("{}", line);
    }
    Modification::Unchanged
}

fn list(kind: Kind, config: &Config) -> Box<dyn Iterator<Item = String> + '_> {
    match kind {
        Kind::Navigator => Box::new(config.navigators.iter().map(format_nav)),
        Kind::Driver => Box::new(
            config
                .drivers
                .iter()
                .map(|drv| &drv.navigator)
                .map(format_nav),
        ),
    }
}

fn format_nav(nav: &Navigator) -> String {
    format!("{}: {} <{}>", &*nav.alias, nav.name, nav.email)
}

#[cfg(test)]
mod tests {
    use crate::data::tests::{drv1, nav1, nav2};

    use super::*;

    #[test]
    fn list_navigators() {
        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let lines = list(Kind::Navigator, &config).collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec!["nav1: bernd <foo@bar.org>", "nav2: ronny <baz@bar.org>"]
        );
    }

    #[test]
    fn list_drivers() {
        let config = Config::from_iter([nav1().ent(), nav2().ent(), drv1(None).ent()]);
        let lines = list(Kind::Driver, &config).collect::<Vec<_>>();
        assert_eq!(lines, vec!["drv1: ralle <qux@bar.org>"]);
    }
}
