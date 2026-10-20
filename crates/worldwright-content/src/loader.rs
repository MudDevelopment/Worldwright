//! Bounded local input and duplicate-preserving validation before typed parsing.
use crate::{ContentReport, ValidatedWorldContent, schema, validation};
use serde::{
    Deserializer,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use std::{collections::BTreeSet, fmt, fs::File, io::Read, path::Path};

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_DEPTH: usize = 32;
pub const MAX_DIAGNOSTICS: usize = 100;
const MAX_VALUES: usize = 200_000;
const MAX_CONTAINER_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub struct ContentLimits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_diagnostics: usize,
}
impl Default for ContentLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_INPUT_BYTES,
            max_depth: MAX_DEPTH,
            max_diagnostics: MAX_DIAGNOSTICS,
        }
    }
}
impl ContentLimits {
    fn check(self, source: &str) -> Result<(), ContentReport> {
        if !(1..=MAX_INPUT_BYTES).contains(&self.max_bytes)
            || !(1..=MAX_DEPTH).contains(&self.max_depth)
            || !(1..=MAX_DIAGNOSTICS).contains(&self.max_diagnostics)
        {
            return Err(ContentReport::single(
                source,
                "WWC_LIMIT",
                "",
                "limits must be positive and cannot exceed library defaults",
            ));
        }
        Ok(())
    }
}

pub fn load_file(
    path: impl AsRef<Path>,
    limits: ContentLimits,
) -> Result<ValidatedWorldContent, ContentReport> {
    let path = path.as_ref();
    let source = path.to_string_lossy();
    limits.check(&source)?;
    let io_error =
        |error: std::io::Error| ContentReport::single(&source, "WWC_IO", "", error.to_string());
    // Check before opening to avoid opening known devices/FIFOs. Recheck the
    // opened handle; trusted local path selection is still required (TOCTOU).
    let metadata = std::fs::metadata(path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ContentReport::single(
            &source,
            "WWC_IO",
            "",
            "expected a regular file",
        ));
    }
    let file = File::open(path).map_err(io_error)?;
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(ContentReport::single(
            &source,
            "WWC_IO",
            "",
            "expected a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take((limits.max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    load_bytes(&source, &bytes, limits)
}

pub fn load_bytes(
    source: &str,
    bytes: &[u8],
    limits: ContentLimits,
) -> Result<ValidatedWorldContent, ContentReport> {
    limits.check(source)?;
    if bytes.len() > limits.max_bytes {
        return Err(ContentReport::single(
            source,
            "WWC_LIMIT",
            "",
            "input exceeds byte limit",
        ));
    }
    let mut budget = MAX_VALUES;
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    Guard {
        depth: 0,
        max_depth: limits.max_depth,
        budget: &mut budget,
    }
    .deserialize(&mut parser)
    .and_then(|()| parser.end())
    .map_err(|e| parse_report(source, "WWC_JSON", e))?;
    // Version dispatch precedes v1 shape validation; future documents need not
    // satisfy current required fields to receive the correct version error.
    let schema::Object(header): schema::Object<schema::VersionHeader> =
        serde_json::from_slice(bytes).map_err(|e| parse_report(source, "WWC_VERSION", e))?;
    if header.schema_version != Some(1) {
        return Err(ContentReport::single(
            source,
            "WWC_VERSION",
            "/schema_version",
            "supported schema_version is 1; declare it explicitly after reviewing docs/content/SCHEMA_V1.md",
        ));
    }
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let schema::Object(document): schema::Object<schema::Document> =
        serde_path_to_error::deserialize(&mut parser).map_err(|error| {
            let location = format!(
                "{} (line {}, column {})",
                error.path(),
                error.inner().line(),
                error.inner().column()
            );
            let message = error.inner().to_string();
            ContentReport::single(
                source,
                parse_code(&message, "WWC_SCHEMA"),
                location,
                message,
            )
        })?;
    validation::validate(source, document, limits.max_diagnostics)
}
fn parse_code(message: &str, default: &'static str) -> &'static str {
    if message.starts_with("WWC_LIMIT:") {
        "WWC_LIMIT"
    } else if message.starts_with("WWC_DUPLICATE_KEY:") {
        "WWC_DUPLICATE_KEY"
    } else {
        default
    }
}
fn parse_report(source: &str, default: &'static str, error: serde_json::Error) -> ContentReport {
    let message = error.to_string();
    ContentReport::single(
        source,
        parse_code(&message, default),
        format!("line {}, column {}", error.line(), error.column()),
        message,
    )
}

// A streaming structural pass: no Value tree is created. Each object retains
// only its keys, and duplicate decoded keys are rejected before map conversion.
struct Guard<'a> {
    depth: usize,
    max_depth: usize,
    budget: &'a mut usize,
}
impl<'de> DeserializeSeed<'de> for Guard<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if *self.budget == 0 {
            return Err(de::Error::custom("WWC_LIMIT: too many JSON values"));
        }
        *self.budget -= 1;
        deserializer.deserialize_any(self)
    }
}
impl Guard<'_> {
    fn container<E: de::Error>(&self) -> Result<(), E> {
        if self.depth >= self.max_depth {
            return Err(E::custom("WWC_LIMIT: nesting too deep"));
        }
        Ok(())
    }
}
impl<'de> Visitor<'de> for Guard<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON without duplicate keys")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.container()?;
        let mut count = 0;
        while seq
            .next_element_seed(Guard {
                depth: self.depth + 1,
                max_depth: self.max_depth,
                budget: self.budget,
            })?
            .is_some()
        {
            count += 1;
            if count > MAX_CONTAINER_ENTRIES {
                return Err(de::Error::custom("WWC_LIMIT: too many array entries"));
            }
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        self.container()?;
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.len() == MAX_CONTAINER_ENTRIES {
                return Err(de::Error::custom("WWC_LIMIT: too many object entries"));
            }
            if !keys.insert(key) {
                return Err(de::Error::custom("WWC_DUPLICATE_KEY: repeated object key"));
            }
            map.next_value_seed(Guard {
                depth: self.depth + 1,
                max_depth: self.max_depth,
                budget: self.budget,
            })?;
        }
        Ok(())
    }
}
