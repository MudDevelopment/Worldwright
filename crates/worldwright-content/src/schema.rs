//! Private v1 wire types. No public constructor can bypass semantic validation.
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{collections::BTreeMap, fmt, marker::PhantomData};
use worldwright_core::runtime::{MAX_EXITS_PER_ROOM, MAX_ROOMS};

// Serde-derived structs also accept positional sequences. The published JSON
// contract requires objects, so force map dispatch at every record boundary.
pub(crate) struct Object<T>(pub T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Record<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Record<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        d.deserialize_map(Record(PhantomData))
    }
}

#[derive(Deserialize)]
pub(crate) struct VersionHeader {
    pub schema_version: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    pub schema_version: u64,
    pub namespace: String,
    pub entry_room: String,
    #[serde(deserialize_with = "rooms")]
    pub rooms: Vec<Room>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Room {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(deserialize_with = "exits")]
    pub exits: BTreeMap<String, String>,
    #[serde(default)]
    pub allow_unreachable: bool,
}
fn rooms<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Room>, D::Error> {
    struct Rooms(PhantomData<Room>);
    impl<'de> Visitor<'de> for Rooms {
        type Value = Vec<Room>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most 1024 rooms")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(Object(value)) = seq.next_element::<Object<Room>>()? {
                if values.len() == MAX_ROOMS {
                    return Err(de::Error::custom("WWC_LIMIT: too many rooms"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Rooms(PhantomData))
}
fn exits<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    struct Exits;
    impl<'de> Visitor<'de> for Exits {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most 64 named exits")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                if values.len() == MAX_EXITS_PER_ROOM {
                    return Err(de::Error::custom("WWC_LIMIT: too many exits"));
                }
                if values.contains_key(&key) {
                    return Err(de::Error::custom("WWC_DUPLICATE_KEY: repeated exit label"));
                }
                values.insert(key, map.next_value()?);
            }
            Ok(values)
        }
    }
    d.deserialize_map(Exits)
}
