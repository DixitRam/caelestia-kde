//! The installer's exit-code contract.
//!
//! The launcher reads these codes instead of scraping stderr, so every variant
//! here is part of the interface between the binary and `scripts/setup.sh`:
//! changing one means changing the launcher in the same commit.

/// How an installer run ended. The numeric code is the contract.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The requested action completed.
    Ok,
    /// A step failed, or the script the installer handed off to failed.
    Failed,
    /// The command line could not be understood.
    Usage,
    /// The user backed out. Nothing failed. Constructed by the wizard.
    #[allow(dead_code)]
    Cancelled,
    /// The installer cannot do its job where it was run: no bundle, no scripts.
    Environment,
    /// The installer itself failed in a way it does not model.
    Internal,
    /// Terminated by a signal. Constructed by the signal handler.
    #[allow(dead_code)]
    Interrupted,
}

impl Outcome {
    pub fn code(self) -> u8 {
        match self {
            Outcome::Ok => 0,
            Outcome::Failed => 1,
            Outcome::Usage => 2,
            Outcome::Cancelled => 3,
            Outcome::Environment => 4,
            Outcome::Internal => 5,
            // The shell's convention for a process killed by SIGINT.
            Outcome::Interrupted => 130,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Outcome;

    const ALL: [Outcome; 7] = [
        Outcome::Ok,
        Outcome::Failed,
        Outcome::Usage,
        Outcome::Cancelled,
        Outcome::Environment,
        Outcome::Internal,
        Outcome::Interrupted,
    ];

    #[test]
    fn codes_are_distinct() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.code(), b.code(), "{a:?} and {b:?} share an exit code");
            }
        }
    }

    #[test]
    fn success_is_zero_and_interrupt_is_the_shell_convention() {
        assert_eq!(Outcome::Ok.code(), 0);
        assert_eq!(Outcome::Interrupted.code(), 130);
    }
}
