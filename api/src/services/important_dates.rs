//! Household-shared Important Dates. Nobody owns a date; it tags the Household Users it
//! concerns (none, one or several). Axum computes every figure for a Europe/Copenhagen day.

pub mod figures;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use self::figures::{Elapsed, Milestone, Next};
use super::patch::Patch;

#[derive(Debug, thiserror::Error)]
pub enum ImportantDateError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Important Date database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, ImportantDateError>;

/// `since` counts up from the date; `yearly` counts down to its next anniversary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum DateKind {
    Since,
    Yearly,
}

/// A stored date with its figures for one day. Both figures are given for either kind, so a
/// view can lead with the one its kind names: `elapsed` for since, `next` for yearly.
#[derive(Debug, Serialize)]
pub struct ImportantDate {
    pub id: String,
    pub title: String,
    pub date: NaiveDate,
    pub kind: DateKind,
    /// Tagged Household User IDs, ordered by name; empty when the date concerns no member.
    pub person_ids: Vec<String>,
    pub show_when_idle: bool,
    /// Null while the date is still ahead of the day.
    pub elapsed: Option<Elapsed>,
    pub next: Next,
    /// Today's celebration, independent of idle visibility; idle reads filter `show_when_idle`.
    pub milestone: Option<Milestone>,
}

/// Every date's figures for one Europe/Copenhagen day: since dates first (oldest first), then
/// yearly dates by their next occurrence.
#[derive(Debug, Serialize)]
pub struct ImportantDates {
    pub on: NaiveDate,
    pub dates: Vec<ImportantDate>,
}

struct Row {
    id: String,
    title: String,
    date: String,
    kind: DateKind,
    show_when_idle: bool,
    person_ids: Option<String>,
}

impl Row {
    fn figure(self, on: NaiveDate) -> Result<ImportantDate> {
        let date = parse_day(&self.date, "date")?;
        let elapsed = figures::elapsed(date, on);
        let next = figures::next(date, on);
        Ok(ImportantDate {
            elapsed,
            next,
            milestone: figures::milestone(self.kind, elapsed, next),
            id: self.id,
            title: self.title,
            date,
            kind: self.kind,
            person_ids: self
                .person_ids
                .map(|ids| ids.split(',').map(str::to_owned).collect())
                .unwrap_or_default(),
            show_when_idle: self.show_when_idle,
        })
    }
}

/// Optional filters and the day the figures are for (today in Europe/Copenhagen when absent).
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatesQuery {
    pub on: Option<String>,
    pub show_when_idle: Option<bool>,
}

/// The day the figures are for.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnQuery {
    pub on: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddDate {
    pub title: String,
    pub date: String,
    pub kind: DateKind,
    /// Required, even when empty: tags are never inferred.
    pub person_ids: Vec<String>,
    #[serde(default)]
    pub show_when_idle: bool,
    pub request_id: String,
}

#[derive(Serialize)]
pub struct Addition {
    pub important_date: ImportantDate,
    pub replayed: bool,
}

/// Omitted fields keep their value; none of them can be cleared with null.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditDate {
    #[serde(default)]
    pub title: Patch<String>,
    #[serde(default)]
    pub date: Patch<String>,
    #[serde(default)]
    pub kind: Patch<DateKind>,
    #[serde(default)]
    pub person_ids: Patch<Vec<String>>,
    #[serde(default)]
    pub show_when_idle: Patch<bool>,
}

struct AdditionReceipt {
    date_id: String,
    title: String,
    date: String,
    kind: DateKind,
    person_ids: String,
    show_when_idle: bool,
}

