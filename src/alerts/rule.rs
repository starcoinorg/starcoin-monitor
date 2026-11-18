// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! When a check result turns into a Telegram message.

use std::time::Duration;

/// Seconds since the Unix epoch.
pub type UnixSecs = u64;

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// How often the check runs.
    pub every: Duration,
    /// Bad results in a row before the first alert.
    pub after: u32,
    /// Alert again while the problem lasts.
    pub repeat: Duration,
}

/// One check result for one key. `problem` is `None` when healthy.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub key: String,
    pub problem: Option<String>,
    pub detail: String,
}

impl Finding {
    pub fn new(key: impl Into<String>, problem: Option<String>, detail: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            problem,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Alert,
    Recover,
}

/// Alert state of one key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Alarm {
    bad: u32,
    last_sent: Option<UnixSecs>,
}

impl Alarm {
    pub fn is_bad(&self) -> bool {
        self.bad > 0
    }

    pub fn step(self, bad: bool, now: UnixSecs, rule: &Rule) -> (Alarm, Option<Action>) {
        if !bad {
            let action = self.last_sent.map(|_| Action::Recover);
            return (Alarm::default(), action);
        }
        let count = self.bad + 1;
        if count < rule.after {
            return (Alarm { bad: count, ..self }, None);
        }
        let due = match self.last_sent {
            None => true,
            Some(at) => now.saturating_sub(at) >= rule.repeat.as_secs(),
        };
        if due {
            (
                Alarm {
                    bad: count,
                    last_sent: Some(now),
                },
                Some(Action::Alert),
            )
        } else {
            (Alarm { bad: count, ..self }, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(bad: &[bool], rule: Rule) -> Vec<Option<Action>> {
        let mut alarm = Alarm::default();
        bad.iter()
            .enumerate()
            .map(|(i, b)| {
                let (next, action) = alarm.step(*b, i as u64 * 60, &rule);
                alarm = next;
                action
            })
            .collect()
    }

    const R3: Rule = Rule {
        every: Duration::from_secs(60),
        after: 3,
        repeat: Duration::from_secs(1200),
    };

    #[test]
    fn alerts_after_consecutive_then_recovers() {
        use Action::*;
        assert_eq!(
            run(&[true, true, true, true, false], R3),
            vec![None, None, Some(Alert), None, Some(Recover)]
        );
    }

    #[test]
    fn a_good_result_resets_the_count() {
        assert_eq!(run(&[true, false, true, true, false], R3), vec![None; 5]);
    }

    #[test]
    fn repeats_while_bad() {
        let rule = Rule { after: 1, ..R3 };
        let alerts: Vec<usize> = run(&[true; 22], rule)
            .iter()
            .enumerate()
            .filter(|(_, a)| **a == Some(Action::Alert))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(alerts, vec![0, 20]);
    }

    #[test]
    fn no_recovery_without_an_alert() {
        assert_eq!(run(&[true, false], R3), vec![None, None]);
    }
}
