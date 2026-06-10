mod common;

#[test]
fn e_02_invocation_forms_produce_same_status_stdout() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let gop = common::run(fixture.gop().current_dir(&outpost).arg("status"));
    common::assert_success(&gop, "gop status");
    let git_outpost = common::run(fixture.git_outpost().current_dir(&outpost).arg("status"));
    common::assert_success(&git_outpost, "git-outpost status");
    let git_dispatch = common::run(fixture.git_dispatch().current_dir(&outpost).arg("status"));
    common::assert_success(&git_dispatch, "git outpost status");

    assert_eq!(common::stdout(&gop), common::stdout(&git_outpost));
    assert_eq!(common::stdout(&gop), common::stdout(&git_dispatch));
}

#[test]
fn e_04_basic_cli_lifecycle_round_trip_exits_zero() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.outpost("C");

    let add = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .arg("add")
            .arg("../C")
            .arg("main"),
    );
    common::assert_success(&add, "gop add");

    let status = common::run(fixture.gop().arg("-C").arg(&outpost).arg("status"));
    common::assert_success(&status, "gop status");

    let push = common::run(fixture.gop().arg("-C").arg(&outpost).arg("push"));
    common::assert_success(&push, "gop push");

    let list = common::run(fixture.gop().current_dir(&fixture.source).arg("list"));
    common::assert_success(&list, "gop list");

    let remove = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .arg("remove")
            .arg("../C"),
    );
    common::assert_success(&remove, "gop remove");
}

#[test]
fn remove_noninteractive_skips_branch_cleanup() {
    let fixture = common::CliFixture::new();
    let create_branch = common::run(fixture.git(&fixture.source).args(["branch", "feat"]));
    common::assert_success(&create_branch, "create feat");
    let outpost = fixture.outpost("C");
    let add = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "../C", "feat"]),
    );
    common::assert_success(&add, "gop add feat");

    let remove = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["remove", "../C"]),
    );
    common::assert_success(&remove, "gop remove noninteractive");

    assert_eq!(
        common::stdout(&remove),
        format!("removed {}\n", outpost.display())
    );
    let stderr = common::stderr(&remove);
    assert!(
        stderr.contains("branch-cleanup: skipped: non-interactive"),
        "remove stderr should explain non-interactive branch cleanup skip:\n{stderr}"
    );
    assert!(!outpost.exists(), "outpost directory should be removed");
    let branch = common::run(fixture.git(&fixture.source).args([
        "rev-parse",
        "--verify",
        "refs/heads/feat",
    ]));
    common::assert_success(&branch, "source branch should remain");
}

#[test]
fn remove_no_branch_cleanup_reports_disabled_cleanup() {
    let fixture = common::CliFixture::new();
    let create_branch = common::run(fixture.git(&fixture.source).args(["branch", "feat"]));
    common::assert_success(&create_branch, "create feat");
    let outpost = fixture.outpost("C");
    let add = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "../C", "feat"]),
    );
    common::assert_success(&add, "gop add feat");

    let remove = common::run(fixture.gop().current_dir(&fixture.source).args([
        "remove",
        "--no-branch-cleanup",
        "../C",
    ]));
    common::assert_success(&remove, "gop remove with cleanup disabled");

    assert_eq!(
        common::stdout(&remove),
        format!("removed {}\n", outpost.display())
    );
    let stderr = common::stderr(&remove);
    assert!(
        stderr.contains("branch-cleanup: skipped: cleanup disabled"),
        "remove stderr should explain disabled branch cleanup:\n{stderr}"
    );
    let branch = common::run(fixture.git(&fixture.source).args([
        "rev-parse",
        "--verify",
        "refs/heads/feat",
    ]));
    common::assert_success(&branch, "source branch should remain");
}

