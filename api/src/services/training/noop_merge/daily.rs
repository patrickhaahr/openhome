use super::Namespace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: Option<T>,
    pub source: Option<Namespace>,
}

impl<T: Copy> Sourced<T> {
    pub(super) fn of(value: Option<T>, namespace: Namespace) -> Self {
        Self {
            value,
            source: value.map(|_| namespace),
        }
    }
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct DailyRow {
    pub total_sleep_min: Option<f64>,
    pub efficiency: Option<f64>,
    pub deep_min: Option<f64>,
    pub rem_min: Option<f64>,
    pub light_min: Option<f64>,
    pub disturbances: Option<i64>,
    pub resting_hr: Option<f64>,
    pub avg_hrv: Option<f64>,
    pub recovery: Option<f64>,
    pub strain: Option<f64>,
    pub skin_temp_dev_c: Option<f64>,
    pub resp_rate_bpm: Option<f64>,
    pub spo2_pct: Option<f64>,
    pub steps: Option<i64>,
}

impl DailyRow {
    const fn is_bare_sleep_aggregate(&self) -> bool {
        self.total_sleep_min.is_some()
            && self.efficiency.is_none()
            && self.deep_min.is_none()
            && self.rem_min.is_none()
            && self.light_min.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDaily {
    pub total_sleep_min: Sourced<f64>,
    pub efficiency: Sourced<f64>,
    pub deep_min: Sourced<f64>,
    pub rem_min: Sourced<f64>,
    pub light_min: Sourced<f64>,
    pub disturbances: Sourced<i64>,
    pub resting_hr: Sourced<f64>,
    pub avg_hrv: Sourced<f64>,
    pub recovery: Sourced<f64>,
    pub strain: Sourced<f64>,
    pub skin_temp_dev_c: Sourced<f64>,
    pub resp_rate_bpm: Sourced<f64>,
    pub spo2_pct: Sourced<f64>,
    pub steps: Sourced<i64>,
}

pub fn merge_daily(
    imported: Option<&DailyRow>,
    computed: Option<&DailyRow>,
    sleep_edited: bool,
) -> ResolvedDaily {
    fn pick<T: Copy>(
        imported: Option<&DailyRow>,
        computed: Option<&DailyRow>,
        field: fn(&DailyRow) -> Option<T>,
    ) -> Sourced<T> {
        match imported.and_then(field) {
            Some(value) => Sourced::of(Some(value), Namespace::Imported),
            None => Sourced::of(computed.and_then(field), Namespace::Computed),
        }
    }

    let mut resolved = ResolvedDaily {
        total_sleep_min: pick(imported, computed, |row| row.total_sleep_min),
        efficiency: pick(imported, computed, |row| row.efficiency),
        deep_min: pick(imported, computed, |row| row.deep_min),
        rem_min: pick(imported, computed, |row| row.rem_min),
        light_min: pick(imported, computed, |row| row.light_min),
        disturbances: pick(imported, computed, |row| row.disturbances),
        resting_hr: pick(imported, computed, |row| row.resting_hr),
        avg_hrv: pick(imported, computed, |row| row.avg_hrv),
        recovery: pick(imported, computed, |row| row.recovery),
        strain: pick(imported, computed, |row| row.strain),
        skin_temp_dev_c: pick(imported, computed, |row| row.skin_temp_dev_c),
        resp_rate_bpm: pick(imported, computed, |row| row.resp_rate_bpm),
        spo2_pct: pick(imported, computed, |row| row.spo2_pct),
        steps: pick(imported, computed, |row| row.steps),
    };

    if let (Some(imported), Some(computed)) = (imported, computed) {
        let computed_night_replaces_bare_import = imported.is_bare_sleep_aggregate()
            && computed.total_sleep_min.is_some()
            && !computed.is_bare_sleep_aggregate();
        if sleep_edited || computed_night_replaces_bare_import {
            let from_computed = |value| Sourced::of(value, Namespace::Computed);
            resolved.total_sleep_min = from_computed(computed.total_sleep_min);
            resolved.efficiency = from_computed(computed.efficiency);
            resolved.deep_min = from_computed(computed.deep_min);
            resolved.rem_min = from_computed(computed.rem_min);
            resolved.light_min = from_computed(computed.light_min);
            resolved.disturbances = Sourced::of(computed.disturbances, Namespace::Computed);
        }
    }
    resolved
}
