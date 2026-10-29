pub fn with_final_period(text: &str) -> String {
    let text = text.trim();
    let ends_mid_sentence = text.chars().last().is_some_and(char::is_alphanumeric);
    if ends_mid_sentence { format!("{text}.") } else { text.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcript_that_stops_after_a_word_gets_a_period() {
        assert_eq!(with_final_period("And basic filtering"), "And basic filtering.");
        assert_eq!(with_final_period("show me 13 compared to 10"), "show me 13 compared to 10.");
        assert_eq!(with_final_period("Ich habe es gefunden "), "Ich habe es gefunden.");
    }

    #[test]
    fn a_transcript_that_already_ends_in_punctuation_is_left_alone() {
        assert_eq!(with_final_period("Is that necessary?"), "Is that necessary?");
        assert_eq!(with_final_period("Wow!"), "Wow!");
        assert_eq!(with_final_period("Done."), "Done.");
        assert_eq!(with_final_period("Maybe..."), "Maybe...");
        assert_eq!(with_final_period("Maybe…"), "Maybe…");
        assert_eq!(with_final_period("so I said \"hi\""), "so I said \"hi\"");
        assert_eq!(with_final_period("a list:"), "a list:");
        assert_eq!(with_final_period(""), "");
    }
}
