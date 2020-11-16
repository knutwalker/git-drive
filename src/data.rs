use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    pub fn same_as(&self, other: &impl Deref<Target = Navigator>) -> bool {
        self.as_ref() == Deref::deref(other).alias.as_ref()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Navigator {
    pub alias: Id,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Driver {
    #[serde(flatten)]
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

impl Deref for Driver {
    type Target = Navigator;

    fn deref(&self) -> &Self::Target {
        &self.navigator
    }
}

impl Deref for Navigator {
    type Target = Navigator;

    fn deref(&self) -> &Self::Target {
        self
    }
}
