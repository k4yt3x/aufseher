use fancy_regex::Regex;

pub fn is_match(input: &str, regexes: &Vec<Regex>) -> Result<Option<Regex>, fancy_regex::Error> {
    for regex in regexes {
        if regex.is_match(input)? {
            return Ok(Some(regex.clone()));
        }
    }
    return Ok(None);
}

pub fn is_match_obfuscated(
    input: &str,
    regexes: &Vec<Regex>,
) -> Result<Option<Regex>, fancy_regex::Error> {
    // deobfuscate the message text
    let input = deobfuscate_message_text(input)?;

    for regex in regexes {
        if regex.is_match(&input)? {
            return Ok(Some(regex.clone()));
        }
    }
    return Ok(None);
}

fn deobfuscate_message_text(text: &str) -> Result<String, fancy_regex::Error> {
    // define patterns for spaces, invisible characters, and emojis
    let space_pattern = r"[ \n\t\u{00A0}\u{180E}\u{200B}\u{200C}\u{200D}\u{2060}\u{FEFF}]";
    let emoji_pattern = concat!(
        r"[\u{1F600}-\u{1F64F}]|[\u{1F300}-\u{1F5FF}]|",
        r"[\u{1F680}-\u{1F6FF}]|[\u{1F700}-\u{1F77F}]|",
        r"[\u{1F780}-\u{1F7FF}]|[\u{1F800}-\u{1F8FF}]|",
        r"[\u{1F900}-\u{1F9FF}]|[\u{1FA00}-\u{1FA6F}]|",
        r"[\u{1FA70}-\u{1FAFF}]|[\u{1FB00}-\u{1FBFF}]|",
        r"[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]|",
        r"[\u{2B50}-\u{2B55}]|[\u{1F1E6}-\u{1F1FF}]|",
        r"[\u{1F004}]|[\u{1F0CF}]|[\u{1F18E}]|",
        r"[\u{1F191}-\u{1F19A}]|[\u{1F1E6}-\u{1F1FF}]"
    );
    let non_text_pattern = r"[^\p{L}\p{N}\p{P}\p{Z}]";

    // create regex objects
    let space_re = Regex::new(space_pattern)?;
    let emoji_re = Regex::new(emoji_pattern)?;
    let non_text_re = Regex::new(non_text_pattern)?;

    // remove spaces, invisible characters, and emojis
    let text = space_re.replace_all(text, "");
    let text = emoji_re.replace_all(&text, "");
    let text = non_text_re.replace_all(&text, "");

    Ok(text.to_string())
}
