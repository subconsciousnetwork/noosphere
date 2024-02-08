use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{btree_map::Iter as BTreeMapIter, BTreeMap},
    fmt::Debug,
    iter::FlatMap,
    ops::Deref,
};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents a single, flattened capability containing a resource, ability, and caveat.
pub struct Capability {
    pub resource: String,
    pub ability: String,
    pub caveat: Value,
}

impl Capability {
    pub fn new(resource: String, ability: String, caveat: Value) -> Self {
        Capability {
            resource,
            ability,
            caveat,
        }
    }
}

impl From<&Capability> for Capability {
    fn from(value: &Capability) -> Self {
        value.to_owned()
    }
}

impl From<(String, String, Value)> for Capability {
    fn from(value: (String, String, Value)) -> Self {
        Capability::new(value.0, value.1, value.2)
    }
}

impl From<(&str, &str, &Value)> for Capability {
    fn from(value: (&str, &str, &Value)) -> Self {
        Capability::new(value.0.to_owned(), value.1.to_owned(), value.2.to_owned())
    }
}

impl From<Capability> for (String, String, Value) {
    fn from(value: Capability) -> Self {
        (value.resource, value.ability, value.caveat)
    }
}

type MapImpl<K, V> = BTreeMap<K, V>;
type MapIter<'a, K, V> = BTreeMapIter<'a, K, V>;
type AbilitiesImpl = MapImpl<String, Vec<Value>>;
type CapabilitiesImpl = MapImpl<String, AbilitiesImpl>;
type AbilitiesMapClosure<'a> = Box<dyn Fn((&'a String, &'a Vec<Value>)) -> Vec<Capability> + 'a>;
type AbilitiesMap<'a> =
    FlatMap<MapIter<'a, String, Vec<Value>>, Vec<Capability>, AbilitiesMapClosure<'a>>;
type CapabilitiesIterator<'a> = FlatMap<
    MapIter<'a, String, AbilitiesImpl>,
    AbilitiesMap<'a>,
    fn((&'a String, &'a AbilitiesImpl)) -> AbilitiesMap<'a>,
>;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
/// The [Capabilities] struct contains capability data as a map-of-maps, matching the
/// [spec](https://github.com/ucan-wg/spec#326-capabilities--attenuation).
/// See `iter()` to deconstruct this map into a sequence of [Capability] datas.
///
/// ```
/// use noosphere_ucan::capability::Capabilities;
/// use serde_json::json;
///
/// let capabilities = Capabilities::try_from(&json!({
///   "mailto:username@example.com": {
///     "msg/receive": [{}],
///     "msg/send": [{ "draft": true }, { "publish": true, "topic": ["foo"]}]
///   }
/// })).unwrap();
///
/// let resource = capabilities.get("mailto:username@example.com").unwrap();
/// assert_eq!(resource.get("msg/receive").unwrap(), &vec![json!({})]);
/// assert_eq!(resource.get("msg/send").unwrap(), &vec![json!({ "draft": true }), json!({ "publish": true, "topic": ["foo"] })])
/// ```
pub struct Capabilities(CapabilitiesImpl);