#[test]
fn dispatch_matrix_contextual_source_and_outpost_commands() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let source_list = common::run(fixture.gop().current_dir(&fixture.source).arg("list"));
    common::assert_success(&source_list, "source list");
    let outpost_list = common::run(fixture.gop().current_dir(&outpost).arg("list"));
    common::assert_success(&outpost_list, "outpost list");
    assert_eq!(common::stdout(&source_list), common::stdout(&outpost_list));

    let lock_from_outpost = common::run(
        fixture
            .gop()
            .current_dir(&outpost)
            .args(["lock", "--reason", "keep"]),
    );
    common::assert_success(&lock_from_outpost, "lock current outpost");
    let unlock_from_outpost = common::run(fixture.gop().current_dir(&outpost).arg("unlock"));
    common::assert_success(&unlock_from_outpost, "unlock current outpost");

    let lock_from_source = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["lock", "--reason", "source", "../C"]),
    );
    common::assert_success(&lock_from_source, "lock relative outpost from source");
    let unlock_from_source = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["unlock", "../C"]),
    );
    common::assert_success(&unlock_from_source, "unlock relative outpost from source");

    let move_out = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["move", "../C", "../D"]),
    );
    common::assert_success(&move_out, "move relative outpost from source");
    assert!(!outpost.exists(), "old outpost path should be moved");
    assert!(
        fixture.outpost("D").exists(),
        "new outpost path should exist"
    );

    let prune = common::run(fixture.gop().current_dir(&fixture.source).arg("prune"));
    common::assert_success(&prune, "source prune");
}

#[test]
fn dispatch_matrix_rejects_wrong_contexts() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");

    let add_from_outpost = common::run(
        fixture
            .gop()
            .current_dir(&outpost)
            .args(["add", "../D", "main"]),
    );
    common::assert_failure_code(&add_from_outpost, 2, "add from outpost");
    let stderr = common::stderr(&add_from_outpost);
    assert!(
        stderr.contains("add must be run from a source repository"),
        "wrong-context stderr should explain source-only command:\n{stderr}"
    );

    let pull_from_source = common::run(fixture.gop().current_dir(&fixture.source).arg("pull"));
    common::assert_failure_code(&pull_from_source, 2, "pull from source");
    let stderr = common::stderr(&pull_from_source);
    assert!(
        stderr.contains("pull must be run from a managed outpost"),
        "wrong-context stderr should explain outpost-only command:\n{stderr}"
    );

    let lock_without_path = common::run(fixture.gop().current_dir(&fixture.source).arg("lock"));
    common::assert_failure_code(&lock_without_path, 2, "lock without path from source");
    let stderr = common::stderr(&lock_without_path);
    assert!(
        stderr.contains("lock requires <outpost>"),
        "missing-path stderr should explain source lock usage:\n{stderr}"
    );
}

#[test]
fn e_05_push_makes_outpost_commit_visible_upstream() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    let commit = fixture.commit_file(&outpost, "outpost change", "feature.txt", "from C\n");

    let push = common::run(fixture.gop().current_dir(&outpost).arg("push"));
    common::assert_success(&push, "gop push");

    assert_eq!(
        fixture.git_capture(&fixture.upstream, ["rev-parse", "main"]),
        commit
    );
}

#[test]
fn e_06_two_outposts_round_trip_via_source() {
    let fixture = common::CliFixture::new();
    let c1 = fixture.add_outpost("C1");
    let c2 = fixture.add_outpost("C2");
    let commit = fixture.commit_file(&c1, "c1 change", "shared.txt", "from C1\n");

    let push = common::run(fixture.gop().current_dir(&c1).arg("push"));
    common::assert_success(&push, "gop push C1");

    let pull = common::run(fixture.gop().current_dir(&c2).arg("pull"));
    common::assert_success(&pull, "gop pull C2");

    assert_eq!(fixture.git_capture(&c2, ["rev-parse", "HEAD"]), commit);
    assert_eq!(
        std::fs::read_to_string(c2.join("shared.txt")).expect("read shared.txt"),
        "from C1\n"
    );
}

