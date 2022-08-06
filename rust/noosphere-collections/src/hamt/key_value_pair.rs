// Adapted for Noosphere from https://github.com/filecoin-project/ref-fvm
// Source copyright and license:
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use serde::{Deserialize, Serialize};

use super::TargetConditionalSendSync;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct KeyValuePair<K, V>(K, V);

impl<K: TargetConditionalSendSync, V: TargetConditionalSendSync> KeyValuePair<K, V> {
    pub fn key(&self) -> &K {
        &self.0
    }
    pub fn value(&self) -> &V {
        &self.1
    }
    pub fn value_mut(&mut self) -> &mut V {
        &mut self.1
    }
    pub fn take(self) -> (K, V) {
        (self.0, self.1)
    }
    pub fn new(key: K, value: V) -> Self {
        KeyValuePair(key, value)
    }
}
