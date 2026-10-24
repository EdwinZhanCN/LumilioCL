use super::{Collection, InstanceStore, StoreError};

impl InstanceStore {
    pub fn create_collection(&mut self, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::InvalidName);
        }
        if self.index.collections.iter().any(|c| c.name == name) {
            return Err(StoreError::DuplicateCollection(name.to_owned()));
        }
        let mut next = self.index.clone();
        next.collections.push(Collection {
            name: name.to_owned(),
            members: Vec::new(),
        });
        self.save(next)
    }

    pub fn delete_collection(&mut self, name: &str) -> Result<(), StoreError> {
        let mut next = self.index.clone();
        let before = next.collections.len();
        next.collections.retain(|c| c.name != name);
        if next.collections.len() == before {
            return Err(StoreError::UnknownCollection(name.to_owned()));
        }
        self.save(next)
    }

    /// Gives a collection another name; its games stay in it, in their place.
    pub fn rename_collection(&mut self, from: &str, to: &str) -> Result<(), StoreError> {
        let to = to.trim();
        if to.is_empty() {
            return Err(StoreError::InvalidName);
        }
        let mut next = self.index.clone();
        if from != to && next.collections.iter().any(|c| c.name == to) {
            return Err(StoreError::DuplicateCollection(to.to_owned()));
        }
        let target = next
            .collections
            .iter_mut()
            .find(|c| c.name == from)
            .ok_or_else(|| StoreError::UnknownCollection(from.to_owned()))?;
        target.name = to.to_owned();
        self.save(next)
    }

    /// Puts an instance in exactly these collections and in no other, in one
    /// commit. A name that is not a collection changes nothing.
    pub fn set_collections_of(
        &mut self,
        instance: &str,
        wanted: &[String],
    ) -> Result<(), StoreError> {
        if self.get(instance).is_none() {
            return Err(StoreError::UnknownInstance(instance.to_owned()));
        }
        if let Some(unknown) = wanted
            .iter()
            .find(|name| !self.index.collections.iter().any(|c| &c.name == *name))
        {
            return Err(StoreError::UnknownCollection(unknown.clone()));
        }
        let mut next = self.index.clone();
        for collection in &mut next.collections {
            let member = wanted.contains(&collection.name);
            let present = collection.members.iter().any(|id| id == instance);
            match (member, present) {
                (true, false) => collection.members.push(instance.to_owned()),
                (false, true) => collection.members.retain(|id| id != instance),
                _ => {}
            }
        }
        self.save(next)
    }

    /// Adds or removes an instance from a collection; adding twice is a no-op.
    pub fn set_membership(
        &mut self,
        collection: &str,
        instance: &str,
        member: bool,
    ) -> Result<(), StoreError> {
        if self.get(instance).is_none() {
            return Err(StoreError::UnknownInstance(instance.to_owned()));
        }
        let mut next = self.index.clone();
        let target = next
            .collections
            .iter_mut()
            .find(|c| c.name == collection)
            .ok_or_else(|| StoreError::UnknownCollection(collection.to_owned()))?;
        target.members.retain(|id| id != instance);
        if member {
            target.members.push(instance.to_owned());
        }
        self.save(next)
    }
}
