//! Immutable content definitions, constructed only by successful validation.
use std::collections::BTreeMap;
use worldwright_core::{command::PlayerText, ids::DefinitionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRoom {
    pub(crate) title: PlayerText,
    pub(crate) description: PlayerText,
    pub(crate) exits: BTreeMap<String, DefinitionId>,
}
impl ValidatedRoom {
    pub fn title(&self) -> &PlayerText {
        &self.title
    }
    pub fn description(&self) -> &PlayerText {
        &self.description
    }
    pub fn exits(&self) -> &BTreeMap<String, DefinitionId> {
        &self.exits
    }
}

/// Cannot be created through public fields or raw JSON deserialization.
/// ```compile_fail
/// use worldwright_content::ValidatedWorldContent;
/// let content = ValidatedWorldContent { namespace: String::new() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWorldContent {
    pub(crate) namespace: String,
    pub(crate) entry: DefinitionId,
    pub(crate) rooms: BTreeMap<DefinitionId, ValidatedRoom>,
}
impl ValidatedWorldContent {
    pub fn schema_version(&self) -> u64 {
        1
    }
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn entry_room(&self) -> &DefinitionId {
        &self.entry
    }
    pub fn rooms(&self) -> &BTreeMap<DefinitionId, ValidatedRoom> {
        &self.rooms
    }
    pub fn exit_count(&self) -> usize {
        self.rooms.values().map(|room| room.exits.len()).sum()
    }
}
