//! The ECB's euro reference rates (ADR 0017): the 90-day file and the full history file. One
//! bounded GET each, with a timeout, a byte cap and no redirects or retries; nothing a file says
//! is trusted beyond its dates and the USD and DKK rates they must hold. Every failure is
//! [`Unavailable`], so upstream text never reaches a caller.
//!
//! Both files are XML: `<Cube><Cube time="YYYY-MM-DD"><Cube currency="USD" rate="1.1081"/>…`,
//! rates being units of the currency per euro for that working day, newest day first. The
//! 90-day file holds about 64 working days. The history file holds one per working day back to
//! 1999, about 8 MB on a single line, so both are parsed as a stream of events and only the two
//! kept rates of each day are retained.

use std::time::Duration;

use quick_xml::{Reader, XmlVersion, events::Event};
use reqwest::{Client, header};
use url::Url;

use super::card_value::{DailyRates, Rate};
use super::nutrition::open_food_facts::USER_AGENT;
use super::training::CalendarDay;

pub const RECENT_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist-90d.xml";
pub const HISTORY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.xml";

struct Limits {
    timeout: Duration,
    max_bytes: usize,
}

const RECENT: Limits = Limits {
    timeout: Duration::from_secs(5),
    max_bytes: 1024 * 1024,
};
const HISTORY: Limits = Limits {
    timeout: Duration::from_secs(60),
    max_bytes: 16 * 1024 * 1024,
};

/// Unreachable, slow, refused, oversized or malformed; the cause is only logged.
#[derive(Debug)]
pub struct Unavailable;

#[derive(Clone)]
pub struct Ecb {
    client: Client,
    recent: Url,
    history: Url,
}

impl Ecb {
    /// Use [`RECENT_URL`] and [`HISTORY_URL`]; explicit URLs also support isolated provider tests.
    pub fn new(recent: Url, history: Url) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            recent,
            history,
        })
    }

    /// Every working day of the last 90 days that the 90-day file quotes in both USD and DKK.
    pub async fn recent(&self) -> Result<Vec<DailyRates>, Unavailable> {
        self.days(&self.recent, &RECENT).await
    }

    /// Every working day the history file holds that quotes both USD and DKK.
    pub async fn history(&self) -> Result<Vec<DailyRates>, Unavailable> {
        self.days(&self.history, &HISTORY).await
    }

    async fn days(&self, url: &Url, limits: &Limits) -> Result<Vec<DailyRates>, Unavailable> {
        let body = self.fetch(url, limits).await?;
        parse_days(&body).map_err(|reason| unavailable(reason, &body.len()))
    }

    async fn fetch(&self, url: &Url, limits: &Limits) -> Result<Vec<u8>, Unavailable> {
        let mut response = self
            .client
            .get(url.clone())
            .timeout(limits.timeout)
            .send()
            .await
            .map_err(|error| unavailable("request failed", &error))?;
        if !response.status().is_success() {
            return Err(unavailable("refused", &response.status()));
        }
        if response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|length| length.to_str().ok()?.parse::<usize>().ok())
            .is_some_and(|length| length > limits.max_bytes)
        {
            return Err(unavailable("oversized body", &limits.max_bytes));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| unavailable("reading the body failed", &error))?
        {
            if body.len() + chunk.len() > limits.max_bytes {
                return Err(unavailable("oversized body", &limits.max_bytes));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}

fn unavailable(what: &str, detail: &dyn std::fmt::Debug) -> Unavailable {
    tracing::warn!(?detail, "ECB reference rates: {what}");
    Unavailable
}

/// A dated `Cube` as the file has it; very old days lack some currencies.
struct Day {
    date: CalendarDay,
    usd: Option<Rate>,
    dkk: Option<Rate>,
}

impl Day {
    fn complete(self) -> Option<DailyRates> {
        Some(DailyRates {
            date: self.date,
            usd: self.usd?,
            dkk: self.dkk?,
        })
    }
}

/// A day without USD or DKK is skipped; a file with no complete day is refused.
fn parse_days(body: &[u8]) -> Result<Vec<DailyRates>, &'static str> {
    let complete: Vec<_> = days(body)?.into_iter().filter_map(Day::complete).collect();
    if complete.is_empty() {
        return Err("no complete day of USD and DKK rates");
    }
    Ok(complete)
}

