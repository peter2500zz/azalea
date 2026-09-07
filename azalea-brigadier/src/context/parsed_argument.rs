use std::sync::Arc;

use super::string_range::StringRange;
use crate::arguments::ParsedValue;

#[derive(Clone)]
pub struct ParsedArgument {
    pub range: StringRange,
    pub result: Arc<ParsedValue>,
}
