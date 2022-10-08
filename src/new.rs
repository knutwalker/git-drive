use crate::{
    config::Config,
    data::{Kind, Modification, PartialNav},
    ui::{self, PromptAlias, PromptText},
};
use eyre::Result;

pub fn run(
    ui: impl PromptAlias + PromptText,
    kind: Kind,
    config: &mut Config,
    partial: PartialNav,
) -> Result<Modification> {
    match kind {
        Kind::Navigator => {
            let navigator = ui::complete_new_nav(ui, partial, config)?;
            config.navigators.push(navigator);
        }
        Kind::Driver => {
            let driver = ui::complete_new_drv(ui, partial, config)?;
            config.drivers.push(driver);
        }
    }
    Ok(Modification::Changed)
}
