use fancy_regex::Regex;

pub fn is_match(input: &str, regexes: &Vec<Regex>) -> Result<Option<Regex>, fancy_regex::Error> {
    for regex in regexes {
        if regex.is_match(input)? {
            return Ok(Some(regex.clone()));
        }
    }
    return Ok(None);
}
