use std::collections::HashSet;

use super::FoodGroup;

const FUZZY_MIN_LETTERS: usize = 5;
const FUZZY_MIN_SHARE: f64 = 0.6;

#[derive(Clone, Debug)]
pub struct FoodIdentity {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub food_group: Option<FoodGroup>,
}

pub struct SearchIndex {
    entries: Vec<Entry>,
}

struct Entry {
    food: FoodIdentity,
    names: [IndexedName; 2],
}

struct IndexedName {
    words: String,
    head: String,
    plain: bool,
    compact: String,
    triples: HashSet<[char; 3]>,
}

impl IndexedName {
    fn new(name: &str) -> Self {
        let words = normalize(name);
        let compact: String = words.chars().filter(|c| *c != ' ').collect();
        let triples = triples(&compact).collect();
        let head = normalize(name.split(',').next().unwrap_or_default());
        let plain = words
            .split(' ')
            .any(|word| matches!(word, "raw" | "rå" | "uspec"))
            && !words
                .split(' ')
                .any(|word| matches!(word, "canned" | "konserves" | "dried" | "tørret"));
        Self {
            words,
            head,
            plain,
            compact,
            triples,
        }
    }

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
        if self.head == query {
            score -= 0.5;
            if self.plain {
                score -= 0.1;
            }
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

    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&FoodIdentity> {
        let query = normalize(query);
        let query = match query.as_str() {
            "oatmeal" | "rolled oats" => "oats rolled".to_owned(),
            _ => query,
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyday_searches_lead_with_plain_foods_and_preserve_explicit_processed_queries() {
        let foods = [
            (1, "Tomatpure", "Tomato puree"),
            (2, "Tomatjuice", "Tomato juice"),
            (3, "Tomat, flået, konserves", "Tomato, peeled, canned"),
            (4, "Tomat, dansk, rå", "Tomato, Danish, ripe, raw"),
            (5, "Havregryn, uspec.", "Oats, rolled, average values"),
        ];
        let index = SearchIndex::new(
            foods
                .into_iter()
                .map(|(food_id, da, en)| FoodIdentity {
                    food_id,
                    name_da: da.into(),
                    name_en: en.into(),
                    food_group: None,
                })
                .collect(),
        );
        for (query, first) in [
            ("tomato", 4),
            ("tomat", 4),
            ("tomato juice", 2),
            ("tomato puree", 1),
            ("oatmeal", 5),
            ("rolled oats", 5),
        ] {
            assert_eq!(
                index.search(query, 5).first().map(|food| food.food_id),
                Some(first),
                "{query}"
            );
        }
    }
}
