//! A NOOP Workout's captured route: decoded from NOOP's stored polyline and thinned for display.
//!
//! NOOP stores a route as a Google Encoded Polyline, precision 5 (about 1 m), latitude and
//! longitude only: `RouteMath.encode` and `RouteMath.decode` in `Strand/App/GpsWorkoutRecorder.swift`
//! of the NOOP app. The decoder here follows `RouteMath.decode`: it keeps the points it can parse,
//! stops cleanly at a truncated value, and drops anything off the surface of the earth.
//!
//! Route coordinates are private household data, like the map origin in `services::map`: they are
//! only returned to authenticated clients and must never be logged.

/// Points a returned route keeps at most, the first and last always among them.
pub const MAX_ROUTE_POINTS: usize = 300;

/// One decoded fix as `[latitude, longitude]` in degrees.
pub type Fix = [f64; 2];

/// Decodes a precision-5 polyline; `None` when it holds no usable point.
#[must_use]
pub fn decode(encoded: &str) -> Option<Vec<Fix>> {
    let bytes = encoded.as_bytes();
    let mut at = 0;
    let (mut lat, mut lon) = (0_i64, 0_i64);
    let mut fixes = Vec::new();
    while at < bytes.len() {
        let Some(d_lat) = signed(bytes, &mut at) else {
            break;
        };
        let Some(d_lon) = signed(bytes, &mut at) else {
            break;
        };
        lat += d_lat;
        lon += d_lon;
        #[allow(clippy::cast_precision_loss)]
        let fix = [lat as f64 / 1e5, lon as f64 / 1e5];
        if (-90.0..=90.0).contains(&fix[0]) && (-180.0..=180.0).contains(&fix[1]) {
            fixes.push(fix);
        }
    }
    (!fixes.is_empty()).then_some(fixes)
}

/// One zigzag varint, advancing `at`; `None` when the string ends mid-value or a value is too
/// long to be a coordinate.
fn signed(bytes: &[u8], at: &mut usize) -> Option<i64> {
    let (mut result, mut shift) = (0_i64, 0_u32);
    loop {
        let byte = i64::from(*bytes.get(*at)?) - 63;
        if byte < 0 || shift > 30 {
            return None;
        }
        *at += 1;
        result |= (byte & 0x1f) << shift;
        shift += 5;
        if byte < 0x20 {
            break;
        }
    }
    Some(if result & 1 == 1 {
        !(result >> 1)
    } else {
        result >> 1
    })
}

/// Keeps at most `limit` fixes, evenly spaced by index, always including the first and the last.
#[must_use]
pub fn downsample(fixes: Vec<Fix>, limit: usize) -> Vec<Fix> {
    if fixes.len() <= limit || limit < 2 {
        return fixes;
    }
    let last = fixes.len() - 1;
    (0..limit)
        .map(|index| fixes[index * last / (limit - 1)])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Google documentation's example: (38.5, -120.2), (40.7, -120.95), (43.252, -126.453).
    const GOOGLE_EXAMPLE: &str = "_p~iF~ps|U_ulLnnqC_mqNvxq`@";

    /// Fixes within a hundred-thousandth of a degree, the polyline's own resolution.
    fn same(actual: Option<Vec<Fix>>, expected: &[Fix]) {
        let actual = actual.expect("a route");
        assert_eq!(actual.len(), expected.len());
        for (fix, want) in actual.iter().zip(expected) {
            assert!((fix[0] - want[0]).abs() < 1e-6 && (fix[1] - want[1]).abs() < 1e-6);
        }
    }

    #[test]
    fn decodes_the_documented_google_example() {
        same(
            decode(GOOGLE_EXAMPLE),
            &[[38.5, -120.2], [40.7, -120.95], [43.252, -126.453]],
        );
    }

    #[test]
    fn keeps_what_it_can_parse_of_a_truncated_string() {
        // The third pair is cut after its latitude.
        same(
            decode("_p~iF~ps|U_ulLnnqC_mqN"),
            &[[38.5, -120.2], [40.7, -120.95]],
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_polyline() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("route-stand-in"), None);
        assert_eq!(decode("\u{7f}\u{1}"), None);
    }

    #[test]
    fn downsampling_keeps_both_ends_and_the_order() {
        let fixes: Vec<Fix> = (0..1000).map(|i| [f64::from(i), 0.0]).collect();
        let kept = downsample(fixes, 300);
        assert_eq!(kept.len(), 300);
        assert_eq!(kept[0][0].to_bits(), 0.0_f64.to_bits());
        assert_eq!(kept[299][0].to_bits(), 999.0_f64.to_bits());
        assert!(kept.windows(2).all(|pair| pair[0][0] < pair[1][0]));
    }

    #[test]
    fn a_short_route_is_left_alone() {
        let fixes = vec![[1.0, 2.0], [3.0, 4.0]];
        assert_eq!(downsample(fixes.clone(), 300).len(), fixes.len());
    }
}
