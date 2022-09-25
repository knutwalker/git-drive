#![allow(clippy::question_mark)]

use nanoserde::{DeJson, SerJson};
use std::{borrow::Borrow, fmt, ops::Deref};

#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[repr(transparent)]
pub struct Id(pub String);

impl Id {
    pub fn same_as_nav(&self, other: &Navigator) -> bool {
        self == &other.alias
    }

    pub fn same_as_drv(&self, other: &Driver) -> bool {
        self == &other.navigator.alias
    }
}

#[derive(Debug, Default, DeJson, SerJson)]
pub struct Navigator {
    pub alias: Id,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Default)]
pub struct Driver {
    pub navigator: Navigator,
    pub key: Option<String>,
}

#[derive(Debug, Copy, Clone)]
pub enum Kind {
    Navigator,
    Driver,
}

#[derive(Debug)]
pub struct PartialNav {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ShowNav {
    pub color: String,
    pub fail_if_empty: bool,
}

#[derive(Debug)]
pub enum Action {
    DriveFromSelection,
    DriveWith(Vec<Id>),
    DriveAlone,
    ListNavigators,
    ListDrivers,
    ShowCurrentNavigator(ShowNav),
    NewNavigator(PartialNav),
    EditNavigator(PartialNav),
    DeleteNavigatorFromSelection,
    DeleteNavigators(Vec<Id>),

    DriveAsFromSelection,
    DriveAs(Id),
    NewDriver(PartialNav),
    EditDriver(PartialNav),
    DeleteDriverFromSelection,
    DeleteDrivers(Vec<Id>),
}

impl Deref for Id {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Id {
    fn from(v: &str) -> Self {
        Id(v.to_string())
    }
}

impl DeJson for Id {
    fn de_json(
        s: &mut nanoserde::DeJsonState,
        i: &mut std::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        Ok(Id(DeJson::de_json(s, i)?))
    }
}
impl SerJson for Id {
    fn ser_json(&self, d: usize, s: &mut nanoserde::SerJsonState) {
        self.0.ser_json(d, s);
    }
}

impl DeJson for Driver {
    fn de_json(
        s: &mut nanoserde::DeJsonState,
        i: &mut std::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        let flat: FlatDriver = DeJson::de_json(s, i)?;
        Ok(flat.into())
    }
}
impl SerJson for Driver {
    fn ser_json(&self, d: usize, s: &mut nanoserde::SerJsonState) {
        let flat: FlatDriver = Into::into(self);
        flat.ser_json(d, s);
    }
}
#[derive(DeJson, SerJson)]
struct FlatDriver {
    alias: Id,
    name: String,
    email: String,
    key: Option<String>,
}

impl From<&Driver> for FlatDriver {
    fn from(val: &Driver) -> Self {
        FlatDriver {
            alias: val.navigator.alias.clone(),
            name: val.navigator.name.clone(),
            email: val.navigator.email.clone(),
            key: val.key.clone(),
        }
    }
}

impl From<FlatDriver> for Driver {
    fn from(val: FlatDriver) -> Self {
        Driver {
            navigator: Navigator {
                alias: val.alias,
                name: val.name,
                email: val.email,
            },
            key: val.key,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Navigator => f.pad("navigator"),
            Kind::Driver => f.pad("driver"),
        }
    }
}

pub trait IdRef {
    fn id(&self) -> &Id;
}

impl IdRef for Id {
    fn id(&self) -> &Id {
        self
    }
}

impl IdRef for &Id {
    fn id(&self) -> &Id {
        self
    }
}

impl IdRef for Navigator {
    fn id(&self) -> &Id {
        &self.alias
    }
}

impl IdRef for Driver {
    fn id(&self) -> &Id {
        &self.navigator.alias
    }
}

impl Borrow<str> for &Navigator {
    fn borrow(&self) -> &str {
        &self.alias
    }
}