/// Strict on structure, silent about currencies it does not use: dated `Cube`s under the
/// envelope's own `Cube`, each holding USD and DKK at most once. A DTD is refused outright.
fn days(body: &[u8]) -> Result<Vec<Day>, &'static str> {
    let text = std::str::from_utf8(body).map_err(|_| "not UTF-8")?;
    let mut reader = Reader::from_str(text);
    let mut path: Vec<String> = Vec::new();
    let mut days: Vec<Day> = Vec::new();
    loop {
        let (element, opened) = match reader.read_event().map_err(|_| "not well-formed XML")? {
            Event::Eof if path.is_empty() => break,
            Event::Eof => return Err("an unclosed element"),
            Event::DocType(_) => return Err("a DTD"),
            Event::End(_) => {
                path.pop();
                continue;
            }
            Event::Start(element) => (element, true),
            Event::Empty(element) => (element, false),
            _ => continue,
        };
        let name = element.local_name().as_ref().to_owned();
        if name == "Cube" {
            let attribute = |key: &str| -> Result<Option<String>, &'static str> {
                for attr in element.attributes() {
                    let attr = attr.map_err(|_| "a malformed attribute")?;
                    if attr.key.local_name().as_ref() == key {
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_err(|_| "a malformed attribute")?;
                        return Ok(Some(value.into_owned()));
                    }
                }
                Ok(None)
            };
            let parents: Vec<&str> = path.iter().map(String::as_str).collect();
            match (parents.as_slice(), days.last_mut()) {
                (["Envelope", "Cube"], _) => {
                    let date = attribute("time")?.ok_or("a Cube without a time")?;
                    days.push(Day {
                        date: CalendarDay::parse(&date).ok_or("an unusable date")?,
                        usd: None,
                        dkk: None,
                    });
                }
                (["Envelope", "Cube", "Cube"], Some(day)) => {
                    let slot = match attribute("currency")?.as_deref() {
                        Some("USD") => Some(&mut day.usd),
                        Some("DKK") => Some(&mut day.dkk),
                        _ => None,
                    };
                    if let Some(slot) = slot {
                        if slot.is_some() {
                            return Err("a repeated currency");
                        }
                        let rate = attribute("rate")?.ok_or("a currency without a rate")?;
                        *slot = Some(Rate::parse(&rate).ok_or("an unusable rate")?);
                    }
                }
                _ => {}
            }
        }
        if opened {
            path.push(name);
        }
    }
    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENVELOPE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
  <gesmes:subject>Reference rates</gesmes:subject>
  <gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
  <Cube>
    <Cube time="2026-11-12">
      <Cube currency="USD" rate="1.1081"/>
      <Cube currency="JPY" rate="171.30"/>
      <Cube currency="DKK" rate="7.4754"/>
    </Cube>
  </Cube>
