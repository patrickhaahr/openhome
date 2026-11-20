use super::SMOOTHING;
use crate::services::nutrition::intake::Intake;
use crate::services::nutrition::meal_logs::{self, MealLogError, Selection, StoredPortion, Values};
use crate::services::nutrition::parameters;
use crate::services::nutrition::supplements::StoredDose;
use crate::services::training::calendar::CalendarDay;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct TrendWeight {
    points: Vec<(CalendarDay, f64)>,
}

impl TrendWeight {
    #[must_use]
    pub fn of(weigh_ins: &[(CalendarDay, f64)]) -> Self {
        let mut trend: Option<f64> = None;
        let points = weigh_ins
            .iter()
            .map(|(day, weight)| {
                let next = trend.map_or(*weight, |trend| trend + SMOOTHING * (weight - trend));
                trend = Some(next);
                (*day, next)
            })
            .collect();
        Self { points }
    }

    #[must_use]
    pub fn on(&self, day: CalendarDay) -> Option<f64> {
        let after = self.points.partition_point(|(weighed, _)| *weighed <= day);
        after.checked_sub(1).map(|index| self.points[index].1)
    }

    pub(super) fn first(&self) -> Option<(CalendarDay, f64)> {
        self.points.first().copied()
    }

    pub(super) fn first_between(
        &self,
        from: CalendarDay,
        to: CalendarDay,
    ) -> Option<(CalendarDay, f64)> {
        self.points
            .iter()
            .find(|(day, _)| *day >= from && *day <= to)
            .copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

impl Sex {
    pub(in crate::services::nutrition) fn parse(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "male" => Some(Self::Male),
            "female" => Some(Self::Female),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Missing {
    Height,
    Sex,
    BirthDate,
    BodyWeight,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Body {
    height_cm: f64,
    sex: Sex,
    birth_date: CalendarDay,
}

impl Body {
    pub(super) async fn read(db: &SqlitePool) -> Result<Result<Self, Vec<Missing>>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT height_cm, sex, birth_date AS "birth_date?" FROM profile WHERE id = 1"#
        )
        .fetch_optional(db)
        .await?;
        let (height, sex, birth) = row.map_or((None, None, None), |row| {
            (row.height_cm, row.sex, row.birth_date)
        });
        let height_cm = height.filter(|height| *height > 0.0);
        let sex = sex.as_deref().and_then(Sex::parse);
        let birth_date = birth.as_deref().and_then(CalendarDay::parse);
        Ok(match (height_cm, sex, birth_date) {
            (Some(height_cm), Some(sex), Some(birth_date)) => Ok(Self {
                height_cm,
                sex,
                birth_date,
            }),
            _ => Err([
                (height_cm.is_none(), Missing::Height),
                (sex.is_none(), Missing::Sex),
                (birth_date.is_none(), Missing::BirthDate),
            ]
            .into_iter()
            .filter_map(|(missing, what)| missing.then_some(what))
            .collect()),
        })
    }

    pub(super) fn bmr(self, weight_kg: f64, day: CalendarDay) -> Option<f64> {
        let age = f64::from(day.years_since(self.birth_date)?);
        let offset = match self.sex {
            Sex::Male => 5.0,
            Sex::Female => -161.0,
        };
        Some(10.0 * weight_kg + 6.25 * self.height_cm - 5.0 * age + offset)
    }
}

pub(in crate::services::nutrition) async fn weigh_ins(
    db: &SqlitePool,
    to: CalendarDay,
) -> Result<Vec<(CalendarDay, f64)>, sqlx::Error> {
    let to = to.to_string();
    let rows = sqlx::query!(
        r#"SELECT CAST(date AS TEXT) AS "date!: String", weight_kg FROM body_weight
        WHERE date <= ?1 ORDER BY date"#,
        to
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| Some((CalendarDay::parse(&row.date)?, row.weight_kg)))
        .collect())
}

pub(super) async fn logged_energy(
    db: &SqlitePool,
    from: CalendarDay,
    to: CalendarDay,
) -> Result<BTreeMap<CalendarDay, f64>, MealLogError> {
    if to < from {
        return Ok(BTreeMap::new());
    }
    let logs = meal_logs::load(
        db,
        &Selection::Days(from, to),
        Values::Only(&meal_logs::KEY_NUTRIENTS),
    )
    .await?;
    let mut by_day: BTreeMap<CalendarDay, Intake> = BTreeMap::new();
    for log in &logs {
        let Some(day) = CalendarDay::parse(&log.day) else {
            continue;
        };
        let portions: Vec<&StoredPortion> = log.portions.iter().collect();
        let doses: Vec<&StoredDose> = log.doses.iter().collect();
        let intake = Intake::of(parameters::ENERGY_KCAL, &portions, &doses);
        let total = by_day.remove(&day).unwrap_or_default() + intake;
        by_day.insert(day, total);
    }
    Ok(by_day
        .into_iter()
        .filter_map(|(day, intake)| Some((day, intake.total()?)))
        .collect())
}
