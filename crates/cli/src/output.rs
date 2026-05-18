use outpost_core::ops;
use outpost_core::ops::status::ConfigProblem;
use outpost_core::AheadBehind;

pub fn print_added(outpost: &outpost_core::Outpost) {
    println!("added {}", outpost.work_tree().display());
}

pub fn print_list(summaries: &[ops::list::OutpostSummary], verbose: bool) {
    for summary in summaries {
        let branch = summary
            .current_branch
            .as_ref()
            .map(|branch| branch.as_str())
            .unwrap_or("-");
        println!(
            "{}\t{}\t{}\t{}{}",
            summary.path.display(),
            branch,
            list_state(summary.state),
            format_ahead_behind(summary.ahead_behind),
            lock_suffix(summary.locked)
        );
        if verbose {
            if let Some(reason) = &summary.lock_reason {
                println!("  lock-reason: {reason}");
            }
        }
    }
}

pub fn print_status(report: &ops::status::StatusReport) {
    println!("outpost: {}", report.outpost_path.display());
    match &report.source_path {
        Some(path) => println!("source: {}", path.display()),
        None => println!("source: -"),
    }
    println!("source-present: {}", report.source_present);
    match &report.remote_name {
        Some(remote) => println!("remote: {}", remote.as_str()),
        None => println!("remote: -"),
    }
    match &report.current_branch {
        Some(branch) => println!("branch: {}", branch.as_str()),
        None => println!("branch: detached"),
    }
    println!(
        "outpost-state: {}",
        if report.outpost_dirty {
            "dirty"
        } else {
            "clean"
        }
    );
    println!(
        "outpost-vs-source: {}",
        format_ahead_behind(report.outpost_ahead_behind_source)
    );
    println!(
        "source-vs-upstream: {}",
        format_ahead_behind(report.source_ahead_behind_upstream)
    );

    if report.problems.is_empty() {
        println!("problems: none");
    } else {
        println!("problems:");
        for problem in &report.problems {
            println!("  - {}", format_problem(problem));
        }
    }
}

pub fn print_pull(report: &ops::pull::PullReport) {
    println!(
        "source: {}",
        if report.source_updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
    println!(
        "outpost: {}",
        if report.outpost_updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
}

pub fn print_source_pull(report: &ops::source::SourcePullReport) {
    println!(
        "source {}: {}",
        report.branch.as_str(),
        if report.updated {
            "updated"
        } else {
            "up-to-date"
        }
    );
}

pub fn print_merge(report: &ops::merge::MergeReport) {
    println!(
        "merged {}/{}",
        report.source_ref.remote.as_str(),
        report.source_ref.branch.as_str()
    );
}

pub fn print_rebase(report: &ops::rebase::RebaseReport) {
    println!(
        "rebased onto {}/{}",
        report.source_ref.remote.as_str(),
        report.source_ref.branch.as_str()
    );
}

pub fn print_push(report: &ops::push::PushReport) {
    println!(
        "outpost-to-source: {}",
        format_push_step(report.outpost_to_source)
    );
    println!(
        "source-to-origin: {}",
        format_push_step(report.source_to_origin)
    );
}

pub fn print_prune(report: &ops::prune::PruneReport, verbose: bool) {
    if report.dry_run {
        println!("dry-run: true");
    }
    println!("removed: {}", report.removed_entries.len());
    if verbose {
        for path in &report.removed_entries {
            println!("  {}", path.display());
        }
    }
    println!("source-missing: {}", report.orphaned_source_missing.len());
    println!("locked: {}", report.locked_entries.len());
}

fn list_state(state: ops::list::OutpostState) -> &'static str {
    match state {
        ops::list::OutpostState::Clean => "clean",
        ops::list::OutpostState::Dirty => "dirty",
        ops::list::OutpostState::Missing => "missing",
        ops::list::OutpostState::NotManaged => "not-managed",
    }
}

fn lock_suffix(locked: bool) -> &'static str {
    if locked {
        "\tlocked"
    } else {
        ""
    }
}

fn format_ahead_behind(value: Option<AheadBehind>) -> String {
    match value {
        Some(value) => format!("ahead {}, behind {}", value.ahead, value.behind),
        None => "-".to_owned(),
    }
}

fn format_problem(problem: &ConfigProblem) -> String {
    match problem {
        ConfigProblem::MissingSourceRepoConfig => "missing source repo config".to_owned(),
        ConfigProblem::SourceMissing(path) => format!("source missing: {}", path.display()),
        ConfigProblem::MissingRemoteNameConfig => "missing remote name config".to_owned(),
        ConfigProblem::LocalRemoteMismatch { configured, actual } => format!(
            "local remote mismatch: configured {}, actual {}",
            configured.display(),
            actual.display()
        ),
        ConfigProblem::NoUpstreamTracking { branch } => {
            format!("no upstream tracking for {}", branch.as_str())
        }
        ConfigProblem::NotInRegistry => "not in source registry".to_owned(),
        ConfigProblem::PushWouldFail { branch } => {
            format!("push would fail for {}", branch.as_str())
        }
    }
}

fn format_push_step(step: ops::push::StepResult) -> String {
    match step {
        ops::push::StepResult::Pushed { commits } => format!("pushed {commits} commit(s)"),
    }
}
