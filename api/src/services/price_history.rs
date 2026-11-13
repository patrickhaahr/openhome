//! Price History (`CONTEXT.md`): the Card Prices the household observed for a Card, one Price Point per
//! observation date in USD cents. The first point is written when the Card is first stored and
//! each later one only when the catalogue's observation date is newer than the Card's latest, so
//! nothing is backfilled or interpolated. A read converts every point at its own date's rates.

use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};

use super::card_value::{Cents, Converted, convert};
use super::cards::{CardId, CardsError};
use super::exchange_rates;
use super::training::CalendarDay;

type Result<T, E = CardsError> = std::result::Result<T, E>;

fn cents_of(usd: f64) -> Result<i64, sqlx::Error> {
    Cents::from_dollars(usd)
        .and_then(Cents::whole_cents)
        .ok_or_else(|| sqlx::Error::Decode("price is not an amount".into()))
}

/// The point recorded with a Card the moment it is stored. A concurrent store of the same Card
/// loses the Card insert and so never reaches this.
pub(super) async fn record_first(
    tx: &mut SqliteConnection,
    card_id: &str,
    usd: f64,
    observed_on: &str,
) -> Result<(), sqlx::Error> {
    let cents = cents_of(usd)?;
    sqlx::query!(
        "INSERT INTO price_points (card_id, observed_on, price_usd_cents) VALUES (?, ?, ?)
         ON CONFLICT DO NOTHING",
        card_id,
        observed_on,
        cents
    )
    .execute(tx)
    .await?;
    Ok(())
}

/// Updates an owned Card's latest price and appends its point when `observed_on` is newer than
/// the Card's latest. The guarded update is one statement, so a retry or a concurrent run sees
/// the first one's date and adds nothing. `false` when the Card is unowned or already as new.
pub async fn record_newer(
    db: &SqlitePool,
    card_id: &str,
    usd: f64,
    observed_on: &str,
) -> Result<bool, sqlx::Error> {
    cents_of(usd)?;
    let mut tx = db.begin().await?;
    let updated = sqlx::query!(
        "UPDATE cards SET price_usd = ?, price_observed_on = ?
         WHERE id = ? AND (price_observed_on IS NULL OR price_observed_on < ?)
           AND EXISTS (SELECT 1 FROM binder_entries WHERE card_id = cards.id)",
        usd,
        observed_on,
        card_id,
        observed_on
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Ok(false);
    }
    record_first(&mut tx, card_id, usd, observed_on).await?;
    tx.commit().await?;
    Ok(true)
}

/// One Price Point. The converted amounts and the rate day are null when no ECB rates exist at
/// or before the point's date.
#[derive(Debug, Serialize)]
pub struct PricePoint {
    pub observed_on: CalendarDay,
    pub usd: Cents,
    pub eur: Option<Cents>,
    pub dkk: Option<Cents>,
    pub rate_date: Option<CalendarDay>,
}

#[derive(Debug, Serialize)]
pub struct PriceHistory {
    pub card_id: String,
    pub prices: Vec<PricePoint>,
}

fn bound(name: &str, text: Option<&str>) -> Result<Option<String>> {
    text.map(|text| {
        CalendarDay::parse(text)
            .map(|day| day.to_string())
            .ok_or_else(|| CardsError::Validation(format!("{name} must be a YYYY-MM-DD date")))
    })
    .transpose()
}

/// A Card's Price Points observed from `from` to `to` (both inclusive, either optional), oldest
/// first. A Card with no point in the range has an empty list.
pub async fn history(
    db: &SqlitePool,
    id: &CardId,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<PriceHistory> {
    let (from, to) = (bound("from", from)?, bound("to", to)?);
    if from
        .as_deref()
        .zip(to.as_deref())
        .is_some_and(|(from, to)| from > to)
    {
        return Err(CardsError::Validation("from must not be after to".into()));
    }
    let card_id = id.as_str();
    sqlx::query_scalar!(
        r#"SELECT 1 AS "found!: i64" FROM cards WHERE id = ?"#,
        card_id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| CardsError::NotFound("Card not found".into()))?;
    let rows = sqlx::query!(
        "SELECT observed_on, price_usd_cents FROM price_points
         WHERE card_id = ? AND (?2 IS NULL OR observed_on >= ?2) AND (?3 IS NULL OR observed_on <= ?3)
         ORDER BY observed_on",
        card_id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let rates = exchange_rates::load(db).await?;
    let prices = rows
        .into_iter()
        .map(|row| {
            let observed_on = CalendarDay::parse(&row.observed_on)
                .ok_or_else(|| sqlx::Error::Decode("stored Price Point is invalid".into()))?;
            let usd = Cents::from_whole_cents(row.price_usd_cents);
            Ok(match convert(usd, observed_on, &rates) {
                Ok(Converted {
                    eur,
                    dkk,
                    rate_date,
                    ..
                }) => PricePoint {
                    observed_on,
                    usd,
                    eur: Some(eur),
                    dkk: Some(dkk),
                    rate_date: Some(rate_date),
                },
                Err(_) => PricePoint {
                    observed_on,
                    usd,
                    eur: None,
                    dkk: None,
                    rate_date: None,
                },
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(PriceHistory {
        card_id: id.as_str().to_owned(),
        prices,
    })
}