</gesmes:Envelope>"#;

    #[test]
    fn a_file_of_one_day_yields_its_date_and_the_two_rates() {
        let [rates] =
            <[DailyRates; 1]>::try_from(parse_days(ENVELOPE.as_bytes()).unwrap()).unwrap();
        assert_eq!(rates.date.to_string(), "2026-11-12");
        assert_eq!(rates.usd.micros(), 1_108_100);
        assert_eq!(rates.dkk.micros(), 7_475_400);
    }

    #[test]
    fn anything_malformed_or_incomplete_is_rejected() {
        let broken = [
            ENVELOPE.replace("7.4754", "seven"),
            ENVELOPE.replace("7.4754", "-7.4754"),
            ENVELOPE.replace("7.4754", "0"),
            ENVELOPE.replace(r#"<Cube currency="DKK" rate="7.4754"/>"#, ""),
            ENVELOPE.replace(r#"<Cube currency="USD" rate="1.1081"/>"#, ""),
            ENVELOPE.replace(
                r#"<Cube currency="JPY" rate="171.30"/>"#,
                r#"<Cube currency="USD" rate="1.2"/>"#,
            ),
            ENVELOPE.replace("2026-11-12", "2026-13-12"),
            ENVELOPE.replace("2026-11-12", "12/11/2026"),
            ENVELOPE.replace(r#" time="2026-11-12""#, ""),
            ENVELOPE.replace(" rate=\"1.1081\"", ""),
            ENVELOPE.replace("</gesmes:Envelope>", ""),
            ENVELOPE.replace("<gesmes:Envelope", "<!DOCTYPE x [<!ENTITY a \"b\">]><gesmes:Envelope"),
            r#"<gesmes:Envelope xmlns:gesmes="x"><Other><Cube time="2026-11-12"><Cube currency="USD" rate="1"/><Cube currency="DKK" rate="7"/></Cube></Other></gesmes:Envelope>"#.into(),
            String::new(),
            "{\"rates\":{}}".into(),
        ];
        for body in broken {
            assert!(parse_days(body.as_bytes()).is_err(), "{body}");
        }
        assert!(parse_days(b"\xff\xfe").is_err());
    }

    #[test]
    fn rates_of_currencies_it_does_not_use_may_be_odd() {
        let odd = ENVELOPE.replace("171.30", "n/a");
        assert!(parse_days(odd.as_bytes()).is_ok());
    }

    const HISTORY_FILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref"><gesmes:subject>Reference rates</gesmes:subject><gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender><Cube><Cube time="2026-11-12"><Cube currency="USD" rate="1.1081"/><Cube currency="DKK" rate="7.4754"/></Cube><Cube time="2026-11-11"><Cube currency="USD" rate="1.1000"/><Cube currency="DKK" rate="7.4700"/></Cube><Cube time="1999-01-04"><Cube currency="USD" rate="1.1789"/><Cube currency="DKK" rate="7.4501"/></Cube></Cube></gesmes:Envelope>"#;

    #[test]
    fn the_history_file_yields_every_day_it_holds() {
        let days = parse_days(HISTORY_FILE.as_bytes()).unwrap();
        let dates: Vec<_> = days.iter().map(|day| day.date.to_string()).collect();
        assert_eq!(dates, ["2026-11-12", "2026-11-11", "1999-01-04"]);
        assert_eq!(days[2].usd.micros(), 1_178_900);
        assert_eq!(days[2].dkk.micros(), 7_450_100);
    }

    #[test]
    fn a_history_day_without_usd_or_dkk_is_skipped() {
        let sparse = HISTORY_FILE
            .replace(r#"<Cube currency="DKK" rate="7.4700"/>"#, "")
            .replace(r#"<Cube currency="USD" rate="1.1789"/>"#, "");
        let days = parse_days(sparse.as_bytes()).unwrap();
        let dates: Vec<_> = days.iter().map(|day| day.date.to_string()).collect();
        assert_eq!(dates, ["2026-11-12"]);
    }

    #[test]
    fn a_malformed_history_file_is_rejected_outright() {
        let broken = [
            HISTORY_FILE.replace("7.4700", "free"),
            HISTORY_FILE.replace("1999-01-04", "1999-13-04"),
            HISTORY_FILE.replace(r#" time="1999-01-04""#, ""),
            HISTORY_FILE.replace(
                r#"<Cube currency="USD" rate="1.1000"/>"#,
                r#"<Cube currency="USD" rate="1.1000"/><Cube currency="USD" rate="1.1"/>"#,
            ),
            HISTORY_FILE.replace("</gesmes:Envelope>", ""),
            HISTORY_FILE.replace(
                "<gesmes:Envelope",
                "<!DOCTYPE x [<!ENTITY a \"b\">]><gesmes:Envelope",
            ),
            HISTORY_FILE.replace(
                r#"<Cube currency="USD" rate="1.1081"/>"#,
                r#"<Cube currency="USD"/>"#,
            ),
            HISTORY_FILE.replace("</Cube></Cube></gesmes", "</Cube></gesmes"),
            "<gesmes:Envelope xmlns:gesmes=\"x\"><Cube/></gesmes:Envelope>".into(),
            String::new(),
        ];
        for body in broken {
            assert!(parse_days(body.as_bytes()).is_err(), "{body}");
        }
    }
}
