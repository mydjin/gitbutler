use anyhow::Result;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use gitbutler_stack::{PatchReferenceUpdate, Stack, TargetUpdate};
use itertools::Itertools;
use tempfile::TempDir;

#[test]
fn init_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    assert!(!branch.initialized());
    assert_eq!(branch.heads.len(), 0);
    let result = branch.init(&ctx);
    assert!(result.is_ok());
    assert!(branch.initialized());
    assert_eq!(branch.heads.len(), 1);
    assert_eq!(branch.heads[0].name, "virtual"); // matches the stack name
    assert_eq!(
        branch.heads[0].target,
        CommitOrChangeId::ChangeId(
            ctx.repository()
                .find_commit(branch.head)?
                .change_id()
                .unwrap()
        )
    );
    // Assert persisted
    assert_eq!(branch, test_ctx.handle.get_branch(branch.id)?);
    Ok(())
}

#[test]
fn init_already_initialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    let result = branch.init(&ctx);
    assert!(result.is_ok());
    let result = branch.init(&ctx);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn add_series_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads.len(), 2);
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn add_multiple_series() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["virtual"]); // defalts to stack name
    let default_head = test_ctx.branch.heads[0].clone();

    let head_4 = PatchReference {
        name: "head_4".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
    };
    let result = test_ctx
        .branch
        .add_series(&ctx, head_4, Some(default_head.clone()));
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["virtual", "head_4"]);

    let head_2 = PatchReference {
        name: "head_2".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
    };
    let result = test_ctx.branch.add_series(&ctx, head_2, None);
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["head_2", "virtual", "head_4"]);

    let head_1 = PatchReference {
        name: "head_1".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.first().unwrap().change_id().unwrap()),
    };

    let result = test_ctx.branch.add_series(&ctx, head_1, None);
    assert!(result.is_ok());
    assert_eq!(
        head_names(&test_ctx),
        vec!["head_1", "head_2", "virtual", "head_4"]
    );
    Ok(())
}

#[test]
fn add_series_commitid_when_changeid_available() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[1].id().to_string()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} has a change id associated with it. Use the change id instead",
            test_ctx.commits[1].id()
        )
    );
    Ok(())
}

#[test]
fn add_series_uninitialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack has not been initialized"
    );
    Ok(())
}

#[test]
fn add_series_invalid_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "name with spaces".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn add_series_duplicate_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert!(result.is_ok());
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "A patch reference with the name asdf exists"
    );
    Ok(())
}

#[test]
fn add_series_matching_git_ref_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "existing-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "A git reference with the name existing-branch exists"
    );
    Ok(())
}

#[test]
fn add_series_including_refs_head_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "refs/heads/my-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack head name cannot start with 'refs/heads'"
    );
    Ok(())
}

#[test]
fn add_series_target_commit_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId("30696678319e0fa3a20e54f22d47fc8cf1ceaade".into()), // does not exist
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("object not found"),);
    Ok(())
}

#[test]
fn add_series_target_change_id_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::ChangeId("does-not-exist".into()), // does not exist
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Commit with change id does-not-exist not found"
    );
    Ok(())
}

#[test]
fn add_series_target_commit_not_in_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let other_commit_id = test_ctx.other_commits.last().unwrap().id().to_string();
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId(other_commit_id.clone()), // does not exist
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} is not between the stack head and the stack base",
            other_commit_id
        )
    );
    Ok(())
}

#[test]
fn remove_series_uninitialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let result = test_ctx.branch.remove_series(&ctx, "some-name".to_string());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack has not been initialized"
    );
    Ok(())
}

#[test]
fn remove_series_last_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let result = test_ctx
        .branch
        .remove_series(&ctx, test_ctx.branch.heads[0].name.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Cannot remove the last branch from the stack"
    );
    Ok(())
}

#[test]
fn remove_series_nonexistent_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let result = test_ctx
        .branch
        .remove_series(&ctx, "does-not-exist".to_string());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Series with name does-not-exist not found"
    );
    Ok(())
}

#[test]
fn remove_series_with_multiple_last_heads() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["virtual"]); // defalts to stack name
    let default_head = test_ctx.branch.heads[0].clone();

    let to_stay = PatchReference {
        name: "to_stay".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
    };
    let result = test_ctx.branch.add_series(&ctx, to_stay.clone(), None);
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay", "virtual"]);

    let result = test_ctx
        .branch
        .remove_series(&ctx, default_head.name.clone());
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay"]);
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap())
    ); // it references the newest commit
    Ok(())
}

