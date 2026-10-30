//! Food name search over both languages.
//!
//! Danish names are written as comma-separated parts ("Kylling, bryst, rå") while people type
//! compounds ("kyllingebryst"), so a query word matches a name when it occurs in the name's words
//! or in the name with spaces removed, or, for words of five or more letters, when most of its
//! letter triples occur in the name. Exact and leading matches rank first.

use std::collections::HashSet;

use super::FoodGroup;

/// Query words of at least this many letters may match by letter triples.
const FUZZY_MIN_LETTERS: usize = 5;
/// The share of a query word's letter triples a name must contain to match by triples.
const FUZZY_MIN_SHARE: f64 = 0.6;

/// A Food's identity as search results show it.
#[derive(Clone, Debug)]
pub struct FoodIdentity {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub food_group: Option<FoodGroup>,
}

/// Every Food of one complete import, ready to search in memory.
pub struct SearchIndex {
    entries: Vec<Entry>,
}

struct Entry {
    food: FoodIdentity,
    names: [IndexedName; 2],
}

struct IndexedName {
    /// Lower-case words separated by single spaces.
    words: String,
    /// `words` without spaces, so "kylling bryst" also reads "kyllingbryst".
    compact: String,
    triples: HashSet<[char; 3]>,
}

impl IndexedName {
    fn new(name: &str) -> Self {
        let words = normalize(name);
        let compact: String = words.chars().filter(|c| *c != ' ').collect();
        let triples = triples(&compact).collect();
        Self {
            words,
            compact,
            triples,
        }
    }

    /// Lower is better; `None` when a query word does not match this name.
    fn score(&self, query: &str, words: &[&str]) -> Option<f64> {
        if self.words == query {
            return Some(-1.0);
        }
        let mut score = 0.0;
        for word in words {
            score += if self.words.starts_with(word) {
                0.0
            } else if self.words.contains(&format!(" {word}")) {
                1.0
            } else if self.words.contains(word) || self.compact.contains(word) {
                2.0
            } else {
                3.0 + (1.0 - self.fuzzy_share(word)?)
            };
        }
        Some(score)
    }

    fn fuzzy_share(&self, word: &str) -> Option<f64> {
        if word.chars().count() < FUZZY_MIN_LETTERS {
            return None;
        }
        let wanted: HashSet<[char; 3]> = triples(word).collect();
        let found = wanted
            .iter()
            .filter(|triple| self.triples.contains(*triple))
            .count();
        let share = found as f64 / wanted.len() as f64;
        (share >= FUZZY_MIN_SHARE).then_some(share)
    }
}

impl SearchIndex {
    #[must_use]
    pub fn new(foods: Vec<FoodIdentity>) -> Self {
        let entries = foods
            .into_iter()
            .map(|food| Entry {
                names: [
                    IndexedName::new(&food.name_da),
                    IndexedName::new(&food.name_en),
                ],
                food,
            })
            .collect();
        Self { entries }
    }

    /// The best-matching Foods, best first; empty when the query has no letters or digits.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&FoodIdentity> {
        let query = normalize(query);
        let words: Vec<&str> = query.split(' ').filter(|word| !word.is_empty()).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut matches: Vec<(f64, usize, &FoodIdentity)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                entry
                    .names
                    .iter()
                    .filter_map(|name| {
                        name.score(&query, &words)
                            .map(|score| (score, name.words.chars().count()))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
                    .map(|(score, length)| (score, length, &entry.food))
            })
            .collect();
        matches.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.food_id.cmp(&b.2.food_id))
        });
        matches
            .into_iter()
            .take(limit)
            .map(|(_, _, food)| food)
            .collect()
    }
}

/// Lower-cases and keeps letters and digits, turning everything else into single spaces.
fn normalize(text: &str) -> String {
    let mapped: String = text
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn triples(text: &str) -> impl Iterator<Item = [char; 3]> {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len().saturating_sub(2))
        .map(move |start| [chars[start], chars[start + 1], chars[start + 2]])
}
