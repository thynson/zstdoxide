// Generated and checked by import_fast_frame_publication.py.
// Unchanged C v1.5.7 completed-frame observations; partial headers excluded.
#[derive(Clone, Copy, Debug)]
pub enum SourceRecipe {
    Empty, Uniform, PeriodicUniformPeriodic, RawThenCopy, Mixed, Tiny,
    LiteralTree, Window, RawCandidate { length: usize, destination: usize },
}
#[derive(Clone, Copy, Debug)]
pub struct FrameExtent { pub offset: usize, pub size: usize }
#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub name: &'static str, pub recipe: SourceRecipe, pub source_size: usize,
    pub frame: FrameExtent, pub whole_frame: Option<FrameExtent>,
}
pub const FRAME_BYTES: &[u8] = include_bytes!("frames.bin");
pub const CASES: &[Case] = &[
    Case {
        name: "empty",
        recipe: SourceRecipe::Empty,
        source_size: 0,
        frame: FrameExtent { offset: 0, size: 9 },
        whole_frame: None,
    },
    Case {
        name: "first_uniform",
        recipe: SourceRecipe::Uniform,
        source_size: 4096,
        frame: FrameExtent { offset: 9, size: 22 },
        whole_frame: Some(FrameExtent { offset: 31, size: 19 }),
    },
    Case {
        name: "empty_then_first_uniform",
        recipe: SourceRecipe::Uniform,
        source_size: 4096,
        frame: FrameExtent { offset: 50, size: 22 },
        whole_frame: None,
    },
    Case {
        name: "compressed_uniform_repeat",
        recipe: SourceRecipe::PeriodicUniformPeriodic,
        source_size: 12288,
        frame: FrameExtent { offset: 72, size: 298 },
        whole_frame: None,
    },
    Case {
        name: "raw_then_copy",
        recipe: SourceRecipe::RawThenCopy,
        source_size: 262144,
        frame: FrameExtent { offset: 370, size: 131100 },
        whole_frame: Some(FrameExtent { offset: 131470, size: 131097 }),
    },
    Case {
        name: "mixed_fallback",
        recipe: SourceRecipe::Mixed,
        source_size: 16384,
        frame: FrameExtent { offset: 262567, size: 4397 },
        whole_frame: None,
    },
    Case {
        name: "tiny_stale",
        recipe: SourceRecipe::Tiny,
        source_size: 8210,
        frame: FrameExtent { offset: 266964, size: 321 },
        whole_frame: None,
    },
    Case {
        name: "literal_tree",
        recipe: SourceRecipe::LiteralTree,
        source_size: 9216,
        frame: FrameExtent { offset: 267285, size: 4150 },
        whole_frame: None,
    },
    Case {
        name: "negative_literal_policy",
        recipe: SourceRecipe::LiteralTree,
        source_size: 9216,
        frame: FrameExtent { offset: 271435, size: 9235 },
        whole_frame: None,
    },
    Case {
        name: "window_expiry_17",
        recipe: SourceRecipe::Window,
        source_size: 393216,
        frame: FrameExtent { offset: 280670, size: 393238 },
        whole_frame: Some(FrameExtent { offset: 673908, size: 393235 }),
    },
    Case {
        name: "window_retained_19",
        recipe: SourceRecipe::Window,
        source_size: 393216,
        frame: FrameExtent { offset: 1067143, size: 262175 },
        whole_frame: Some(FrameExtent { offset: 1329318, size: 262172 }),
    },
    Case {
        name: "raw_candidate_k4_d128",
        recipe: SourceRecipe::RawCandidate { length: 4, destination: 128 },
        source_size: 9216,
        frame: FrameExtent { offset: 1591490, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k4_d192",
        recipe: SourceRecipe::RawCandidate { length: 4, destination: 192 },
        source_size: 9216,
        frame: FrameExtent { offset: 1592811, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k4_d256",
        recipe: SourceRecipe::RawCandidate { length: 4, destination: 256 },
        source_size: 9216,
        frame: FrameExtent { offset: 1594132, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k4_d320",
        recipe: SourceRecipe::RawCandidate { length: 4, destination: 320 },
        source_size: 9216,
        frame: FrameExtent { offset: 1595453, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k8_d128",
        recipe: SourceRecipe::RawCandidate { length: 8, destination: 128 },
        source_size: 9216,
        frame: FrameExtent { offset: 1596774, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k8_d192",
        recipe: SourceRecipe::RawCandidate { length: 8, destination: 192 },
        source_size: 9216,
        frame: FrameExtent { offset: 1598095, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k8_d256",
        recipe: SourceRecipe::RawCandidate { length: 8, destination: 256 },
        source_size: 9216,
        frame: FrameExtent { offset: 1599416, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k8_d320",
        recipe: SourceRecipe::RawCandidate { length: 8, destination: 320 },
        source_size: 9216,
        frame: FrameExtent { offset: 1600737, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k12_d128",
        recipe: SourceRecipe::RawCandidate { length: 12, destination: 128 },
        source_size: 9216,
        frame: FrameExtent { offset: 1602058, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k12_d192",
        recipe: SourceRecipe::RawCandidate { length: 12, destination: 192 },
        source_size: 9216,
        frame: FrameExtent { offset: 1603379, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k12_d256",
        recipe: SourceRecipe::RawCandidate { length: 12, destination: 256 },
        source_size: 9216,
        frame: FrameExtent { offset: 1604700, size: 1321 },
        whole_frame: None,
    },
    Case {
        name: "raw_candidate_k12_d320",
        recipe: SourceRecipe::RawCandidate { length: 12, destination: 320 },
        source_size: 9216,
        frame: FrameExtent { offset: 1606021, size: 1321 },
        whole_frame: None,
    },
];
