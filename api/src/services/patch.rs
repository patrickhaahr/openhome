use serde::Deserialize;

/// An absent field preserves its stored value; explicit null clears a nullable field.
#[derive(Default)]
pub enum Patch<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}

impl<T> Patch<T> {
    pub(super) fn supplied(&self) -> bool {
        matches!(self, Self::Value(_))
    }
    pub(super) fn value(self) -> Option<T> {
        match self {
            Self::Missing => None,
            Self::Value(value) => value,
        }
    }
}
