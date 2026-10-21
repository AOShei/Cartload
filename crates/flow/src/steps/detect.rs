//! Identify the console and the state of its hakchi install.

use async_trait::async_trait;

use crate::{Context, Error, Outcome, Result, Step};

/// What [`DetectState`] requires the console to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Any hakchi kernel that answers.
    Any,
    /// The minimal memboot shell (nothing booted from NAND).
    MembootShell,
}

/// Runs the hakchi2 CE identification sequence (`hakchi ismdPartitioning`, the `cf_memboot`
/// check, `hakchi hwid`, then the versions, console type and storage when it is a full boot)
/// and records the result in [`Context::state`].
#[derive(Clone, Copy, Debug)]
pub struct DetectState {
    expect: Expect,
}

impl DetectState {
    /// Accepts any answering console.
    pub fn any() -> Self {
        Self {
            expect: Expect::Any,
        }
    }

    /// Fails unless the console is in the minimal memboot shell.
    pub fn expect_memboot_shell() -> Self {
        Self {
            expect: Expect::MembootShell,
        }
    }
}

#[async_trait]
impl Step<Context> for DetectState {
    fn name(&self) -> &str {
        "detect"
    }

    fn description(&self) -> &str {
        match self.expect {
            Expect::Any => "Identify the console and the state of its hakchi install",
            Expect::MembootShell => {
                "Identify the console and check that it is in the memboot shell"
            }
        }
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        ctx.refresh_state().await?;
        let description = ctx.state.describe();
        if !ctx.state.is_online() {
            return Err(Error::WrongState(format!(
                "{description}; wait for it to boot or rerun the memboot step"
            )));
        }
        ctx.reporter.info(format!("console: {description}"));
        if self.expect == Expect::MembootShell && !ctx.state.is_memboot_shell() {
            return Err(Error::WrongState(format!(
                "expected the minimal memboot shell but found {description}; rerun the \
                 command so the memboot step runs again"
            )));
        }
        Ok(Outcome::Done)
    }
}
