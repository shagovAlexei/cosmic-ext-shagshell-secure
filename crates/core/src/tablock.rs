//! Lock Session (SecureCRT): a tab locked with a password the user types; nothing reaches it
//! until the same password is typed again. The password lives in memory only, never written.
//! An empty one is a lock without a password: Unlock frees it at once (no typing by mistake).

/// Why a lock was not set.
#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
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
        if password != again {
            return Err(LockError::Mismatch);
        }
        Ok(Self {
            secret: password.to_string(),
        })
    }

    /// Locked without a password.
    pub fn open(&self) -> bool {
        self.secret.is_empty()
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
        let free = Lock::new("", "").unwrap();
        assert!(free.open() && free.opens(""));
        assert_eq!(Lock::new("a", "b").unwrap_err(), LockError::Mismatch);
        let l = Lock::new("s3cret", "s3cret").unwrap();
        assert!(l.opens("s3cret"));
        assert!(!l.opens("s3cre"));
        assert!(!l.opens("") && !l.open());
        assert_eq!(format!("{l:?}"), "Lock(<hidden>)");
    }
}
