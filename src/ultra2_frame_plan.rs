//! Allocation-free public boundary around the qualified private two-pass plan.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "Checked plan diagnostics share copied values")
)]
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::ultra2_plan::Ultra2ExecutionPlan;
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ultra2FramePlan {
    execution: Ultra2ExecutionPlan,
    settings: ReferenceFrameSettings,
    frame_bound: usize,
}
impl Ultra2FramePlan {
    pub(crate) fn new(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        let size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        if parameters.strategy() != ReferenceStrategy::Ultra2 {
            return Err(Error::UnsupportedFeature);
        }
        if !(64..=16384).contains(&source_len) {
            return Err(Error::ResourceLimit);
        }
        let execution = Ultra2ExecutionPlan::new(parameters, source_len, true, false)?;
        let settings = ReferenceFrameSettings::new(source_len, parameters.window_log())?;
        let frame_bound = ReferenceFrameHeader::raw_frame_bound(&settings)?;
        Layout::array::<u8>(frame_bound).map_err(|_| Error::ResourceLimit)?;
        Ok(Self {
            execution,
            settings,
            frame_bound,
        })
    }
    pub(crate) const fn execution(self) -> Ultra2ExecutionPlan {
        self.execution
    }
    pub(crate) const fn parameters(self) -> ResolvedCompressionParameters {
        self.execution.parameters()
    }
    pub(crate) const fn source_len(self) -> usize {
        self.execution.source_len()
    }
    pub(crate) const fn block_limit(self) -> usize {
        self.source_len()
    }
    pub(crate) const fn owner_bytes(self) -> [usize; 9] {
        self.execution.owner_bytes()
    }
    pub(crate) const fn frame_bound(self) -> usize {
        self.frame_bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: 3,
            target_length: 4095,
            strategy: 9,
        }
    }
    #[test]
    fn public_plan_enforces_full_source_and_forces_seed_and_automatic_literals() {
        for n in [64, 65, 2048, 4096, 16384] {
            let p = Ultra2FramePlan::new(raw().validate().unwrap(), n).unwrap();
            assert_eq!(p.source_len(), n);
            assert_eq!(p.block_limit(), n);
            assert!(p.execution().seeds().unwrap());
            assert!(!p.execution().disabled_literals());
            assert!(p.frame_bound() > n);
            assert_eq!(p.owner_bytes(), p.execution().owner_bytes());
        }
        for n in [0, 1, 7, 8, 9, 63, 16385, 131072] {
            assert_eq!(
                Ultra2FramePlan::new(raw().validate().unwrap(), n),
                Err(Error::ResourceLimit)
            );
        }
        for field in 0..6 {
            let mut bad = raw();
            match field {
                0 => bad.window_log = 17,
                1 => bad.hash_log = 13,
                2 => bad.chain_log = 13,
                3 => bad.search_log = 7,
                4 => bad.target_length = 4096,
                _ => bad.window_log = 13,
            }
            assert_eq!(
                Ultra2FramePlan::new(bad.validate().unwrap(), 16384),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in 1..9 {
            let mut bad = raw();
            bad.strategy = strategy;
            assert_eq!(
                Ultra2FramePlan::new(bad.validate().unwrap(), 64),
                Err(Error::UnsupportedFeature)
            );
        }
    }
}