#[test]
fn remove_series_no_orphan_commits() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["virtual"]); // defalts to stack name
    let default_head = test_ctx.branch.heads[0].clone(); // references the newest commit

    let to_stay = PatchReference {
        name: "to_stay".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.first().unwrap().change_id().unwrap()),
    }; // references the oldest commit
    let result = test_ctx.branch.add_series(&ctx, to_stay.clone(), None);
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay", "virtual"]);

    let result = test_ctx
        .branch
        .remove_series(&ctx, default_head.name.clone());
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay"]);
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap())
    ); // it was updated to reference the newest commit
    Ok(())
}

#[test]
fn update_series_uninitialized_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let update = PatchReferenceUpdate {
        name: Some("foo".into()),
        target_update: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &update);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack has not been initialized"
    );
    Ok(())
}

#[test]
fn update_series_noop_does_nothing() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let heads_before = test_ctx.branch.heads.clone();
    let noop_update = PatchReferenceUpdate::default();
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &noop_update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads, heads_before);
    Ok(())
}

#[test]
fn update_series_name_fails_validation() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let update = PatchReferenceUpdate {
        name: Some("invalid name".into()),
        target_update: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &update);
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn update_series_name_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let update = PatchReferenceUpdate {
        name: Some("new-name".into()),
        target_update: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].name, "new-name");
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn update_series_target_fails_commit_not_in_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let other_commit_id = test_ctx.other_commits.last().unwrap().id().to_string();
    let update = PatchReferenceUpdate {
        name: None,
        target_update: Some(TargetUpdate {
            target: CommitOrChangeId::CommitId(other_commit_id.clone()),
            preceding_head: None,
        }),
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &update);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} is not between the stack head and the stack base",
            other_commit_id
        )
    );
    Ok(())
}

#[test]
fn update_series_target_orphan_commit_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let initial_state = test_ctx.branch.heads.clone();
    let first_commit_change_id = test_ctx.commits.first().unwrap().change_id().unwrap();
    let update = PatchReferenceUpdate {
        name: Some("new-lol".into()),
        target_update: Some(TargetUpdate {
            target: CommitOrChangeId::ChangeId(first_commit_change_id.clone()),
            preceding_head: None,
        }),
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "virtual".into(), &update);

    assert_eq!(
        result.err().unwrap().to_string(),
        "This update would cause orphaned patches, which is disallowed"
    );
    assert_eq!(initial_state, test_ctx.branch.heads); // no change due to failure
    Ok(())
}

#[test]
fn update_series_target_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.init(&ctx)?;
    let commit_0_change_id = CommitOrChangeId::ChangeId(test_ctx.commits[0].change_id().unwrap());
    let series_1 = PatchReference {
        name: "series_1".into(),
        target: commit_0_change_id.clone(),
    };
    let result = test_ctx.branch.add_series(&ctx, series_1, None);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].target, commit_0_change_id);
    let commit_1_change_id = CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap());
    let update = PatchReferenceUpdate {
        name: None,
        target_update: Some(TargetUpdate {
            target: commit_1_change_id.clone(),
            preceding_head: None,
        }),
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "series_1".into(), &update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].target, commit_1_change_id);
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

fn command_ctx(name: &str) -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("stacking.sh", name)
}

fn head_names(test_ctx: &TestContext) -> Vec<String> {
    test_ctx
        .branch
        .heads
        .iter()
        .map(|h| h.name.clone())
        .collect_vec()
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_branches()?;
    let branch = branches.iter().find(|b| b.name == "virtual").unwrap();
    let other_branch = branches.iter().find(|b| b.name != "virtual").unwrap();
    let target = handle.get_default_target()?;
    let mut branch_commits = ctx
        .repository()
        .log(branch.head, LogUntil::Commit(target.sha))?;
    branch_commits.reverse();
    let mut other_commits = ctx
        .repository()
        .log(other_branch.head, LogUntil::Commit(target.sha))?;
    other_commits.reverse();
    Ok(TestContext {
        branch: branch.clone(),
        commits: branch_commits,
        // other_branch: other_branch.clone(),
        other_commits,
        handle,
    })
}
struct TestContext<'a> {
    branch: gitbutler_branch::Branch,
    /// Oldest commit first
    commits: Vec<git2::Commit<'a>>,
    /// Oldest commit first
    #[allow(dead_code)]
    other_commits: Vec<git2::Commit<'a>>,
    handle: VirtualBranchesHandle,
}
