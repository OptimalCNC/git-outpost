pub trait Reporter {
    fn step(&mut self, kind: StepKind, message: &str);

    fn warn(&mut self, message: &str);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    SourceFetch,
    SourcePush,
    OutpostFetch,
    OutpostPush,
    ConfigChange,
    Cleanup,
}
