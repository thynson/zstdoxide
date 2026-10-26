//! One-owner dictionary selection for stable-output sessions.
//!
//! This is a plain value record shared by all storage configurations. It owns
//! no byte allocation and performs no pointer access; dictionary and output
//! storage keep their existing independent access implementations.

/// Persistent selections are never cloned when a standard frame starts.
/// A pending prefix moves to active only at the caller's validated claim point.
pub(crate) enum Attachment<D> {
    None,
    Persistent(D),
    // Private integration precedes public prefix constructors.
    #[cfg_attr(not(test), allow(dead_code))]
    PendingPrefix(D),
    ActivePrefix(D),
}

impl<D> Attachment<D> {
    /// Payload reachable through any selection, including an unclaimed prefix.
    pub(crate) fn retained(&self) -> Option<&D> {
        match self {
            Self::None => None,
            Self::Persistent(value) | Self::PendingPrefix(value) | Self::ActivePrefix(value) => {
                Some(value)
            }
        }
    }

    /// Payload usable by a standard frame after the header's claim point.
    pub(crate) fn current(&self) -> Option<&D> {
        match self {
            Self::Persistent(value) | Self::ActivePrefix(value) => Some(value),
            Self::None | Self::PendingPrefix(_) => None,
        }
    }

    /// Move a pending prefix into the frame without cloning its owner.
    /// The caller invokes this after standard-header parsing and before ID and
    /// resource checks. Skippable frames do not invoke it.
    pub(crate) fn claim(&mut self) {
        *self = match core::mem::replace(self, Self::None) {
            Self::PendingPrefix(value) => Self::ActivePrefix(value),
            other => other,
        };
    }

    /// Complete or abandon a claimed frame. An unclaimed prefix is preserved.
    /// Ordinary Rust drop behavior applies to the released value.
    pub(crate) fn release_active(&mut self) {
        if matches!(self, Self::ActivePrefix(_)) {
            *self = Self::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Attachment;
    use std::cell::Cell;
    use std::rc::Rc;

    // Deliberately not Clone: transitions must move the same owner.
    struct Owner {
        id: usize,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    fn owner(id: usize, drops: &Rc<Cell<usize>>) -> Owner {
        Owner {
            id,
            drops: Rc::clone(drops),
        }
    }

    #[test]
    fn pending_claim_and_release_move_one_nonclone_owner_exactly_once() {
        let drops = Rc::new(Cell::new(0));
        let mut selection = Attachment::PendingPrefix(owner(7, &drops));
        assert!(selection.current().is_none());
        assert_eq!(selection.retained().unwrap().id, 7);
        // Skip completion or reset before the claim point preserves selection.
        selection.release_active();
        assert!(matches!(selection, Attachment::PendingPrefix(_)));
        assert_eq!(drops.get(), 0);
        selection.claim();
        assert!(matches!(selection, Attachment::ActivePrefix(_)));
        assert_eq!(selection.current().unwrap().id, 7);
        selection.claim();
        assert_eq!(selection.retained().unwrap().id, 7);
        assert_eq!(drops.get(), 0);
        selection.release_active();
        assert!(selection.current().is_none());
        assert!(selection.retained().is_none());
        assert_eq!(drops.get(), 1);
        selection.release_active();
        selection.claim();
        drop(selection);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn persistent_owner_survives_repeated_frame_claim_and_reset_until_drop() {
        let drops = Rc::new(Cell::new(0));
        let mut selection = Attachment::Persistent(owner(11, &drops));
        for _ in 0..8 {
            selection.claim();
            assert_eq!(selection.current().unwrap().id, 11);
            selection.release_active();
            assert_eq!(selection.retained().unwrap().id, 11);
            assert_eq!(drops.get(), 0);
        }
        drop(selection);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn replacement_and_session_drop_release_each_owned_or_borrowed_selection() {
        let old = Rc::new(Cell::new(0));
        let next = Rc::new(Cell::new(0));
        let mut selection = Attachment::PendingPrefix(owner(1, &old));
        assert_eq!(selection.retained().unwrap().id, 1);
        selection = Attachment::Persistent(owner(2, &next));
        assert_eq!(old.get(), 1);
        assert_eq!(selection.current().unwrap().id, 2);
        selection = Attachment::None;
        assert_eq!(next.get(), 1);
        assert!(selection.retained().is_none());

        let drops = Rc::new(Cell::new(0));
        let value = owner(3, &drops);
        let mut borrowed = Attachment::PendingPrefix(&value);
        borrowed.claim();
        assert!(core::ptr::eq(*borrowed.current().unwrap(), &value));
        borrowed.release_active();
        assert_eq!(drops.get(), 0);
        drop(value);
        assert_eq!(drops.get(), 1);

        let pending_drops = Rc::new(Cell::new(0));
        let pending = Attachment::PendingPrefix(owner(4, &pending_drops));
        drop(pending);
        assert_eq!(pending_drops.get(), 1);
        let active_drops = Rc::new(Cell::new(0));
        let mut active = Attachment::PendingPrefix(owner(5, &active_drops));
        active.claim();
        drop(active);
        assert_eq!(active_drops.get(), 1);
    }
}
