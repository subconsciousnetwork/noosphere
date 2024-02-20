use std::ops::Deref;

use anyhow::{anyhow, Error, Result};
use serde_json::{Map, Value};

#[derive(Clone)]
pub struct Caveat(Map<String, Value>);

impl Caveat {
    /// Determines if this [Caveat] enables/allows the provided caveat.
    ///
    /// ```
    /// use noosphere_ucan::capability::{Caveat};
    /// use serde_json::json;
    ///
    /// let no_caveat = Caveat::try_from(json!({})).unwrap();
    /// let x_caveat = Caveat::try_from(json!({ "x": true })).unwrap();
    /// let x_diff_caveat = Caveat::try_from(json!({ "x": false })).unwrap();
    /// let y_caveat = Caveat::try_from(json!({ "y": true })).unwrap();
    /// let xz_caveat = Caveat::try_from(json!({ "x": true, "z": true })).unwrap();
    ///
    /// assert!(no_caveat.enables(&no_caveat));
    /// assert!(x_caveat.enables(&x_caveat));
    /// assert!(no_caveat.enables(&x_caveat));
    /// assert!(x_caveat.enables(&xz_caveat));
    ///
    /// assert!(!x_caveat.enables(&x_diff_caveat));
    /// assert!(!x_caveat.enables(&no_caveat));
    /// assert!(!x_caveat.enables(&y_caveat));
    /// ```
    pub fn enables(&self, other: &Caveat) -> bool {
        if self.is_empty() {
            return true;
        }

        if other.is_empty() {
            return false;
        }

        if self == other {
            return true;
        }

        for (key, value) in self.iter() {
            if let Some(other_value) = other.get(key) {
                if value != other_value {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl Deref for Caveat {
    type Target = Map<String, Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for Caveat {
    fn eq(&self, other: &Caveat) -> bool {
        self.0 == other.0
    }
}

impl TryFrom<Value> for Caveat {
    type Error = Error;
    fn try_from(value: Value) -> Result<Caveat> {
        Ok(Caveat(match value {
            Value::Object(obj) => obj,
            _ => return Err(anyhow!("Caveat must be an object")),
        }))
    }
}

impl TryFrom<&Value> for Caveat {
    type Error = Error;
    fn try_from(value: &Value) -> Result<Caveat> {
        Caveat::try_from(value.to_owned())
    }
}
