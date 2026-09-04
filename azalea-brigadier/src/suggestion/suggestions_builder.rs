use std::collections::HashSet;

use super::{Suggestion, SuggestionValue, Suggestions};
use crate::context::StringRange;

#[derive(Debug, PartialEq)]
pub struct SuggestionsBuilder {
    input: String,
    input_lowercase: String,
    start: usize,
    remaining: String,
    remaining_lowercase: String,
    result: HashSet<Suggestion>,
}

impl SuggestionsBuilder {
    pub fn new(input: &str, start: usize) -> Self {
        Self::new_with_lowercase(input, input.to_lowercase().as_str(), start)
    }

    pub fn new_with_lowercase(input: &str, input_lowercase: &str, start: usize) -> Self {
        // `start` indexes the original UTF-8 string. Lowercasing can change a
        // character's encoded width, so translate the prefix length before
        // using the offset on `input_lowercase`.
        let lowercase_start = input[..start]
            .chars()
            .flat_map(char::to_lowercase)
            .map(char::len_utf8)
            .sum::<usize>();

        Self {
            start,
            input: input.to_owned(),
            input_lowercase: input_lowercase.to_owned(),
            remaining: input[start..].to_owned(),
            remaining_lowercase: input_lowercase[lowercase_start..].to_owned(),
            result: HashSet::new(),
        }
    }
}

impl SuggestionsBuilder {
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn remaining(&self) -> &str {
        &self.remaining
    }

    pub fn remaining_lowercase(&self) -> &str {
        &self.remaining_lowercase
    }

    /// Treat a terminal run of ASCII separators as an empty token while
    /// matching suggestions. The original input and replacement range remain
    /// unchanged, so applying a suggestion removes only the extra separators.
    pub(crate) fn ignore_terminal_spaces(mut self) -> Self {
        if !self.remaining.is_empty() && self.remaining.chars().all(|character| character == ' ') {
            self.remaining.clear();
            self.remaining_lowercase.clear();
        }
        self
    }

    pub fn build(&self) -> Suggestions {
        Suggestions::create(&self.input, &self.result)
    }

    pub fn suggest(mut self, text: &str) -> Self {
        if text == self.remaining {
            return self;
        }
        self.result.insert(Suggestion {
            range: StringRange::between(self.start, self.input.len()),
            value: SuggestionValue::Text(text.to_owned()),
            tooltip: None,
        });
        self
    }

    pub fn suggest_with_tooltip(mut self, text: &str, tooltip: String) -> Self {
        if text == self.remaining {
            return self;
        }
        self.result.insert(Suggestion {
            range: StringRange::between(self.start, self.input.len()),
            value: SuggestionValue::Text(text.to_owned()),
            tooltip: Some(tooltip),
        });
        self
    }

    pub fn suggest_integer(mut self, value: i32) -> Self {
        self.result.insert(Suggestion {
            range: StringRange::between(self.start, self.input.len()),
            value: SuggestionValue::Integer(value),
            tooltip: None,
        });
        self
    }

    pub fn suggest_integer_with_tooltip(mut self, value: i32, tooltip: String) -> Self {
        self.result.insert(Suggestion {
            range: StringRange::between(self.start, self.input.len()),
            value: SuggestionValue::Integer(value),
            tooltip: Some(tooltip),
        });
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, other: SuggestionsBuilder) -> Self {
        self.result.extend(other.result);
        self
    }

    pub fn create_offset(&self, start: usize) -> SuggestionsBuilder {
        SuggestionsBuilder::new_with_lowercase(&self.input, &self.input_lowercase, start)
    }

    pub fn restart(&self) -> SuggestionsBuilder {
        self.create_offset(self.start)
    }
}
