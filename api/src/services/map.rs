//! Private origin for the dashboard's shared maps.

use serde::Serialize;

/// Central Copenhagen demonstration point, unrelated to any household.
const DEMO_LATITUDE: f64 = 55.6869;
const DEMO_LONGITUDE: f64 = 12.5692;

/// Where shared maps open. Only authenticated clients receive the coordinates; never log them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MapOrigin {
    latitude: f64,
    longitude: f64,
    source: MapOriginSource,
}

/// Whether the origin is the household's private configuration or the explicit public demo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapOriginSource {
    Private,
    Demo,
}

impl MapOrigin {
    /// Longitude and latitude for server-side integrations; never log them.
    #[must_use]
    pub fn coordinates(self) -> [f64; 2] {
        [self.longitude, self.latitude]
    }

    /// Private coordinates must be supplied together. The demo origin is only used when explicitly
    /// enabled and never combined with private coordinates, so it cannot become a silent default.
    pub fn parse(
        latitude: Option<&str>,
        longitude: Option<&str>,
        demo: Option<&str>,
    ) -> anyhow::Result<Option<Self>> {
        let demo = match demo {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => anyhow::bail!("MAP_DEMO_ORIGIN must be true or false"),
        };
        match (latitude, longitude, demo) {
            (None, None, false) => Ok(None),
            (None, None, true) => Ok(Some(Self {
                latitude: DEMO_LATITUDE,
                longitude: DEMO_LONGITUDE,
                source: MapOriginSource::Demo,
            })),
            (Some(lat), Some(lon), false) => {
                let latitude: f64 = lat
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid MAP_ORIGIN_LATITUDE"))?;
                let longitude: f64 = lon
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid MAP_ORIGIN_LONGITUDE"))?;
                anyhow::ensure!(
                    latitude.is_finite() && (-85.0..=85.0).contains(&latitude),
                    "Invalid MAP_ORIGIN_LATITUDE"
                );
                anyhow::ensure!(
                    longitude.is_finite() && (-180.0..=180.0).contains(&longitude),
                    "Invalid MAP_ORIGIN_LONGITUDE"
                );
                Ok(Some(Self {
                    latitude,
                    longitude,
                    source: MapOriginSource::Private,
                }))
            }
            (_, _, true) => {
                anyhow::bail!("Set MAP_DEMO_ORIGIN or private MAP_ORIGIN coordinates, not both")
            }
            _ => anyhow::bail!("Set both MAP_ORIGIN_LATITUDE and MAP_ORIGIN_LONGITUDE"),
        }
    }
}

/// Origin read for map clients, without echoing configuration errors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum MapOriginRead {
    Configured { origin: MapOrigin },
    Unavailable { reason: MapUnavailable },
}

/// Expected reasons a map cannot be centered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapUnavailable {
    MissingConfiguration,
}

impl From<Option<MapOrigin>> for MapOriginRead {
    fn from(origin: Option<MapOrigin>) -> Self {
        match origin {
            Some(origin) => Self::Configured { origin },
            None => Self::Unavailable {
                reason: MapUnavailable::MissingConfiguration,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_private_coordinates_together() {
        let origin = MapOrigin::parse(Some("55.5"), Some("12.25"), None)
            .unwrap()
            .unwrap();
        assert_eq!(origin.source, MapOriginSource::Private);
        assert_eq!((origin.latitude, origin.longitude), (55.5, 12.25));
        assert_eq!(MapOrigin::parse(None, None, Some("false")).unwrap(), None);
    }

    #[test]
    fn uses_the_demo_only_when_explicitly_enabled() {
        let origin = MapOrigin::parse(None, None, Some("true")).unwrap().unwrap();
        assert_eq!(origin.source, MapOriginSource::Demo);
        assert_eq!(MapOrigin::parse(None, None, None).unwrap(), None);
    }

    #[test]
    fn rejects_partial_invalid_or_mixed_configuration_without_echoing_input() {
        for (lat, lon, demo) in [
            (Some("55.5"), None, None),
            (None, Some("12.25"), None),
            (Some("north"), Some("12.25"), None),
            (Some("NaN"), Some("12.25"), None),
            (Some("89"), Some("12.25"), None),
            (Some("55.5"), Some("181"), None),
            (Some("55.5"), Some("12.25"), Some("true")),
            (None, None, Some("yes")),
        ] {
            let error = MapOrigin::parse(lat, lon, demo).unwrap_err().to_string();
            assert!(!error.contains("55.5") && !error.contains("12.25"));
        }
    }
}
