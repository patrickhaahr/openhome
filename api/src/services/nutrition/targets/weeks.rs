use super::{ACTIVITY_FACTOR, Body, FULL_BLEND_DAYS, KCAL_PER_KG, TrendWeight, WINDOW_DAYS};
use crate::services::training::calendar::CalendarDay;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Week {
    pub monday: CalendarDay,
    pub expenditure_kcal: f64,
    pub bmr_kcal: f64,
    pub prior_kcal: f64,
    pub data_estimate_kcal: Option<f64>,
    pub logged_days: usize,
    pub blend_weight: f64,
    pub trend_weight_kg: f64,
    pub trend_weight_day: CalendarDay,
    pub fixed: bool,
}

impl Week {
    pub(super) fn window(monday: CalendarDay) -> Option<(CalendarDay, CalendarDay)> {
        Some((monday.checked_sub_days(WINDOW_DAYS)?, monday.previous()))
    }

    pub(super) fn compute(
        monday: CalendarDay,
        last_read: CalendarDay,
        body: Body,
        trend: &TrendWeight,
        energy: &BTreeMap<CalendarDay, f64>,
    ) -> Option<Self> {
        let (window_start, window_end) = Self::window(monday)?;
        let (trend_weight_day, trend_weight_kg) = trend
            .on(window_end)
            .map(|weight| (window_end, weight))
            .or_else(|| trend.first_between(monday, last_read))?;
        let bmr_kcal = body.bmr(trend_weight_kg, monday)?;
        let prior_kcal = bmr_kcal * ACTIVITY_FACTOR;

        let logged: Vec<f64> = energy
            .range(window_start..=window_end)
            .map(|(_, kcal)| *kcal)
            .collect();
        let data_estimate_kcal = Self::data_estimate(window_start, window_end, trend, &logged);
        let blend_weight = if data_estimate_kcal.is_some() {
            logged.len().min(FULL_BLEND_DAYS) as f64 / FULL_BLEND_DAYS as f64
        } else {
            0.0
        };
        let expenditure_kcal = data_estimate_kcal.map_or(prior_kcal, |data| {
            blend_weight * data + (1.0 - blend_weight) * prior_kcal
        });
        (bmr_kcal > 0.0 && expenditure_kcal > 0.0).then_some(Self {
            monday,
            expenditure_kcal,
            bmr_kcal,
            prior_kcal,
            data_estimate_kcal,
            logged_days: logged.len(),
            blend_weight,
            trend_weight_kg,
            trend_weight_day,
            fixed: false,
        })
    }

    fn data_estimate(
        window_start: CalendarDay,
        window_end: CalendarDay,
        trend: &TrendWeight,
        logged: &[f64],
    ) -> Option<f64> {
        if logged.is_empty() {
            return None;
        }
        let before = window_start.previous();
        let (start_day, start) = trend
            .on(before)
            .map(|weight| (before, weight))
            .or_else(|| trend.first().filter(|(day, _)| *day >= window_start))?;
        let end = trend.on(window_end)?;
        let days = start_day.days_until(window_end);
        if days < 1 {
            return None;
        }
        let mean = logged.iter().sum::<f64>() / logged.len() as f64;
        Some(mean - (end - start) * KCAL_PER_KG / days as f64)
    }
}

pub(super) async fn stored_weeks(
    db: &SqlitePool,
    first: CalendarDay,
    last: CalendarDay,
) -> Result<BTreeMap<CalendarDay, Week>, sqlx::Error> {
    let (first, last) = (first.to_string(), last.to_string());
    let rows = sqlx::query!(
        r#"SELECT monday, expenditure_kcal, bmr_kcal, prior_kcal, data_estimate_kcal,
            logged_days, blend_weight, trend_weight_kg, trend_weight_day
        FROM nutrition_weeks WHERE monday BETWEEN ?1 AND ?2"#,
        first,
        last
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|row| {
            let day = |text: &str| {
                CalendarDay::parse(text)
                    .ok_or_else(|| sqlx::Error::Decode(format!("invalid week day {text}").into()))
            };
            let monday = day(&row.monday)?;
            Ok((
                monday,
                Week {
                    monday,
                    expenditure_kcal: row.expenditure_kcal,
                    bmr_kcal: row.bmr_kcal,
                    prior_kcal: row.prior_kcal,
                    data_estimate_kcal: row.data_estimate_kcal,
                    logged_days: usize::try_from(row.logged_days).unwrap_or_default(),
                    blend_weight: row.blend_weight,
                    trend_weight_kg: row.trend_weight_kg,
                    trend_weight_day: day(&row.trend_weight_day)?,
                    fixed: true,
                },
            ))
        })
        .collect()
}

pub(super) async fn freeze_weeks(
    db: &SqlitePool,
    weeks: &[Week],
) -> Result<BTreeMap<CalendarDay, Week>, sqlx::Error> {
    let (Some(first), Some(last)) = (weeks.first(), weeks.last()) else {
        return Ok(BTreeMap::new());
    };
    let mut transaction = db.begin().await?;
    for week in weeks {
        insert_week(&mut transaction, week).await?;
    }
    transaction.commit().await?;
    let mut stored = stored_weeks(db, first.monday, last.monday).await?;
    weeks
        .iter()
        .map(|week| {
            let frozen = stored
                .remove(&week.monday)
                .ok_or(sqlx::Error::RowNotFound)?;
            Ok((frozen.monday, frozen))
        })
        .collect()
}

async fn insert_week(
    transaction: &mut sqlx::SqliteConnection,
    week: &Week,
) -> Result<(), sqlx::Error> {
    let monday = week.monday.to_string();
    let trend_weight_day = week.trend_weight_day.to_string();
    let logged_days = week.logged_days as i64;
    sqlx::query!(
        r#"INSERT INTO nutrition_weeks (monday, expenditure_kcal, bmr_kcal, prior_kcal,
            data_estimate_kcal, logged_days, blend_weight, trend_weight_kg, trend_weight_day)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT (monday) DO NOTHING"#,
        monday,
        week.expenditure_kcal,
        week.bmr_kcal,
        week.prior_kcal,
        week.data_estimate_kcal,
        logged_days,
        week.blend_weight,
        week.trend_weight_kg,
        trend_weight_day
    )
    .execute(transaction)
    .await?;
    Ok(())
}