fn text(value: &str, field: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(ImportantDateError::Validation(format!(
            "{field} must contain 1-{max} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

fn parse_day(value: &str, field: &str) -> Result<NaiveDate> {
    let shaped = value.len() == 10
        && value.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(day) if shaped && !value.starts_with("0000") => Ok(day),
        _ => Err(ImportantDateError::Validation(format!(
            "{field} must be a real calendar date in YYYY-MM-DD (years 0001-9999)"
        ))),
    }
}

/// The requested day, or today in Europe/Copenhagen.
pub fn resolve_on(on: Option<&str>) -> Result<NaiveDate> {
    on.map_or_else(
        || Ok(figures::day_at(chrono::Utc::now())),
        |value| parse_day(value, "on"),
    )
}

/// Validate explicit tags: each must be a configured Household User, listed once.
async fn people(db: &SqlitePool, ids: &[String]) -> Result<Vec<String>> {
    let mut sorted: Vec<String> = ids.iter().map(|id| id.trim().to_owned()).collect();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ImportantDateError::Validation(
            "person_ids must list each Household User once".into(),
        ));
    }
    for id in &sorted {
        if sqlx::query!("SELECT id FROM household_users WHERE id = ?", id)
            .fetch_optional(db)
            .await?
            .is_none()
        {
            return Err(ImportantDateError::NotFound(
                "Household User not found; discover household users first".into(),
            ));
        }
    }
    Ok(sorted)
}

async fn rows(db: &SqlitePool, id: Option<&str>) -> Result<Vec<Row>> {
    Ok(sqlx::query_as!(
        Row,
        r#"SELECT d.id AS "id!", d.title, d.date, d.kind AS "kind: DateKind", d.show_when_idle AS "show_when_idle: bool",
        (SELECT group_concat(person_id, ',') FROM (SELECT p.person_id FROM important_date_people p
            JOIN household_users u ON u.id = p.person_id WHERE p.date_id = d.id ORDER BY u.name, u.id)) AS "person_ids?: String"
        FROM important_dates d WHERE (? IS NULL OR d.id = ?)"#,
        id,
        id
    )
    .fetch_all(db)
    .await?)
}

/// All dates with their figures for `query.on`, optionally only those shown when idle.
pub async fn dates(db: &SqlitePool, query: &DatesQuery) -> Result<ImportantDates> {
    let on = resolve_on(query.on.as_deref())?;
    let mut dates = rows(db, None)
        .await?
        .into_iter()
        .filter(|row| {
            query
                .show_when_idle
                .is_none_or(|idle| row.show_when_idle == idle)
        })
        .map(|row| row.figure(on))
        .collect::<Result<Vec<_>>>()?;
    dates.sort_by(|a, b| {
        let key = |value: &ImportantDate| match value.kind {
            DateKind::Since => (0, value.date),
            DateKind::Yearly => (1, value.next.date),
        };
        key(a)
            .cmp(&key(b))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(ImportantDates { on, dates })
}

/// One date with its figures for `on`.
pub async fn date(db: &SqlitePool, id: &str, on: NaiveDate) -> Result<ImportantDate> {
    rows(db, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| ImportantDateError::NotFound("Important Date not found".into()))?
        .figure(on)
}

impl AdditionReceipt {
    async fn replay(self, db: &SqlitePool, request: &AddDate, people: &str) -> Result<Addition> {
        if self.title != request.title
            || self.date != request.date
            || self.kind != request.kind
            || self.person_ids != people
            || self.show_when_idle != request.show_when_idle
        {
            return Err(ImportantDateError::Conflict(
                "request_id was already used for a different addition; use a new ID for a new request".into(),
            ));
        }
        let important_date = date(db, &self.date_id, resolve_on(None)?)
            .await
            .map_err(|error| match error {
                ImportantDateError::NotFound(_) => ImportantDateError::Conflict(
                    "The retried date was removed; use a new request_id to add it again".into(),
                ),
                other => other,
            })?;
        Ok(Addition {
            important_date,
            replayed: true,
        })
    }
}

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<AdditionReceipt>> {
    Ok(sqlx::query_as!(
        AdditionReceipt,
        r#"SELECT date_id, title, date, kind AS "kind: DateKind", person_ids, show_when_idle AS "show_when_idle: bool"
        FROM important_date_addition_receipts WHERE request_id = ?"#,
        request_id
    )
    .fetch_optional(db)
    .await?)
}

/// Store a date, its tags and an immutable receipt together. Rejected additions consume no ID.
pub async fn add_date(db: &SqlitePool, mut request: AddDate) -> Result<Addition> {
    request.title = text(&request.title, "title", 200)?;
    request.request_id = text(&request.request_id, "request_id", 128)?;
    parse_day(&request.date, "date")?;
    let mut tagged: Vec<String> = request
        .person_ids
        .iter()
        .map(|id| id.trim().to_owned())
        .collect();
    tagged.sort();
    let receipt_people = tagged.join(",");
    // Replay first, so a since-removed Household User cannot fail an accepted retry.
    if let Some(receipt) = previous(db, &request.request_id).await? {
        return receipt.replay(db, &request, &receipt_people).await;
    }
    let tagged = people(db, &request.person_ids).await?;
    let id = Uuid::new_v4().to_string();
    let mut transaction = db.begin().await?;
    // Claim first to serialize simultaneous retries. The receipt has no cascading foreign key.
    let claimed = sqlx::query!(
        "INSERT INTO important_date_addition_receipts (request_id, date_id, title, date, kind, person_ids, show_when_idle) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(request_id) DO NOTHING RETURNING request_id",
        request.request_id, id, request.title, request.date, request.kind, receipt_people, request.show_when_idle)
        .fetch_optional(&mut *transaction).await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return previous(db, &request.request_id)
            .await?
            .ok_or_else(|| {
                ImportantDateError::Conflict(
                    "Retry receipt unavailable; refresh before trying again".into(),
                )
            })?
            .replay(db, &request, &receipt_people)
            .await;
    }
    sqlx::query!(
        "INSERT INTO important_dates (id, title, date, kind, show_when_idle) VALUES (?, ?, ?, ?, ?)",
        id,
        request.title,
        request.date,
        request.kind,
        request.show_when_idle
    )
    .execute(&mut *transaction)
    .await?;
    for person in &tagged {
        sqlx::query!(
            "INSERT INTO important_date_people (date_id, person_id) VALUES (?, ?)",
            id,
            person
        )
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(Addition {
        important_date: date(db, &id, resolve_on(None)?).await?,
        replayed: false,
    })
}

fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(ImportantDateError::Validation(format!(
            "{field} cannot be null"
        ))),
    }
}

/// Apply the supplied fields together; replacing `person_ids` replaces every tag.
pub async fn edit_date(db: &SqlitePool, id: &str, request: EditDate) -> Result<ImportantDate> {
    let title = required(request.title, "title")?
        .map(|value| text(&value, "title", 200))
        .transpose()?;
    let day = required(request.date, "date")?;
    if let Some(value) = &day {
        parse_day(value, "date")?;
    }
    let kind = required(request.kind, "kind")?;
    let show_when_idle = required(request.show_when_idle, "show_when_idle")?;
    let tagged = match required(request.person_ids, "person_ids")? {
        Some(ids) => Some(people(db, &ids).await?),
        None => None,
    };
    let mut transaction = db.begin().await?;
    let updated = sqlx::query!(
        "UPDATE important_dates SET title = COALESCE(?, title), date = COALESCE(?, date), kind = COALESCE(?, kind), show_when_idle = COALESCE(?, show_when_idle) WHERE id = ? RETURNING id",
        title, day, kind, show_when_idle, id)
        .fetch_optional(&mut *transaction)
        .await?;
    if updated.is_none() {
        return Err(ImportantDateError::NotFound(
            "Important Date not found".into(),
        ));
    }
    if let Some(tagged) = tagged {
        sqlx::query!("DELETE FROM important_date_people WHERE date_id = ?", id)
            .execute(&mut *transaction)
            .await?;
        for person in &tagged {
            sqlx::query!(
                "INSERT INTO important_date_people (date_id, person_id) VALUES (?, ?)",
                id,
                person
            )
            .execute(&mut *transaction)
            .await?;
        }
    }
    transaction.commit().await?;
    date(db, id, resolve_on(None)?).await
}

/// Remove a date and its tags. Its accepted addition receipt stays retired.
pub async fn remove_date(db: &SqlitePool, id: &str) -> Result<()> {
    let removed = sqlx::query!("DELETE FROM important_dates WHERE id = ?", id)
        .execute(db)
        .await?;
    if removed.rows_affected() == 0 {
        return Err(ImportantDateError::NotFound(
            "Important Date not found".into(),
        ));
    }
    Ok(())
}
