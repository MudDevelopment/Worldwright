use crate::{
    ContentReport,
    diagnostic::pointer_token,
    schema::Document,
    validated::{ValidatedRoom, ValidatedWorldContent},
};
use std::collections::{BTreeMap, BTreeSet};
use worldwright_core::{
    command::PlayerText,
    ids::DefinitionId,
    runtime::{MAX_EXITS_PER_ROOM, MAX_ROOMS},
};

fn segment(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}
fn definition(
    value: &str,
    namespace: &str,
    path: &str,
    report: &mut ContentReport,
) -> Option<DefinitionId> {
    match DefinitionId::parse(value) {
        Ok(id) => {
            if value.split_once(':').map(|v| v.0) != Some(namespace) {
                report.push(
                    "WWC_NAMESPACE",
                    path,
                    "definition must use the document namespace",
                );
            }
            Some(id)
        }
        Err(error) => {
            report.push("WWC_ID", path, error.to_string());
            None
        }
    }
}
pub(crate) fn validate(
    source: &str,
    document: Document,
    cap: usize,
) -> Result<ValidatedWorldContent, ContentReport> {
    let mut report = ContentReport::new(source, cap);
    if document.schema_version != 1 {
        report.push(
            "WWC_VERSION",
            "/schema_version",
            "supported schema_version is 1",
        );
    }
    if !segment(&document.namespace) || document.namespace.len() > DefinitionId::MAX_BYTES {
        report.push(
            "WWC_NAMESPACE",
            "/namespace",
            "expected a bounded lowercase namespace segment",
        );
    }
    if document.rooms.is_empty() || document.rooms.len() > MAX_ROOMS {
        report.push("WWC_LIMIT", "/rooms", "expected 1..1024 rooms");
    }
    let entry = definition(
        &document.entry_room,
        &document.namespace,
        "/entry_room",
        &mut report,
    );
    let mut seen = BTreeSet::new();
    let mut room_ids = Vec::new();
    let mut rooms = BTreeMap::new();
    // First collect definitions; references may point forward in the file.
    for (index, room) in document.rooms.iter().enumerate() {
        let base = format!("/rooms/{index}");
        let id = definition(
            &room.id,
            &document.namespace,
            &format!("{base}/id"),
            &mut report,
        );
        if let Some(id) = &id {
            if !seen.insert(id.clone()) {
                report.push(
                    "WWC_DUPLICATE_ROOM",
                    format!("{base}/id"),
                    "room ID is already defined",
                );
            }
        }
        room_ids.push(id.clone());
        let title = PlayerText::new(&room.title)
            .map_err(|e| report.push("WWC_TEXT", format!("{base}/title"), e.to_string()));
        let description = PlayerText::new(&room.description)
            .map_err(|e| report.push("WWC_TEXT", format!("{base}/description"), e.to_string()));
        if let (Some(id), Ok(title), Ok(description)) = (id, title, description) {
            rooms.entry(id).or_insert(ValidatedRoom {
                title,
                description,
                exits: BTreeMap::new(),
            });
        }
    }
    if let Some(entry) = &entry {
        if !seen.contains(entry) {
            report.push(
                "WWC_UNKNOWN_ROOM",
                "/entry_room",
                "entry room is not defined",
            );
        }
    }
    for (index, room) in document.rooms.iter().enumerate() {
        let base = format!("/rooms/{index}/exits");
        if room.exits.len() > MAX_EXITS_PER_ROOM {
            report.push("WWC_LIMIT", &base, "at most 64 exits per room");
        }
        let mut destinations = BTreeSet::new();
        for (label, target) in &room.exits {
            let path = format!("{base}/{}", pointer_token(label));
            if label.len() > 64 || !segment(label) {
                report.push(
                    "WWC_EXIT_LABEL",
                    &path,
                    "exit label must match [a-z][a-z0-9_-]* and be at most 64 bytes",
                );
            }
            let target = definition(target, &document.namespace, &path, &mut report);
            if let Some(target) = target {
                if !seen.contains(&target) {
                    report.push("WWC_UNKNOWN_ROOM", &path, "exit destination is not defined");
                }
                if room_ids[index].as_ref() == Some(&target) {
                    report.push("WWC_SELF_EXIT", &path, "self-exits are not supported");
                }
                if !destinations.insert(target.clone()) {
                    report.push(
                        "WWC_DUPLICATE_EXIT",
                        &path,
                        "multiple labels cannot target the same room",
                    );
                }
                if let Some(record) = room_ids[index].as_ref().and_then(|id| rooms.get_mut(id)) {
                    record.exits.insert(label.clone(), target);
                }
            }
        }
    }
    // Avoid reachability noise when identity/reference prerequisites are invalid.
    if !report.is_empty() {
        return Err(report);
    }
    let Some(entry) = entry else {
        return Err(ContentReport::single(
            source,
            "WWC_ID",
            "/entry_room",
            "invalid entry room",
        ));
    };
    let mut reached = BTreeSet::new();
    let mut pending = vec![entry.clone()];
    while let Some(id) = pending.pop() {
        if reached.insert(id.clone()) {
            if let Some(room) = rooms.get(&id) {
                pending.extend(room.exits.values().cloned());
            }
        }
    }
    for (index, room) in document.rooms.iter().enumerate() {
        if !room.allow_unreachable
            && room_ids[index]
                .as_ref()
                .is_some_and(|id| !reached.contains(id))
        {
            report.push("WWC_UNREACHABLE", format!("/rooms/{index}/allow_unreachable"), "room is unreachable from entry_room; add a path or explicitly set allow_unreachable to true");
        }
    }
    if !report.is_empty() {
        return Err(report);
    }
    Ok(ValidatedWorldContent {
        namespace: document.namespace,
        entry,
        rooms,
    })
}
