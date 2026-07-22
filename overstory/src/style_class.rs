// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Authoring-time style class references.

use alloc::string::String;

use understory_style::{ClassId, StyleVocabulary};

/// A style class supplied to an element or template spec.
///
/// Existing low-level code can pass a resolved [`ClassId`]. App and parser
/// authoring code can pass a class name such as `.demo-card`; Overstory resolves
/// the name through the owning [`Ui`](crate::Ui)'s [`StyleVocabulary`] when the
/// spec is appended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleClass {
    /// An already-resolved class id.
    Id(ClassId),
    /// An author-facing class name that should be interned in the UI vocabulary.
    Name(String),
}

impl StyleClass {
    /// Creates a class reference from an already-resolved id.
    #[must_use]
    pub const fn id(id: ClassId) -> Self {
        Self::Id(id)
    }

    /// Creates a class reference from an author-facing name.
    pub fn name(name: impl Into<String>) -> Self {
        Self::Name(name.into())
    }

    /// Returns this class's id if it is already present in `vocabulary`.
    ///
    /// This does not intern missing names. It is useful for read-only queries
    /// such as removing a class by name or searching an existing tree.
    #[must_use]
    pub fn registered_id(self, vocabulary: &StyleVocabulary) -> Option<ClassId> {
        match self {
            Self::Id(id) => Some(id),
            Self::Name(name) => vocabulary.class_id_by_name(&name),
        }
    }

    pub(crate) fn resolve_existing(self, vocabulary: &StyleVocabulary) -> Option<ClassId> {
        self.registered_id(vocabulary)
    }
}

impl From<ClassId> for StyleClass {
    fn from(value: ClassId) -> Self {
        Self::Id(value)
    }
}

impl From<String> for StyleClass {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<&str> for StyleClass {
    fn from(value: &str) -> Self {
        Self::Name(String::from(value))
    }
}
