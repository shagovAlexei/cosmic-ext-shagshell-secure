//! Lock Session (SecureCRT): a tab locked with a password the user types; nothing reaches it
//! until the same password is typed again. The password lives in memory only, never written.

/// Why a lock was not set.
#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    Empty,
    /// The two typings differ.
    Mismatch,
}

pub struct Lock {
    secret: String,
}

impl std::fmt::Debug for Lock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Lock(<hidden>)")
    }
}

impl Lock {
    pub fn new(password: &str, again: &str) -> Result<Self, LockError> {
        if password.is_empty() {
            return Err(LockError::Empty);
        }
        if password != again {
            return Err(LockError::Mismatch);
        }
        Ok(Self {
            secret: password.to_string(),
        })
    }

    pub fn opens(&self, password: &str) -> bool {
        self.secret == password
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_and_opens_with_the_same_password() {
        assert_eq!(Lock::new("", "").unwrap_err(), LockError::Empty);
        assert_eq!(Lock::new("a", "b").unwrap_err(), LockError::Mismatch);
        let l = Lock::new("s3cret", "s3cret").unwrap();
        assert!(l.opens("s3cret"));
        assert!(!l.opens("s3cre"));
        assert!(!l.opens(""));
        assert_eq!(format!("{l:?}"), "Lock(<hidden>)");
    }
}
