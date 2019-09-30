use {
    std::time::Duration,
    chrono::prelude::*,
    serde::{
        Serialize,
        Serializer
    }
};

/// Formats a single line of text in info-beamer-text format.
pub(crate) fn render_line(line: impl ToString) -> Vec<String> {
    line.to_string().split(' ').map(ToString::to_string).collect()
}

/// Formats any number of lines of text in info-beamer-text format.
fn render_text(text: impl ToString) -> Vec<Vec<String>> {
    text.to_string().split('\n').map(render_line).collect()
}

#[derive(PartialEq, Eq)]
pub(crate) struct Ib<T>(pub(crate) T);

impl Serialize for Ib<DateTime<Utc>> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.timestamp().serialize(serializer)
    }
}

impl Serialize for Ib<Duration> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_secs_f64().serialize(serializer)
    }
}

impl Serialize for Ib<String> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        render_text(&self.0).serialize(serializer)
    }
}
