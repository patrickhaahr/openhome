//! The ECB's daily euro reference rates (ADR 0017). One bounded GET with a short timeout, a byte
//! cap and no redirects or retries; nothing the file says is trusted beyond the one date and the
//! USD and DKK rates it must hold. Every failure is [`Unavailable`], so upstream text never
//! reaches a caller.
//!
//! The file is XML: `<Cube><Cube time="YYYY-MM-DD"><Cube currency="USD" rate="1.1081"/>…`, rates
//! being units of the currency per euro for that working day.

use std::time::Duration;

use quick_xml::{Reader, XmlVersion, events::Event};
use reqwest::{Client, header};
use url::Url;

use super::card_value::{DailyRates, Rate};
use super::nutrition::open_food_facts::USER_AGENT;
use super::training::CalendarDay;

pub const PRODUCTION_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";
const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BYTES: usize = 64 * 1024;

/// Unreachable, slow, refused, oversized or malformed; the cause is only logged.
#[derive(Debug)]
pub struct Unavailable;

#[derive(Clone)]
pub struct Ecb {
    client: Client,
    url: Url,
}

impl Ecb {
    /// Use [`PRODUCTION_URL`]; an explicit URL also supports isolated provider tests.
    pub fn new(url: Url) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, url })
    }

    /// The reference rates of the file's working day.
    pub async fn daily(&self) -> Result<DailyRates, Unavailable> {
        let mut response = self
            .client
            .get(self.url.clone())
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
            .is_some_and(|length| length > MAX_BYTES)
        {
            return Err(unavailable("oversized body", &MAX_BYTES));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| unavailable("reading the body failed", &error))?
        {
            if body.len() + chunk.len() > MAX_BYTES {
                return Err(unavailable("oversized body", &MAX_BYTES));
            }
            body.extend_from_slice(&chunk);
        }
        parse(&body).map_err(|reason| unavailable(reason, &body.len()))
    }
}

fn unavailable(what: &str, detail: &dyn std::fmt::Debug) -> Unavailable {
    tracing::warn!(?detail, "ECB reference rates: {what}");
    Unavailable
}

#[derive(Default)]
struct Found {
    date: Option<CalendarDay>,
    usd: Option<Rate>,
    dkk: Option<Rate>,
}

/// Strict on structure, silent about currencies it does not use: one dated `Cube` under the
/// envelope's own `Cube`, holding USD and DKK once each. A DTD is refused outright.
fn parse(body: &[u8]) -> Result<DailyRates, &'static str> {
    let text = std::str::from_utf8(body).map_err(|_| "not UTF-8")?;
    let mut reader = Reader::from_str(text);
    let mut path: Vec<String> = Vec::new();
    let mut found = Found::default();
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
            match parents.as_slice() {
                ["Envelope", "Cube"] => {
                    let date = attribute("time")?.ok_or("a Cube without a time")?;
                    let date = CalendarDay::parse(&date).ok_or("an unusable date")?;
                    if found.date.replace(date).is_some() {
                        return Err("more than one dated Cube");
                    }
                }
                ["Envelope", "Cube", "Cube"] => {
                    let slot = match attribute("currency")?.as_deref() {
                        Some("USD") => Some(&mut found.usd),
                        Some("DKK") => Some(&mut found.dkk),
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
    match found {
        Found {
            date: Some(date),
            usd: Some(usd),
            dkk: Some(dkk),
        } => Ok(DailyRates { date, usd, dkk }),
        _ => Err("no complete day of USD and DKK rates"),
    }
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
    fn the_published_file_yields_its_day_and_the_two_rates() {
        let rates = parse(ENVELOPE.as_bytes()).unwrap();
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
            ENVELOPE.replace("</Cube>\n  </Cube>", "</Cube><Cube time=\"2026-11-11\"/>\n  </Cube>"),
            ENVELOPE.replace("</gesmes:Envelope>", ""),
            ENVELOPE.replace("<gesmes:Envelope", "<!DOCTYPE x [<!ENTITY a \"b\">]><gesmes:Envelope"),
            r#"<gesmes:Envelope xmlns:gesmes="x"><Other><Cube time="2026-11-12"><Cube currency="USD" rate="1"/><Cube currency="DKK" rate="7"/></Cube></Other></gesmes:Envelope>"#.into(),
            String::new(),
            "{\"rates\":{}}".into(),
        ];
        for body in broken {
            assert!(parse(body.as_bytes()).is_err(), "{body}");
        }
        assert!(parse(b"\xff\xfe").is_err());
    }

    #[test]
    fn rates_of_currencies_it_does_not_use_may_be_odd() {
        let odd = ENVELOPE.replace("171.30", "n/a");
        assert!(parse(odd.as_bytes()).is_ok());
    }
}