#[test]
fn e_07_copied_outpost_is_git_independent_when_source_is_missing() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.add_outpost("C");
    fixture.commit_file(&outpost, "copy seed", "seed.txt", "seed\n");

    let copy = fixture.outpost("C-copy");
    common::copy_dir_recursively(&outpost, &copy);
    std::fs::remove_dir_all(&fixture.source).expect("remove source repository");

    let git_status = common::run(fixture.git(&copy).arg("status"));
    common::assert_success(&git_status, "git status in copied outpost");
    let git_log = common::run(fixture.git(&copy).arg("log").arg("--oneline"));
    common::assert_success(&git_log, "git log in copied outpost");
    let git_diff = common::run(fixture.git(&copy).arg("diff").arg("HEAD~1"));
    common::assert_success(&git_diff, "git diff HEAD~1 in copied outpost");
    let git_checkout = common::run(fixture.git(&copy).args(["checkout", "-b", "new-branch"]));
    common::assert_success(&git_checkout, "git checkout -b in copied outpost");

    let status = common::run(fixture.gop().current_dir(&copy).arg("status"));
    common::assert_success(&status, "gop status in copied outpost");
    let stdout = common::stdout(&status);
    assert!(
        stdout.contains("source-present: false"),
        "status should report the missing source as absent:\n{stdout}"
    );
    assert!(
        stdout.contains("health: problems"),
        "status should report degraded health:\n{stdout}"
    );
    assert!(
        stdout.contains("source missing:"),
        "status should include the SourceMissing config problem:\n{stdout}"
    );
}

#[test]
fn e_10_story_flow_exits_zero() {
    let fixture = common::CliFixture::new();
    let outpost = fixture.outpost("C");

    let add = common::run(
        fixture
            .gop()
            .current_dir(&fixture.source)
            .args(["add", "-b", "feat"])
            .arg("../C")
            .arg("main"),
    );
    common::assert_success(&add, "gop add -b");
    fixture.commit_file(&outpost, "feat change", "feat.txt", "from feat\n");

    let source_pull = common::run(
        fixture
            .gop()
            .arg("-C")
            .arg(&outpost)
            .args(["source", "pull", "main"]),
    );
    common::assert_success(&source_pull, "gop source pull");

    let rebase = common::run(
        fixture
            .gop()
            .arg("-C")
            .arg(&outpost)
            .args(["rebase", "local/main"]),
    );
    common::assert_success(&rebase, "gop rebase local/main");

    let push = common::run(fixture.gop().arg("-C").arg(&outpost).arg("push"));
    common::assert_success(&push, "gop push");
}

#[test]
fn e_11_merge_and_rebase_accept_story_source_ref() {
    let merge_fixture = common::CliFixture::new();
    let merge_outpost = merge_fixture.add_outpost("C-merge");
    merge_fixture.commit_upstream_file("main", "upstream merge", "merge.txt", "for merge\n");
    let merge_source_pull = common::run(
        merge_fixture
            .gop()
            .current_dir(&merge_outpost)
            .args(["source", "pull", "main"]),
    );
    common::assert_success(&merge_source_pull, "gop source pull for merge");
    let merge = common::run(
        merge_fixture
            .gop()
            .current_dir(&merge_outpost)
            .args(["merge", "local/main"]),
    );
    common::assert_success(&merge, "gop merge local/main");

    let rebase_fixture = common::CliFixture::new();
    let rebase_outpost = rebase_fixture.add_outpost("C-rebase");
    rebase_fixture.commit_upstream_file("main", "upstream rebase", "rebase.txt", "for rebase\n");
    let rebase_source_pull = common::run(
        rebase_fixture
            .gop()
            .current_dir(&rebase_outpost)
            .args(["source", "pull", "main"]),
    );
    common::assert_success(&rebase_source_pull, "gop source pull for rebase");
    let rebase = common::run(
        rebase_fixture
            .gop()
            .current_dir(&rebase_outpost)
            .args(["rebase", "local/main"]),
    );
    common::assert_success(&rebase, "gop rebase local/main");
}
