//! Harness names: a random word from `words.txt`, one per harness, never used twice in a
//! clone. A name that a harness once had stays retired after the harness is deleted.

use std::collections::BTreeSet;
use std::hash::{BuildHasher, Hasher};

/// The text of `words.txt`: one word per line, and lines that start with `#` are comments.
pub const WORDS_TEXT: &str = include_str!("words.txt");

/// The longest name `--name` accepts.
pub const NAME_MAX: usize = 32;

/// The words of a word list text, in file order.
pub fn parse_words(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The words tsk picks from.
pub fn words() -> Vec<&'static str> {
    parse_words(WORDS_TEXT)
}

/// A name a person gives with `--name`: lower case letters, digits and hyphens, starting with
/// a letter or a digit.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= NAME_MAX
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A random word that is not in `used`. `seed` picks among the candidates. When every word
/// is used, the lowest number that leaves a word free is appended to a word: `otter2`, and
/// `otter3` once every word has a 2.
pub fn pick_name(words: &[&str], used: &BTreeSet<String>, seed: u64) -> String {
    let free: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| !used.contains(*word))
        .collect();
    if !free.is_empty() {
        return free[(seed % free.len() as u64) as usize].to_string();
    }
    let mut number: u64 = 2;
    loop {
        let free: Vec<String> = words
            .iter()
            .map(|word| format!("{}{}", word, number))
            .filter(|name| !used.contains(name))
            .collect();
        if !free.is_empty() {
            return free[(seed % free.len() as u64) as usize].clone();
        }
        number += 1;
    }
}

/// A seed from the clock and the process ID.
pub fn seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(nanos);
    hasher.write_u32(std::process::id());
    hasher.finish()
}

/// The names in the text of a `harness-names` file.
pub fn parse_used(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every name in `names` that begins with `prefix`, or the one that equals it.
pub fn match_prefix<'a>(names: &'a [String], prefix: &str) -> Vec<&'a String> {
    if let Some(exact) = names.iter().find(|name| name.as_str() == prefix) {
        return vec![exact];
    }
    names
        .iter()
        .filter(|name| name.starts_with(prefix))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_list_loads_and_every_word_is_a_short_lower_case_word() {
        let words = words();
        assert_eq!(words.len(), 992);
        let distinct: std::collections::HashSet<&&str> = words.iter().collect();
        assert_eq!(distinct.len(), words.len());
        for word in &words {
            assert!(
                (3..=9).contains(&word.len()) && word.chars().all(|c| c.is_ascii_lowercase()),
                "bad word: {}",
                word
            );
        }
    }

    #[test]
    fn comments_and_blank_lines_are_not_words() {
        assert_eq!(parse_words("# a\n\nfox\n  owl \n"), vec!["fox", "owl"]);
    }

    #[test]
    fn a_name_is_lower_case_letters_digits_and_hyphens() {
        assert!(valid_name("otter"));
        assert!(valid_name("otter-2"));
        assert!(!valid_name(""));
        assert!(!valid_name("Otter"));
        assert!(!valid_name("-otter"));
        assert!(!valid_name("ot ter"));
        assert!(!valid_name("ot/ter"));
        assert!(!valid_name(&"a".repeat(NAME_MAX + 1)));
    }

    #[test]
    fn a_pick_is_a_word_that_is_not_used() {
        let words = ["fox", "owl", "elk"];
        let used: BTreeSet<String> = ["fox".to_string(), "owl".to_string()].into();
        for seed in 0..10 {
            assert_eq!(pick_name(&words, &used, seed), "elk");
        }
        assert_eq!(pick_name(&words, &BTreeSet::new(), 4), "owl");
    }

    #[test]
    fn when_every_word_is_used_the_lowest_number_that_is_free_is_appended() {
        let words = ["fox", "owl"];
        let mut used: BTreeSet<String> = ["fox".to_string(), "owl".to_string()].into();
        let name = pick_name(&words, &used, 7);
        assert!(name == "fox2" || name == "owl2", "{}", name);
        used.insert("fox2".to_string());
        assert_eq!(pick_name(&words, &used, 7), "owl2");
        used.insert("owl2".to_string());
        let name = pick_name(&words, &used, 3);
        assert!(name == "fox3" || name == "owl3", "{}", name);
    }

    #[test]
    fn a_prefix_picks_one_name_and_an_exact_name_wins() {
        let names: Vec<String> = ["otter", "otter2", "owl"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(match_prefix(&names, "otter"), vec!["otter"]);
        assert_eq!(match_prefix(&names, "ow"), vec!["owl"]);
        assert_eq!(match_prefix(&names, "ot").len(), 2);
        assert!(match_prefix(&names, "x").is_empty());
    }

    #[test]
    fn used_names_are_one_per_line() {
        assert_eq!(parse_used("a\n\n b \na\n").len(), 2);
    }
}
