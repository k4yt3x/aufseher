//! The pattern file, compiled, and matching text against it.

use std::{borrow::Cow, fmt, fs, path::Path};

use anyhow::{Context, Result};
use fancy_regex::Regex;
use serde::Deserialize;
use tracing::warn;

/// Characters stripped before the second, deobfuscated match attempt: everything but letters,
/// numbers, and punctuation, plus the Hangul fillers, which are letters that render blank. That
/// covers spaces of every width, invisible format characters, emoji and other symbols, and
/// combining marks, so decoration stacked on letters comes off too. Scripts that need combining
/// marks lose them as well, which is why text is first matched as written.
const OBFUSCATION_PATTERN: &str = r"[^\p{L}\p{N}\p{P}]|[\u{115F}\u{1160}\u{3164}\u{FFA0}]";

/// The pattern file as written. Other keys, such as the sample file's `tests`, are ignored.
#[derive(Deserialize)]
struct RuleFile {
    name_regexes: Vec<String>,
    message_regexes: Vec<String>,
}

/// The compiled patterns that mark a user as a spammer.
pub(crate) struct Rules {
    name_patterns: Vec<Regex>,
    message_patterns: Vec<Regex>,
    obfuscation: Regex,
}

impl fmt::Display for Rules {
    /// Counts the patterns, for the log.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} name and {} message patterns",
            self.name_patterns.len(),
            self.message_patterns.len()
        )
    }
}

/// The pattern that matched a piece of text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Match<'a> {
    /// The pattern as written in the pattern file.
    pub(crate) pattern: &'a str,
    /// Whether the text matched only once obfuscation was stripped.
    pub(crate) deobfuscated: bool,
}

impl Rules {
    /// Reads and compiles the pattern file at `path`.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let yaml = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_yaml(&yaml).with_context(|| format!("invalid pattern file {}", path.display()))
    }

    /// Compiles the patterns in the text of a pattern file.
    pub(crate) fn from_yaml(yaml: &str) -> Result<Self> {
        let file: RuleFile = serde_norway::from_str(yaml)?;
        Ok(Self {
            name_patterns: compile(&file.name_regexes, "name_regexes")?,
            message_patterns: compile(&file.message_regexes, "message_regexes")?,
            obfuscation: Regex::new(OBFUSCATION_PATTERN)?,
        })
    }

    /// Returns the first name pattern that matches `name`, retrying with obfuscation stripped when
    /// none matches it as written.
    pub(crate) fn match_name(&self, name: &str) -> Option<Match<'_>> {
        self.match_either_way(&self.name_patterns, name)
    }

    /// Returns the first message pattern that matches `text`, retrying with obfuscation stripped
    /// when none matches it as written.
    pub(crate) fn match_message(&self, text: &str) -> Option<Match<'_>> {
        self.match_either_way(&self.message_patterns, text)
    }

    /// Returns the first of `patterns` that matches `text` as written, or failing that, with
    /// obfuscation stripped.
    fn match_either_way<'a>(&self, patterns: &'a [Regex], text: &str) -> Option<Match<'a>> {
        if let Some(pattern) = first_match(patterns, text) {
            return Some(Match {
                pattern,
                deobfuscated: false,
            });
        }

        // Text with nothing to strip would only repeat the first attempt.
        let Cow::Owned(stripped) = self.deobfuscate(text)? else {
            return None;
        };
        first_match(patterns, &stripped).map(|pattern| Match {
            pattern,
            deobfuscated: true,
        })
    }

    /// Returns `text` without the characters in [`OBFUSCATION_PATTERN`], borrowed when it had none.
    /// A failure is logged and returns `None`, leaving only the match as written.
    fn deobfuscate<'t>(&self, text: &'t str) -> Option<Cow<'t, str>> {
        match self.obfuscation.try_replacen(text, 0, "") {
            Ok(stripped) => Some(stripped),
            Err(error) => {
                warn!(
                    "Failed to strip obfuscation, so only the text as written is matched: {error}"
                );
                None
            }
        }
    }
}

