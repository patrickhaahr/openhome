use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::services::training::calendar::CalendarDay;

#[derive(Debug, thiserror::Error)]
pub enum GoalError {
    #[error("{0}")]
    Validation(String),
    #[error("Nutrition Goal database operation failed")]
    Database(#[from] sqlx::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum Direction {
    Lose,
    Maintain,
    LeanBulk,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Goal {
    pub direction: Direction,
    pub rate_kg_per_week: f64,
}

pub const DEFAULT_GOAL: Goal = Goal {
    direction: Direction::Lose,
    rate_kg_per_week: 0.5,
};

const RATES: [(Direction, &[f64]); 3] = [
    (Direction::Lose, &[0.25, 0.5, 0.75]),
    (Direction::Maintain, &[0.0]),
    (Direction::LeanBulk, &[0.1, 0.25]),
];

impl Goal {
    fn parse(direction: Direction, rate_kg_per_week: Option<f64>) -> Result<Self, GoalError> {
        let offered = RATES
            .iter()
            .find(|(offered, _)| *offered == direction)
            .map_or(&[][..], |(_, rates)| *rates);
        let rate = match (direction, rate_kg_per_week) {
            (Direction::Maintain, None) => 0.0,
            (_, Some(rate)) => rate,
            (_, None) => {
                return Err(GoalError::Validation(
                    "rate_kg_per_week is required to lose or lean bulk".into(),
                ));
            }
        };
        offered
            .iter()
            .find(|offered| (**offered - rate).abs() < 1e-9)
            .map(|rate| Self {
                direction,
                rate_kg_per_week: *rate,
            })
            .ok_or_else(|| {
                GoalError::Validation(
                    "Choose lose 0.25, 0.5 or 0.75 kg per week, maintain, or lean_bulk 0.1 or \
                     0.25 kg per week"
                        .into(),
                )
            })
    }

    #[must_use]
    pub fn weekly_change_kg(self) -> f64 {
        match self.direction {
            Direction::Lose => -self.rate_kg_per_week,
            Direction::Maintain => 0.0,
            Direction::LeanBulk => self.rate_kg_per_week,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct DatedGoal {
    #[serde(flatten)]
    pub goal: Goal,
    pub effective_from: Option<CalendarDay>,
}

#[derive(Clone, Debug, Default)]
pub struct GoalHistory(Vec<DatedGoal>);

impl GoalHistory {
    #[must_use]
    pub fn on(&self, day: CalendarDay) -> DatedGoal {
        self.0
            .iter()
            .rev()
            .find(|change| change.effective_from.is_some_and(|from| from <= day))
            .copied()
            .unwrap_or(DatedGoal {
                goal: DEFAULT_GOAL,
                effective_from: None,
            })
    }
}

#[derive(Debug, Serialize)]
pub struct NutritionGoal {
    pub goal: DatedGoal,
    pub history: Vec<DatedGoal>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetGoal {
    pub direction: Direction,
    #[serde(default)]
    pub rate_kg_per_week: Option<f64>,
}

pub async fn history(db: &SqlitePool) -> Result<GoalHistory, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT effective_from, direction AS "direction: Direction", rate_kg_per_week
        FROM nutrition_goals ORDER BY effective_from"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|row| {
            let day = CalendarDay::parse(&row.effective_from).ok_or_else(|| {
                sqlx::Error::Decode(format!("invalid goal day {}", row.effective_from).into())
            })?;
            Ok(DatedGoal {
                goal: Goal {
                    direction: row.direction,
                    rate_kg_per_week: row.rate_kg_per_week,
                },
                effective_from: Some(day),
            })
        })
        .collect::<Result<_, _>>()
        .map(GoalHistory)
}

pub async fn nutrition_goal(
    db: &SqlitePool,
    today: CalendarDay,
) -> Result<NutritionGoal, GoalError> {
    let history = history(db).await?;
    Ok(NutritionGoal {
        goal: history.on(today),
        history: history.0,
    })
}

pub async fn set_nutrition_goal(
    db: &SqlitePool,
    today: CalendarDay,
    request: SetGoal,
) -> Result<NutritionGoal, GoalError> {
    let goal = Goal::parse(request.direction, request.rate_kg_per_week)?;
    let day = today.to_string();
    sqlx::query!(
        r#"INSERT INTO nutrition_goals (effective_from, direction, rate_kg_per_week)
        VALUES (?1, ?2, ?3)
        ON CONFLICT (effective_from) DO UPDATE SET direction = excluded.direction,
            rate_kg_per_week = excluded.rate_kg_per_week,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
        day,
        goal.direction,
        goal.rate_kg_per_week
    )
    .execute(db)
    .await?;
    nutrition_goal(db, today).await
}
