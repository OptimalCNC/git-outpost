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
