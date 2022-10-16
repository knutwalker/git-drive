use super::{CheckMode, Seat};
use crate::{config::Config, data::Field};
use console::style;
use eyre::{bail, eyre, Result};
use std::{cell::Cell, marker::PhantomData};

pub trait Validator {
    fn validate(&mut self, input: &str) -> Result<()>;

    fn and_then<T: Validator>(self, other: T) -> AndThen<Self, T>
    where
        Self: Sized,
    {
        AndThen(self, other)
    }
}

impl<T> Validator for &mut T
where
    T: Validator,
{
    fn validate(&mut self, input: &str) -> Result<()> {
        T::validate(self, input)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NoOp;

impl Validator for NoOp {
    fn validate(&mut self, _input: &str) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CheckForEmpty {
    messages: Cell<[MsgTemplate; 2]>,
    field: Field,
    allow_empty: bool,
}

impl CheckForEmpty {
    pub const fn new(field: Field) -> Self {
        Self {
            messages: Cell::new([MsgTemplate::MustNotBeEmpty, MsgTemplate::EnterANonEmptyName]),
            field,
            allow_empty: false,
        }
    }

    pub const fn with_allow_empty(self, allow_empty: bool) -> Self {
        Self {
            allow_empty,
            ..self
        }
    }
}

impl Validator for CheckForEmpty {
    fn validate(&mut self, input: &str) -> Result<()> {
        if self.allow_empty || !input.trim().is_empty() {
            return Ok(());
        }

        let mut messages = self.messages.get();
        let tpl = messages[0];
        messages.rotate_left(1);
        self.messages.set(messages);

        let err = match tpl {
            MsgTemplate::MustNotBeEmpty => {
                eyre!("The {} must not be empty.", self.field)
            }
            MsgTemplate::EnterANonEmptyName => {
                eyre!("Please enter a non-empty {}.", self.field)
            }
        };
        Err(err)
    }
}

#[derive(Copy, Clone, Debug)]
enum MsgTemplate {
    MustNotBeEmpty,
    EnterANonEmptyName,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AndThen<A, B>(A, B);

impl<A, B> Validator for AndThen<A, B>
where
    A: Validator,
    B: Validator,
{
    fn validate(&mut self, input: &str) -> Result<()> {
        self.0.validate(input)?;
        self.1.validate(input)?;
        Ok(())
    }
}

impl<A, B> AndThen<A, B> {
    pub fn into_inner(self) -> (A, B) {
        (self.0, self.1)
    }
}

#[derive(Copy, Clone)]
pub(super) struct Lookup<'config, T> {
    config: &'config Config,
    check: CheckMode,
    _kind: PhantomData<T>,
}

impl<'config, T: Seat> Lookup<'config, T> {
    pub fn new(config: &'config Config, check: CheckMode) -> Self {
        Self {
            config,
            check,
            _kind: PhantomData,
        }
    }

    pub fn matching_navigator(&self, id: &str) -> Option<&'config T::Entity> {
        T::find(self.config, id)
    }
}

impl<T: Seat> Validator for Lookup<'_, T> {
    fn validate(&mut self, input: &str) -> Result<()> {
        let id_exists = self.matching_navigator(input).is_some();
        match self.check {
            CheckMode::MustExist if !id_exists => {
                bail!("Alias {} does not exist.", style(input).cyan());
            }
            CheckMode::MustNotExist if id_exists => {
                bail!("Alias {} already exists.", style(input).cyan());
            }
            CheckMode::MustExist | CheckMode::MustNotExist => {}
        }

        Ok(())
    }
}
