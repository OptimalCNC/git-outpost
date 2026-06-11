use std::io::Write;

use outpost_core::{Reporter, StepKind};

#[derive(Default)]
pub struct StderrReporter {
    pending_analysis: bool,
}

impl StderrReporter {
    pub fn new() -> Self {
        Self::default()
    }

    fn finish_pending_analysis_line(&mut self) {
        if self.pending_analysis {
            eprintln!();
            self.pending_analysis = false;
        }
    }
}

impl Reporter for StderrReporter {
    fn step(&mut self, kind: StepKind, message: &str) {
        if kind == StepKind::Analysis && is_analysis_progress(message) {
            self.finish_pending_analysis_line();
            eprint!("{} {message}", label(kind));
            let _ = std::io::stderr().flush();
            self.pending_analysis = true;
            return;
        }

        if kind == StepKind::Analysis && self.pending_analysis {
            eprintln!(" ... {message}");
            self.pending_analysis = false;
            return;
        }

        self.finish_pending_analysis_line();
        eprintln!("{} {message}", label(kind));
    }

    fn warn(&mut self, message: &str) {
        self.finish_pending_analysis_line();
        eprintln!("warning: {message}");
    }
}

impl Drop for StderrReporter {
    fn drop(&mut self) {
        self.finish_pending_analysis_line();
    }
}

fn is_analysis_progress(message: &str) -> bool {
    message.starts_with("checking ")
        || message.starts_with("resolving ")
        || message.starts_with("comparing ")
        || message.starts_with("discovering ")
}

fn label(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Analysis => "analysis:",
        StepKind::SourceFetch => "source-fetch:",
        StepKind::SourcePush => "source-push:",
        StepKind::OutpostFetch => "outpost-fetch:",
        StepKind::OutpostPush => "outpost-push:",
        StepKind::ConfigChange => "config:",
        StepKind::Cleanup => "cleanup:",
    }
}
