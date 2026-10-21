//! Constructor-only retained Ultra scope; every provider remains separately gated.
use super::*;
use crate::optimal_search_layout::OptimalSearchLayout;
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use alloc::{alloc::Global, vec};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

struct NoProvider;
impl PathProvider for NoProvider {
    fn query<A: Allocator>(&mut self, _: OptimalQuery, _: &mut OptimalMatches<A>) -> Result<()> {
        panic!("constructor-only retained scope reached a provider")
    }
}
struct NoObserver;
impl PathObserver for NoObserver {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        _: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        panic!("constructor-only retained scope reached an observer")
    }
}
fn parser(
    source: &[u8],
    scope: Scope,
    policy: PricePolicy,
) -> OptimalParser<'_, Global, Global, Global> {
    let f = OptimalFrequencies::new_with_policy_in(policy, false, Global).unwrap();
    let p = OptimalPaths::new_in(Global).unwrap();
    let m = OptimalMatches::new_in(Global).unwrap();
    match scope {
        Scope::Scripted => OptimalParser::from_policy_parts(policy, source, 4, 0, f, p, m),
        Scope::UltraFresh => OptimalParser::from_ultra_fresh_parts(source, 4, 0, f, p, m),
        Scope::UltraRetained => OptimalParser::from_ultra_retained_parts(source, 4, 0, f, p, m),
        Scope::Ultra2 => OptimalParser::from_ultra2_parts(source, 4, 0, f, p, m),
    }
    .unwrap()
}

#[test]
fn retained_scope_accepts_only_its_bounded_geometry_and_resets_without_allocation() {
    let source = vec![b'a'; 32768];
    for length in [128, 8192, 16384, 16385, 24577, 32768] {
        let owners: [_; 3] = core::array::from_fn(|_| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: None,
            })
            .unwrap()
        });
        let [f, p, m] = &owners;
        let mut parser = OptimalParser::from_ultra_retained_parts(
            &source[..length],
            3,
            4095,
            OptimalFrequencies::new_with_policy_in(PricePolicy::Ultra, true, f).unwrap(),
            OptimalPaths::new_in(p).unwrap(),
            OptimalMatches::new_in(m).unwrap(),
        )
        .unwrap();
        for _ in 0..3 {
            assert_eq!(
                parser.ultra_retained_configuration(),
                Ok((&source[..length], 3, 4095, true))
            );
            assert!(!parser.frequencies.snapshot().unwrap().initialized);
            parser.reset().unwrap();
        }
        for owner in &owners {
            let snapshot = owner.monitor().snapshot();
            assert_eq!(snapshot.attempts, 1);
            assert!(!snapshot.violations.any());
        }
        drop(parser);
        for owner in &owners {
            owner.monitor().assert_released();
        }
    }
}

#[test]
fn rejected_retained_scope_releases_nonclone_owners_without_extra_attempts() {
    let source = vec![b'a'; 32769];
    for (length, minimum, target, policy) in [
        (127, 4, 0, PricePolicy::Ultra),
        (32769, 4, 0, PricePolicy::Ultra),
        (128, 2, 0, PricePolicy::Ultra),
        (128, 8, 0, PricePolicy::Ultra),
        (128, 4, 4096, PricePolicy::Ultra),
        (128, 4, 0, PricePolicy::Optimal),
    ] {
        let owners: [_; 3] = core::array::from_fn(|_| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: None,
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let [f, p, m] = owners;
        let result = OptimalParser::from_ultra_retained_parts(
            &source[..length],
            minimum,
            target,
            OptimalFrequencies::new_with_policy_in(policy, false, f).unwrap(),
            OptimalPaths::new_in(p).unwrap(),
            OptimalMatches::new_in(m).unwrap(),
        );
        assert!(matches!(result, Err(Error::InvalidParameter)));
        for monitor in monitors {
            monitor.assert_released();
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.attempts, 1);
            assert_eq!(snapshot.owners_dropped, 1);
            assert!(!snapshot.violations.any());
        }
    }
}

#[test]
fn every_existing_entry_rejects_retained_scope_before_provider_or_state_mutation() {
    let source = [b'a'; 128];
    let mut p = parser(&source, Scope::UltraRetained, PricePolicy::Ultra);
    let layout = OptimalSearchLayout::new(10, 6, 6, 1, 4, 0, None).unwrap();
    let mut search = OptimalSearch::from_parts(
        &source,
        layout,
        ReferenceTree::new_in(layout.tree(), Global, Global).unwrap(),
        OptimalShortKey::new_in(None, Global).unwrap(),
        OptimalMatches::new_in(Global).unwrap(),
    )
    .unwrap();
    let initial = search.test_snapshot();
    for _ in 0..2 {
        assert_eq!(
            p.parse(&mut NoProvider, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            p.parse_fresh(&mut search, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            p.parse_ultra_fresh(&mut search, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            p.parse_ultra2(&mut search, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            p.parse_retained(0, 64, [1, 4, 8], &mut search, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(p.fresh_configuration(), Err(Error::UnsupportedFeature));
        assert_eq!(
            p.ultra_fresh_configuration(),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(p.retained_configuration(), Err(Error::UnsupportedFeature));
        assert_eq!(p.ultra2_configuration(), Err(Error::UnsupportedFeature));
        assert_eq!(p.status, Status::Ready);
        assert!(!p.frequencies.snapshot().unwrap().initialized);
        assert_eq!(search.test_snapshot(), initial);
        p.reset().unwrap();
    }
}

#[test]
fn earlier_constructor_scopes_cannot_claim_retained_ultra_ownership() {
    let source = [b'a'; 128];
    for (scope, policy) in [
        (Scope::Scripted, PricePolicy::Optimal),
        (Scope::Scripted, PricePolicy::Ultra),
        (Scope::UltraFresh, PricePolicy::Ultra),
        (Scope::Ultra2, PricePolicy::Ultra),
    ] {
        let mut p = parser(&source, scope, policy);
        for _ in 0..2 {
            assert_eq!(
                p.ultra_retained_configuration(),
                Err(Error::UnsupportedFeature)
            );
            assert_eq!(p.status, Status::Ready);
            p.reset().unwrap();
        }
    }
    let mut retained = parser(&source, Scope::UltraRetained, PricePolicy::Ultra);
    retained.status = Status::Poisoned;
    assert_eq!(
        retained.ultra_retained_configuration(),
        Err(Error::InvalidState)
    );
    retained.reset().unwrap();
    assert!(retained.ultra_retained_configuration().is_ok());
}