impl Capabilities {
    /// Using a [FlatMap] implementation, iterate over a [Capabilities] map-of-map
    /// as a sequence of [Capability] datas.
    ///
    /// ```
    /// use noosphere_ucan::capability::{Capabilities, Capability};
    /// use serde_json::json;
    ///
    /// let capabilities = Capabilities::try_from(&json!({
    ///   "example://example.com/private/84MZ7aqwKn7sNiMGsSbaxsEa6EPnQLoKYbXByxNBrCEr": {
    ///     "wnfs/append": [{}]
    ///   },
    ///   "mailto:username@example.com": {
    ///     "msg/receive": [{}],
    ///     "msg/send": [{ "draft": true }, { "publish": true, "topic": ["foo"]}]
    ///   }
    /// })).unwrap();
    ///
    /// assert_eq!(capabilities.iter().collect::<Vec<Capability>>(), vec![
    ///   Capability::from(("example://example.com/private/84MZ7aqwKn7sNiMGsSbaxsEa6EPnQLoKYbXByxNBrCEr", "wnfs/append", &json!({}))),
    ///   Capability::from(("mailto:username@example.com", "msg/receive", &json!({}))),
    ///   Capability::from(("mailto:username@example.com", "msg/send", &json!({ "draft": true }))),
    ///   Capability::from(("mailto:username@example.com", "msg/send", &json!({ "publish": true, "topic": ["foo"] }))),
    /// ]);
    /// ```
    pub fn iter(&self) -> CapabilitiesIterator {
        self.0
            .iter()
            .flat_map(|(resource, abilities): (&String, &AbilitiesImpl)| {
                abilities
                    .iter()
                    .flat_map(Box::new(
                        |(ability, caveats): (&String, &Vec<Value>)| match caveats.len() {
                            0 => vec![], // An empty caveats list is the same as no capability at all
                            _ => caveats
                                .iter()
                                .map(|caveat| {
                                    Capability::from((
                                        resource.to_owned(),
                                        ability.to_owned(),
                                        caveat.to_owned(),
                                    ))
                                })
                                .collect(),
                        },
                    ))
            })
    }
}

impl Deref for Capabilities {
    type Target = CapabilitiesImpl;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Vec<Capability>> for Capabilities {
    type Error = anyhow::Error;
    fn try_from(value: Vec<Capability>) -> Result<Self, Self::Error> {
        let mut resources: CapabilitiesImpl = BTreeMap::new();
        for capability in value.into_iter() {
            let (resource_name, ability, caveat) = <(String, String, Value)>::from(capability);

            let resource = if let Some(resource) = resources.get_mut(&resource_name) {
                resource
            } else {
                let resource: AbilitiesImpl = BTreeMap::new();
                resources.insert(resource_name.clone(), resource);
                resources.get_mut(&resource_name).unwrap()
            };

            if !caveat.is_object() {
                return Err(anyhow!("Caveat must be an object: {}", caveat));
            }

            if let Some(ability_vec) = resource.get_mut(&ability) {
                ability_vec.push(caveat);
            } else {
                resource.insert(ability, vec![caveat]);
            }
        }
        Capabilities::try_from(resources)
    }
}

impl TryFrom<CapabilitiesImpl> for Capabilities {
    type Error = anyhow::Error;

    fn try_from(value: CapabilitiesImpl) -> Result<Self, Self::Error> {
        for (resource, abilities) in value.iter() {
            if abilities.is_empty() {
                // [0.10.0/3.2.6.2](https://github.com/ucan-wg/spec#3262-abilities):
                // One or more abilities MUST be given for each resource.
                return Err(anyhow!("No abilities given for resource: {}", resource));
            }
        }
        Ok(Capabilities(value))
    }
}

impl TryFrom<&Value> for Capabilities {
    type Error = anyhow::Error;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let map = value
            .as_object()
            .ok_or_else(|| anyhow!("Capabilities must be an object."))?;
        let mut resources: CapabilitiesImpl = BTreeMap::new();

        for (key, value) in map.iter() {
            let resource = key.to_owned();
            let abilities_object = value
                .as_object()
                .ok_or_else(|| anyhow!("Abilities must be an object."))?;

            let abilities = {
                let mut abilities: AbilitiesImpl = BTreeMap::new();
                for (key, value) in abilities_object.iter() {
                    let ability = key.to_owned();
                    let mut caveats: Vec<Value> = vec![];

                    let array = value
                        .as_array()
                        .ok_or_else(|| anyhow!("Caveats must be defined as an array."))?;
                    for value in array.iter() {
                        if !value.is_object() {
                            return Err(anyhow!("Caveat must be an object: {}", value));
                        }
                        caveats.push(value.to_owned());
                    }
                    abilities.insert(ability, caveats);
                }
                abilities
            };

            resources.insert(resource, abilities);
        }

        Capabilities::try_from(resources)
    }
}
