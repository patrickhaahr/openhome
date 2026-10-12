use rmcp::schemars;
use serde::{Deserialize, Serialize};

/// Distinguishes omitted fields from explicit null without interpreting their values.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Patch<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}

impl<T> Patch<T> {
    pub(super) fn missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}
