use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($name:ident) => {
        #[cfg_attr(test, derive(ts_rs::TS))]
        #[derive(
            Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

string_id!(JobId);
string_id!(ItemId);
string_id!(WorkerSlotId);
string_id!(CorrelationId);
string_id!(DiagnosticId);

/// Milliseconds since the Unix epoch. The producer owns clock accuracy.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct TimestampMs(pub u64);

/// A globally monotonic JobManager observation revision.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct Revision(pub u64);

impl Revision {
    pub const INITIAL: Self = Self(0);

    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    pub fn relation_to(self, incoming: Self) -> RevisionRelation {
        if incoming <= self {
            RevisionRelation::Stale
        } else if self.next() == Some(incoming) {
            RevisionRelation::Next
        } else {
            RevisionRelation::Gap
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionRelation {
    Stale,
    Next,
    Gap,
}

#[cfg(test)]
mod tests {
    use super::{Revision, RevisionRelation};

    #[test]
    fn classifies_revision_order() {
        let current = Revision(4);
        assert_eq!(current.relation_to(Revision(3)), RevisionRelation::Stale);
        assert_eq!(current.relation_to(Revision(4)), RevisionRelation::Stale);
        assert_eq!(current.relation_to(Revision(5)), RevisionRelation::Next);
        assert_eq!(current.relation_to(Revision(7)), RevisionRelation::Gap);
    }
}
