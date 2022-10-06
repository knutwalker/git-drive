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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartialNav {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub key: Option<String>,
}

impl PartialNav {
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
