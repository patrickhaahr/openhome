use serde::{Serialize, Serializer};

use super::training::CalendarDay;

const STALE_AFTER_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cents(i128);

impl Cents {
    #[must_use]
    pub fn from_dollars(dollars: f64) -> Option<Self> {
        (dollars.is_finite() && dollars >= 0.0).then(|| Self((dollars * 100.0).round() as i128))
    }

    #[must_use]
    pub fn from_whole_cents(cents: i64) -> Self {
        Self(i128::from(cents))
    }

    #[must_use]
    pub fn whole_cents(self) -> Option<i64> {
        i64::try_from(self.0).ok()
    }

    fn scaled(self, numerator: i128, denominator: i128) -> Self {
        let product = self.0 * numerator;
        let rounded = (product.abs() * 2 + denominator) / (denominator * 2);
        Self(rounded * product.signum())
    }
}

impl Serialize for Cents {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0 as f64 / 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate(i64);

impl Rate {
    const SCALE: i128 = 1_000_000;

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
        let digits = |part: &str| {
            (1..=6).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit())
        };
        if !digits(whole) || !digits(fraction) {
            return None;
        }
        let micros = format!("{whole}{fraction:0<6}").parse::<i64>().ok()?;
        Self::from_micros(micros)
    }

    #[must_use]
    pub const fn from_micros(micros: i64) -> Option<Self> {
        if micros > 0 { Some(Self(micros)) } else { None }
    }

    #[must_use]
    pub const fn micros(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyRates {
    pub date: CalendarDay,
    pub usd: Rate,
    pub dkk: Rate,
}

#[derive(Debug, Default)]
pub struct Rates(Vec<DailyRates>);

impl Rates {
    #[must_use]
    pub fn new(mut days: Vec<DailyRates>) -> Self {
        days.sort_by_key(|day| day.date);
        Self(days)
    }

    #[must_use]
    pub fn at_or_before(&self, day: CalendarDay) -> Option<&DailyRates> {
        let after = self.0.partition_point(|rates| rates.date <= day);
        after.checked_sub(1).map(|index| &self.0[index])
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Converted {
    pub usd: Cents,
    pub eur: Cents,
    pub dkk: Cents,
    pub rate_date: CalendarDay,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NoRates;

pub fn convert(usd: Cents, observed_on: CalendarDay, rates: &Rates) -> Result<Converted, NoRates> {
    let day = rates.at_or_before(observed_on).ok_or(NoRates)?;
    let per_eur = i128::from(day.usd.0);
    Ok(Converted {
        usd,
        eur: usd.scaled(Rate::SCALE, per_eur),
        dkk: usd.scaled(i128::from(day.dkk.0), per_eur),
        rate_date: day.date,
    })
}

#[must_use]
pub fn is_stale(observed_on: CalendarDay, today: CalendarDay) -> bool {
    observed_on.days_until(today) > STALE_AFTER_DAYS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardValue {
    pub usd: Cents,
    pub eur: Cents,
    pub dkk: Cents,
    pub observed_on: CalendarDay,
    pub rate_date: CalendarDay,
    pub stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unpriced {
    JapanesePrinting,
    NoCataloguePrice,
    NoExchangeRates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Valuation {
    Priced(CardValue),
    Unpriced(Unpriced),
}

#[derive(Serialize)]
struct Wire {
    card_value: Option<CardValue>,
    value_unavailable: Option<Unpriced>,
}

impl Serialize for Valuation {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Priced(value) => Wire {
                card_value: Some(value.clone()),
                value_unavailable: None,
            },
            Self::Unpriced(reason) => Wire {
                card_value: None,
                value_unavailable: Some(*reason),
            },
        }
        .serialize(serializer)
    }
}

#[derive(Debug)]
pub struct Valuer {
    rates: Rates,
    today: CalendarDay,
}

impl Valuer {
    #[must_use]
    pub const fn new(rates: Rates, today: CalendarDay) -> Self {
        Self { rates, today }
    }

    #[must_use]
    pub fn card(&self, price: Option<(f64, &str)>) -> Valuation {
        let Some((usd, observed_on)) = price
            .and_then(|(usd, day)| Some((Cents::from_dollars(usd)?, CalendarDay::parse(day)?)))
        else {
            return Valuation::Unpriced(Unpriced::NoCataloguePrice);
        };
        match convert(usd, observed_on, &self.rates) {
            Ok(Converted {
                usd,
                eur,
                dkk,
                rate_date,
            }) => Valuation::Priced(CardValue {
                usd,
                eur,
                dkk,
                observed_on,
                rate_date,
                stale: is_stale(observed_on, self.today),
            }),
            Err(NoRates) => Valuation::Unpriced(Unpriced::NoExchangeRates),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct BinderValue {
    pub usd: Cents,
    pub eur: Cents,
    pub dkk: Cents,
    pub unpriced_entries: usize,
    pub oldest_observed_on: Option<CalendarDay>,
}

impl BinderValue {
    pub fn total<'a>(entries: impl IntoIterator<Item = (i64, &'a Valuation)>) -> Self {
        entries
            .into_iter()
            .fold(Self::default(), |mut total, (quantity, valuation)| {
                match valuation {
                    Valuation::Priced(value) => {
                        let copies = i128::from(quantity);
                        total.usd = Cents(total.usd.0 + value.usd.0 * copies);
                        total.eur = Cents(total.eur.0 + value.eur.0 * copies);
                        total.dkk = Cents(total.dkk.0 + value.dkk.0 * copies);
                        total.oldest_observed_on = total
                            .oldest_observed_on
                            .map_or(Some(value.observed_on), |oldest| {
                                Some(oldest.min(value.observed_on))
                            });
                    }
                    Valuation::Unpriced(_) => total.unpriced_entries += 1,
                }
                total
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> CalendarDay {
        CalendarDay::parse(text).unwrap()
    }

    fn rate(text: &str) -> Rate {
        Rate::parse(text).unwrap()
    }

    fn rates(days: &[(&str, &str, &str)]) -> Rates {
        Rates::new(
            days.iter()
                .map(|(date, usd, dkk)| DailyRates {
                    date: day(date),
                    usd: rate(usd),
                    dkk: rate(dkk),
                })
                .collect(),
        )
    }

    fn cents(dollars: f64) -> Cents {
        Cents::from_dollars(dollars).unwrap()
    }

    #[test]
    fn a_price_uses_the_rates_of_its_own_day() {
        let rates = rates(&[
            ("2026-11-11", "1.1000", "7.4000"),
            ("2026-11-12", "1.1081", "7.4754"),
        ]);
        let value = convert(cents(2.25), day("2026-11-12"), &rates).unwrap();
        assert_eq!(value.rate_date, day("2026-11-12"));
        assert_eq!(
            (value.usd, value.eur, value.dkk),
            (cents(2.25), cents(2.03), cents(15.18))
        );
        let earlier = convert(cents(2.25), day("2026-11-11"), &rates).unwrap();
        assert_eq!(earlier.rate_date, day("2026-11-11"));
        assert_eq!((earlier.eur, earlier.dkk), (cents(2.05), cents(15.14)));
    }

    #[test]
    fn a_day_without_rates_falls_back_to_the_latest_earlier_day_and_never_a_later_one() {
        let rates = rates(&[
            ("2026-11-06", "1.1000", "7.4000"),
            ("2026-11-09", "1.2000", "7.5000"),
        ]);
        for (observed, used) in [
            ("2026-11-07", "2026-11-06"),
            ("2026-11-08", "2026-11-06"),
            ("2026-11-10", "2026-11-09"),
            ("2027-01-01", "2026-11-09"),
        ] {
            let converted = convert(cents(1.0), day(observed), &rates).unwrap();
            assert_eq!(converted.rate_date, day(used), "{observed}");
        }
        assert_eq!(convert(cents(1.0), day("2026-11-05"), &rates), Err(NoRates));
        assert_eq!(
            convert(cents(1.0), day("2026-11-06"), &Rates::default()),
            Err(NoRates)
        );
    }

    #[test]
    fn amounts_round_to_cents_half_away_from_zero_without_float_error() {
        let two = rates(&[("2026-11-12", "2", "3")]);
        for (price, eur, dkk) in [(0.01, 0.01, 0.02), (0.03, 0.02, 0.05), (0.02, 0.01, 0.03)] {
            let value = convert(cents(price), day("2026-11-12"), &two).unwrap();
            assert_eq!((value.eur, value.dkk), (cents(eur), cents(dkk)), "{price}");
        }
        let exact = rates(&[("2026-11-12", "1.1", "7.5")]);
        let value = convert(cents(1.10), day("2026-11-12"), &exact).unwrap();
        assert_eq!((value.eur, value.dkk), (cents(1.0), cents(7.5)));
        assert_eq!(cents(568.01), Cents(56801));
        assert_eq!(cents(0.29), Cents(29));
    }

    #[test]
    fn rates_are_plain_decimals_with_up_to_six_fractional_digits() {
        assert_eq!(rate("1.1081").micros(), 1_108_100);
        assert_eq!(rate("7").micros(), 7_000_000);
        assert_eq!(rate("0.000001").micros(), 1);
        for text in [
            "",
            ".5",
            "1.",
            "-1",
            "+1",
            "1e3",
            "0",
            "0.0",
            "1.0000001",
            "1234567",
            "1,5",
            " 1",
            "NaN",
        ] {
            assert_eq!(Rate::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_price_is_stale_only_after_thirty_whole_days() {
        let today = day("2026-11-13");
        assert!(!is_stale(day("2026-10-14"), today));
        assert!(is_stale(day("2026-10-13"), today));
        assert!(!is_stale(day("2026-11-14"), today));
    }

    #[test]
    fn a_valuation_says_why_it_has_no_card_value() {
        let valuer = Valuer::new(
            rates(&[
                ("2026-01-01", "1.1000", "7.4000"),
                ("2026-11-12", "1.1081", "7.4754"),
            ]),
            day("2026-11-13"),
        );
        assert_eq!(
            valuer.card(None),
            Valuation::Unpriced(Unpriced::NoCataloguePrice)
        );
        assert_eq!(
            valuer.card(Some((1.0, "2025-12-31"))),
            Valuation::Unpriced(Unpriced::NoExchangeRates)
        );
        let Valuation::Priced(value) = valuer.card(Some((1.0, "2026-09-01"))) else {
            panic!("priced")
        };
        assert!(value.stale);
        assert_eq!(value.rate_date, day("2026-01-01"));
    }

    #[test]
    fn a_binder_value_sums_quantities_of_priced_entries_and_counts_the_rest() {
        let valuer = Valuer::new(rates(&[("2026-01-01", "2", "4")]), day("2026-11-13"));
        let cheap = valuer.card(Some((1.0, "2026-11-12")));
        let old = valuer.card(Some((10.0, "2026-06-09")));
        let none = valuer.card(None);
        let japanese = Valuation::Unpriced(Unpriced::JapanesePrinting);
        let total = BinderValue::total([(3, &cheap), (2, &old), (5, &none), (1, &japanese)]);
        assert_eq!(
            total,
            BinderValue {
                usd: cents(23.0),
                eur: cents(11.5),
                dkk: cents(46.0),
                unpriced_entries: 2,
                oldest_observed_on: Some(day("2026-06-09")),
            }
        );
        assert_eq!(
            BinderValue::total([(1, &none)]),
            BinderValue {
                unpriced_entries: 1,
                ..BinderValue::default()
            }
        );
    }
}
