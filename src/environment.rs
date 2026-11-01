//! Environments: the chained scopes that hold Gloss variables.
//!
//! Each block creates a child environment whose `parent` points at the
//! enclosing scope. Looking a variable up walks the chain outwards;
//! `let` always defines in the current scope; assignment walks the
//! chain and updates the scope that already holds the name.
//!
//! Environments live behind `Rc<RefCell<...>>` so a function value can
//! keep its defining environment alive (a closure) after that scope has
//! finished executing.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

/// One scope: a set of names plus an optional link to the parent scope.
#[derive(Debug, PartialEq)]
pub struct Environment {
    values: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    /// Creates the global (outermost) environment.
    pub fn new_global() -> Rc<RefCell<Environment>> {
        Rc::new(RefCell::new(Environment {
            values: HashMap::new(),
            parent: None,
        }))
    }

    /// Creates a child environment enclosed by `parent`.
    pub fn new_child(parent: Rc<RefCell<Environment>>) -> Rc<RefCell<Environment>> {
        Rc::new(RefCell::new(Environment {
            values: HashMap::new(),
            parent: Some(parent),
        }))
    }

    /// Defines (or redefines) a name in this scope only.
    pub fn define(&mut self, name: &str, value: Value) {
        self.values.insert(name.to_string(), value);
    }

    /// Looks a name up in this scope, then in each enclosing scope.
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(value) = self.values.get(name) {
            return Some(value.clone());
        }
        match &self.parent {
            Some(parent) => parent.borrow().get(name),
            None => None,
        }
    }

    /// Assigns to an existing name in the nearest scope that holds it.
    /// Returns `false` when no scope holds the name.
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            return true;
        }
        match &self.parent {
            Some(parent) => parent.borrow_mut().assign(name, value),
            None => false,
        }
    }
}
