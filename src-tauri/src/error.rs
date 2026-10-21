//! Errors as the frontend sees them: one plain sentence, with the raw cause chain available
//! for a disclosure.

use serde::Serialize;

/// A failed command. Serialized as the rejection value of the `invoke` promise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommandError {
    /// What went wrong and, where the crates provide it, what to do about it.
    pub message: String,
    /// The underlying causes, one per line, when there are any.
    pub detail: Option<String>,
}

impl CommandError {
    /// An error with just a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
        }
    }

    /// Adds the raw detail.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Builds from any error: the message is its `Display`, the detail its source chain.
    pub fn from_error(error: &(dyn std::error::Error + 'static)) -> Self {
        let mut chain = Vec::new();
        let mut source = error.source();
        while let Some(cause) = source {
            chain.push(cause.to_string());
            source = cause.source();
        }
        Self {
            message: error.to_string(),
            detail: (!chain.is_empty()).then(|| chain.join("\ncaused by: ")),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<cartload_console::Error> for CommandError {
    fn from(error: cartload_console::Error) -> Self {
        Self::from_error(&error)
    }
}

impl From<cartload_fel::Error> for CommandError {
    fn from(error: cartload_fel::Error) -> Self {
        Self::from_error(&error)
    }
}

impl From<nusb::Error> for CommandError {
    fn from(error: nusb::Error) -> Self {
        Self::from_error(&error)
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(error: anyhow::Error) -> Self {
        let causes: Vec<String> = error.chain().skip(1).map(ToString::to_string).collect();
        Self {
            message: error.to_string(),
            detail: (!causes.is_empty()).then(|| causes.join("\ncaused by: ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_and_chain() {
        let io = std::io::Error::other("boom");
        let error: CommandError = cartload_console::Error::Io(io).into();
        assert_eq!(error.message, "I/O error: boom");
        assert_eq!(error.detail.as_deref(), Some("boom"));

        let nested = anyhow::anyhow!("inner").context("middle").context("outer");
        let error: CommandError = nested.into();
        assert_eq!(error.message, "outer");
        assert_eq!(error.detail.as_deref(), Some("middle\ncaused by: inner"));

        let plain = CommandError::new("nope").with_detail("raw");
        assert_eq!(plain.to_string(), "nope");
        assert_eq!(
            serde_json::to_value(&plain).expect("json"),
            serde_json::json!({"message": "nope", "detail": "raw"})
        );
    }
}