/// Compiles every pattern in one section of the pattern file.
fn compile(sources: &[String], section: &str) -> Result<Vec<Regex>> {
    sources
        .iter()
        .map(|source| {
            Regex::new(source).with_context(|| format!("invalid pattern {source:?} in {section}"))
        })
        .collect()
}

/// Returns the source of the first pattern in `patterns` that matches `text`. A pattern that fails
/// at run time, such as by exceeding the backtracking limit, is logged and counts as no match, so
/// that it cannot shield the text from the patterns after it.
fn first_match<'a>(patterns: &'a [Regex], text: &str) -> Option<&'a str> {
    patterns
        .iter()
        .find_map(|pattern| match pattern.is_match(text) {
            Ok(true) => Some(pattern.as_str()),
            Ok(false) => None,
            Err(error) => {
                warn!(
                    "Pattern {:?} failed and counts as no match: {error}",
                    pattern.as_str()
                );
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use serde::Deserialize;

    use super::Rules;

    const SAMPLE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/configs/aufseher.yaml");

    /// The sample pattern file's `tests` section, which the bot itself ignores.
    #[derive(Deserialize)]
    struct Samples {
        tests: SampleInputs,
    }

    #[derive(Deserialize)]
    struct SampleInputs {
        usernames: Vec<String>,
        messages: Vec<String>,
    }

    #[test]
    fn every_sample_matches_a_pattern_of_its_kind() {
        let rules = Rules::open(Path::new(SAMPLE_PATH)).unwrap();
        let yaml = fs::read_to_string(SAMPLE_PATH).unwrap();
        let samples = serde_norway::from_str::<Samples>(&yaml).unwrap().tests;
        for username in samples.usernames {
            assert!(
                rules.match_name(&username).is_some(),
                "sample username {username:?} matches no name pattern"
            );
        }
        for message in samples.messages {
            assert!(
                rules.match_message(&message).is_some(),
                "sample message {message:?} matches no message pattern"
            );
        }
    }

    #[test]
    fn deobfuscation_keeps_only_letters_digits_and_punctuation() {
        let rules = Rules::from_yaml("{name_regexes: [], message_regexes: []}").unwrap();
        let stripped = rules
            .deobfuscate("B\u{200B}u\u{00A0}y\u{2009}🚀 n\u{3164}o\n w ✅, 42!")
            .unwrap();
        assert_eq!(stripped, "Buynow,42!");
    }

    #[test]
    fn obfuscated_text_matches_patterns_of_its_own_kind_once_deobfuscated() {
        let rules =
            Rules::from_yaml("{name_regexes: ['(?i)spambot'], message_regexes: ['(?i)buynow']}")
                .unwrap();
        assert!(rules.match_name("Spam 🤖 Bot").unwrap().deobfuscated);
        assert!(rules.match_message("B u y 💰 N o w").unwrap().deobfuscated);
        assert!(rules.match_name("B u y 💰 N o w").is_none());
    }

    #[test]
    fn a_pattern_that_fails_at_run_time_does_not_shield_the_rest() {
        let rules =
            Rules::from_yaml(r"{name_regexes: [], message_regexes: ['(a|aa)*\1c', 'aaaa']}")
                .unwrap();
        let found = rules.match_message(&"a".repeat(40)).unwrap();
        assert_eq!(found.pattern, "aaaa");
    }

    #[test]
    fn an_invalid_pattern_is_named_in_the_error() {
        let Err(error) = Rules::from_yaml("{name_regexes: ['(unclosed'], message_regexes: []}")
        else {
            panic!("an unclosed group compiled");
        };
        let message = format!("{error:#}");
        assert!(
            message.contains(r#""(unclosed" in name_regexes"#),
            "{message}"
        );
    }
}
