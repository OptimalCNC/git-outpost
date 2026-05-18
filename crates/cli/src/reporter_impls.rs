use outpost_core::{Reporter, StepKind};

pub struct StderrReporter;

impl Reporter for StderrReporter {
    fn step(&mut self, kind: StepKind, message: &str) {
        eprintln!("{} {message}", label(kind));
    }

    fn warn(&mut self, message: &str) {
        eprintln!("warning: {message}");
    }
}

fn label(kind: StepKind) -> &'static str {
    match kind {
        StepKind::SourceFetch => "source-fetch:",
        StepKind::SourcePush => "source-push:",
        StepKind::OutpostFetch => "outpost-fetch:",
        StepKind::OutpostPush => "outpost-push:",
        StepKind::ConfigChange => "config:",
        StepKind::Cleanup => "cleanup:",
    }
}
