/// A sequence whose issued values are never zero or reused.
#[derive(Debug)]
pub struct Sequence(Option<u64>);

impl Default for Sequence {
    fn default() -> Self {
        Self(Some(1))
    }
}

impl Sequence {
    #[must_use]
    pub fn can_take(&self, count: usize) -> bool {
        if count == 0 {
            return true;
        }
        let Ok(offset) = u64::try_from(count - 1) else {
            return false;
        };
        self.0.and_then(|value| value.checked_add(offset)).is_some()
    }
    /// Starts above every previously issued value, or remains exhausted.
    #[must_use]
    pub const fn after(previous: u64) -> Self {
        Self(previous.checked_add(1))
    }

    /// Takes the next identity. Exhaustion is permanent.
    pub const fn take(&mut self) -> Option<u64> {
        let value = self.0;
        self.0 = match value {
            Some(value) => value.checked_add(1),
            None => None,
        };
        value
    }
}

#[cfg(test)]
mod tests {
    use super::Sequence;

    #[test]
    fn exhausted_sequence_never_reuses_an_issued_identity() {
        let mut ids = Sequence::after(u64::MAX - 1);
        assert_eq!(ids.take(), Some(u64::MAX));
        assert_eq!(ids.take(), None);
        assert_eq!(ids.take(), None);
    }
}
