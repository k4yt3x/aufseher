#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use fancy_regex::Regex;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Tests {
        usernames: Vec<String>,
        messages: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct AufseherConfig {
        name_regexes: Vec<String>,
        message_regexes: Vec<String>,
        tests: Tests,
    }

    #[test]
    fn test_regexes() {
        let mut config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("configs/aufseher.yaml");
        let file_contents = fs::read_to_string(config_path).unwrap();
        let config: AufseherConfig = serde_yaml::from_str(&file_contents).unwrap();

        for s in config.tests.usernames {
            let mut matched = false;
            for pattern in &config.name_regexes {
                let re = Regex::new(pattern).unwrap();
                if re.is_match(&s).unwrap() {
                    matched = true;
                    println!("'{}' matched '{}'", s, pattern);
                    break;
                }
            }
            assert!(
                matched,
                "String '{}' did not match any of the provided patterns",
                s
            );
        }

        for s in config.tests.messages {
            let mut matched = false;
            for pattern in &config.message_regexes {
                let re = Regex::new(pattern).unwrap();
                if re.is_match(&s).unwrap() {
                    matched = true;
                    println!("'{}' matched '{}'", s, pattern);
                    break;
                }
            }
            assert!(
                matched,
                "String '{}' did not match any of the provided patterns",
                s
            );
        }
    }
}
