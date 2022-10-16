use std::{borrow::Borrow, fmt, ops::Deref};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigator {
    pub alias: Id,
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Driver {
    pub navigator: Navigator,
    pub key: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Kind {
    Navigator,
    Driver,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Field {
    Alias,
    Name,
    Email,
    Key,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias => f.write_str("alias"),
            Self::Name => f.write_str("name"),
            Self::Email => f.write_str("email"),
            Self::Key => f.write_str("signing key"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialNav {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub key: Option<String>,
}

impl PartialNav {
    pub fn with(self, field: Field, value: impl Into<Option<String>>) -> Self {
        match field {
            Field::Alias => self.with_id(value),
            Field::Name => self.with_name(value),
            Field::Email => self.with_email(value),
            Field::Key => self.with_key(value),
        }
    }

    pub fn with_id(self, id: impl Into<Option<String>>) -> Self {
        Self {
            id: id.into(),
            ..self
        }
    }

    pub fn with_name(self, name: impl Into<Option<String>>) -> Self {
        Self {
            name: name.into(),
            ..self
        }
    }

    pub fn with_email(self, email: impl Into<Option<String>>) -> Self {
        Self {
            email: email.into(),
            ..self
        }
    }

    pub fn with_key(self, key: impl Into<Option<String>>) -> Self {
        Self {
            key: key.into(),
            ..self
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialIdNav {
    pub id: Id,
    pub name: Option<String>,
    pub email: Option<String>,
    pub key: Option<String>,
}

impl PartialIdNav {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: Id(id.into()),
            name: None,
            email: None,
            key: None,
        }
    }

    pub fn merge(self, partial: PartialNav) -> Self {
        Self {
            id: self.id,
            name: partial.name.or(self.name),
            email: partial.email.or(self.email),
            key: partial.key.or(self.key),
        }
    }

    #[cfg(test)]
    pub fn with_name<S: Into<String>>(self, name: impl Into<Option<S>>) -> Self {
        Self {
            name: name.into().map(Into::into),
            ..self
        }
    }

    #[cfg(test)]
    pub fn with_email<S: Into<String>>(self, email: impl Into<Option<S>>) -> Self {
        Self {
            email: email.into().map(Into::into),
            ..self
        }
    }

    #[cfg(test)]
    pub fn with_key<S: Into<String>>(self, key: impl Into<Option<S>>) -> Self {
        Self {
            key: key.into().map(Into::into),
            ..self
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShowNav {
    pub color: String,
    pub fail_if_empty: bool,
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

impl<T: Into<String>> From<T> for Id {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Navigator => f.pad("navigator"),
            Self::Driver => f.pad("driver"),
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

impl From<&Id> for String {
    fn from(id: &Id) -> Self {
        id.0.clone()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Modification {
    Changed,
    Unchanged,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn nav1() -> Navigator {
        Navigator {
            alias: Id::from("nav1"),
            name: String::from("bernd"),
            email: String::from("foo@bar.org"),
        }
    }

    pub fn nav2() -> Navigator {
        Navigator {
            alias: Id::from("nav2"),
            name: String::from("ronny"),
            email: String::from("baz@bar.org"),
        }
    }

    pub fn drv1(key: impl Into<Option<&'static str>>) -> Driver {
        Driver {
            navigator: Navigator {
                alias: Id::from("drv1"),
                name: String::from("ralle"),
                email: String::from("qux@bar.org"),
            },
            key: key.into().map(String::from),
        }
    }
}
