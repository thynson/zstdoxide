//! Call-level dictionary ownership, independent of incremental frame prefixes.
//!
//! Plain ownership records share one safe representation across backends.
//! Corresponds to the selection lifetime in pinned v1.5.7 `ZSTD_getDDict`.
#![forbid(unsafe_code)]

pub(crate) enum Selection<D> {
    None,
    Persistent(D),
    Prefix(D),
}

/// A persistent owner stays borrowed; a claimed prefix lives only in this call.
pub(crate) enum Claimed<'a, D> {
    None,
    Borrowed(&'a D),
    Owned(D),
}

impl<D> Claimed<'_, D> {
    pub(crate) fn retained(&self) -> Option<&D> {
        match self {
            Self::None => None,
            Self::Borrowed(value) => Some(value),
            Self::Owned(value) => Some(value),
        }
    }
}

impl<D> Selection<D> {
    pub(crate) fn retained(&self) -> Option<&D> {
        match self {
            Self::None => None,
            Self::Persistent(value) | Self::Prefix(value) => Some(value),
        }
    }

    pub(crate) fn prefix_pending(&self) -> bool {
        matches!(self, Self::Prefix(_))
    }

    /// Remove next-call ownership before any decoder or allocator operation.
    /// The returned record keeps that owner alive until the attempt ends.
    pub(crate) fn claim(&mut self) -> Claimed<'_, D> {
        if self.prefix_pending() {
            let Self::Prefix(value) = core::mem::replace(self, Self::None) else {
                unreachable!("exclusive selection remained a prefix until replacement")
            };
            return Claimed::Owned(value);
        }
        match self {
            Self::None => Claimed::None,
            Self::Persistent(value) => Claimed::Borrowed(value),
            Self::Prefix(_) => unreachable!("pending prefix was removed above"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;

    // No Clone implementation: ownership transitions cannot duplicate a handle.
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
    fn prefix_claim_moves_owner_until_call_record_is_dropped() {
        let drops = Rc::new(Cell::new(0));
        let mut selection = Selection::Prefix(owner(7, &drops));
        assert!(selection.prefix_pending());
        assert_eq!(selection.retained().unwrap().id, 7);
        {
            let call = selection.claim();
            assert_eq!(call.retained().unwrap().id, 7);
            assert_eq!(drops.get(), 0);
            // All frame visits in this call see the same immutable owner.
            assert!(core::ptr::eq(
                call.retained().unwrap(),
                call.retained().unwrap()
            ));
        }
        assert_eq!(drops.get(), 1);
        assert!(!selection.prefix_pending());
        assert!(selection.retained().is_none());
        assert!(selection.claim().retained().is_none());
        drop(selection);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn persistent_claims_borrow_the_same_nonclone_owner_until_replacement() {
        let drops = Rc::new(Cell::new(0));
        let mut selection = Selection::Persistent(owner(11, &drops));
        let original = selection.retained().unwrap() as *const Owner;
        for _ in 0..8 {
            let call = selection.claim();
            assert!(core::ptr::eq(original, call.retained().unwrap()));
            assert_eq!(drops.get(), 0);
        }
        assert!(!selection.prefix_pending());
        selection = Selection::None;
        assert_eq!(drops.get(), 1);
        assert!(selection.retained().is_none());
    }

    #[test]
    fn replacement_and_borrowed_claim_preserve_one_owner_lifetime() {
        let old = Rc::new(Cell::new(0));
        let next = Rc::new(Cell::new(0));
        let mut selection = Selection::Prefix(owner(1, &old));
        assert_eq!(selection.retained().unwrap().id, 1);
        selection = Selection::Persistent(owner(2, &next));
        assert_eq!(old.get(), 1);
        assert_eq!(selection.retained().unwrap().id, 2);
        drop(selection);
        assert_eq!(next.get(), 1);
        let drops = Rc::new(Cell::new(0));
        let value = owner(3, &drops);
        let mut borrowed = Selection::Prefix(&value);
        {
            let call = borrowed.claim();
            assert!(core::ptr::eq(*call.retained().unwrap(), &value));
        }
        assert!(borrowed.retained().is_none());
        assert_eq!(drops.get(), 0);
        drop(value);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn unwinding_an_attempt_drops_claimed_prefix_without_reinstalling_it() {
        let drops = Rc::new(Cell::new(0));
        let mut selection = Selection::Prefix(owner(4, &drops));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let call = selection.claim();
            assert_eq!(call.retained().unwrap().id, 4);
            panic!("simulated caller allocator panic");
        }));
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
        assert!(selection.retained().is_none());
        assert!(!selection.prefix_pending());
    }
}
