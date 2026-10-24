use crate::instances::{Collection, InstanceRecord};

/// What the library page shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub instances: Vec<InstanceRecord>,
    pub collections: Vec<Collection>,
}
