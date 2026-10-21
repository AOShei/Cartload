//! Error type for the crate.

use std::time::Duration;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A step of a flow failed. The flow keeps its position on that step so a later
    /// [`Flow::run`](crate::Flow::run) resumes there.
    #[error("step {} ({name}) failed: {source}", .index + 1)]
    Step {
        /// Zero-based position of the step in the flow.
        index: usize,
        /// The step's name.
        name: String,
        /// What went wrong.
        #[source]
        source: Box<Error>,
    },

    /// Something that was waited for did not happen in time. `hint` tells the user what to
    /// check.
    #[error("{what} did not happen within {} s; {hint}", .timeout.as_secs())]
    Timeout {
        /// What was being waited for.
        what: String,
        /// How long was waited.
        timeout: Duration,
        /// What to check or do about it.
        hint: String,
    },

    /// The console is not in the state the step needs.
    #[error("the console is in the wrong state: {0}")]
    WrongState(String),

    /// Something the flow needs is missing or unusable (no package, no stock kernel, ...).
    #[error("{0}")]
    Precondition(String),

    /// The console reported a failure while running a command.
    #[error("the console reported a failure: {0}")]
    Device(String),

    /// FEL (USB boot) error.
    #[error("FEL: {0}")]
    Fel(#[from] cartload_fel::Error),

    /// Discovery, SSH or device script error.
    #[error("console: {0}")]
    Console(#[from] cartload_console::Error),

    /// Kernel package error.
    #[error("hakchi.hmod: {0}")]
    Hmod(#[from] cartload_hmod::Error),

    /// Local file I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// The innermost error of a [`Error::Step`] chain.
    pub fn root(&self) -> &Error {
        match self {
            Error::Step { source, .. } => source.root(),
            other => other,
        }
    }

    /// Whether the root cause is a timeout.
    pub fn is_timeout(&self) -> bool {
        matches!(self.root(), Error::Timeout { .. })
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_error_shows_one_based_position_and_cause() {
        let error = Error::Step {
            index: 2,
            name: "upload".into(),
            source: Box::new(Error::Precondition("no package".into())),
        };
        assert_eq!(error.to_string(), "step 3 (upload) failed: no package");
        assert!(!error.is_timeout());
    }

    #[test]
    fn timeout_message_has_seconds_and_hint() {
        let error = Error::Step {
            index: 0,
            name: "wait".into(),
            source: Box::new(Error::Timeout {
                what: "the console reappearing on the USB network".into(),
                timeout: Duration::from_secs(60),
                hint: "check the USB cable".into(),
            }),
        };
        assert!(error.is_timeout());
        assert_eq!(
            error.to_string(),
            "step 1 (wait) failed: the console reappearing on the USB network did not happen \
             within 60 s; check the USB cable"
        );
    }
}
