use nanoserde::{DeJson, SerJson};
use std::ops::Deref;

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
pub struct Provided(pub Option<Vec<Id>>);

#[derive(Debug)]
pub struct New {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug)]
pub struct Command {
    pub kind: Kind,
    pub action: Action,
}

impl Command {
    pub fn new(kind: Kind, action: Action) -> Self {
        Self { kind, action }
    }

    pub fn nav(action: Action) -> Self {
        Self::new(Kind::Navigator, action)
    }

    pub fn drv(action: Action) -> Self {
        Self::new(Kind::Driver, action)
    }
}

#[derive(Debug)]
pub enum Action {
    Drive(Provided),
    Change(Provided),
    Show(String),
    List,
    New(New),
    Edit(New),
    Delete(Provided),
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

impl Into<FlatDriver> for &Driver {
    fn into(self) -> FlatDriver {
        FlatDriver {
            alias: self.navigator.alias.clone(),
            name: self.navigator.name.clone(),
            email: self.navigator.email.clone(),
            key: self.key.clone(),
        }
    }
}

impl Into<Driver> for FlatDriver {
    fn into(self) -> Driver {
        Driver {
            navigator: Navigator {
                alias: self.alias,
                name: self.name,
                email: self.email,
            },
            key: self.key,
        }
    }
}
