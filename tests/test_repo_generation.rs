// integration tests for test repository generation

use deptrack::SeverityConfig;
use deptrack::utils::cargo_ops::CrateDependencyGraph;
use deptrack::utils::git_ops::{GitOps, GitRef};
use deptrack::utils::testing::{TestCrate, TestRepoBuilder, TestScenario, TestWorkspace};

#[test]
fn test_simple_chain_scenario() {
    let repo = TestScenario::SimpleChain.build().unwrap();
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    assert_eq!(graph.crates.len(), 3);
    assert!(!graph.has_cycles());

    // verify chain dependency
    let crate_a = graph.find_crate_by_name("crate_A").unwrap();
    let crate_b = graph.find_crate_by_name("crate_B").unwrap();
    let crate_c = graph.find_crate_by_name("crate_C").unwrap();

    let b_deps = graph.get_dependencies(&crate_b.id);
    assert_eq!(b_deps.len(), 1);
    assert_eq!(b_deps[0], &crate_a.id);

    let c_deps = graph.get_dependencies(&crate_c.id);
    assert_eq!(c_deps.len(), 1);
    assert_eq!(c_deps[0], &crate_b.id);
}

#[test]
fn test_cross_workspace_scenario() {
    let repo = TestScenario::CrossWorkspace.build().unwrap();
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    assert_eq!(graph.crates.len(), 4);

    // verify cross-workspace dependency
    let crate_c = graph.find_crate_by_name("crate_C").unwrap();
    let crate_b = graph.find_crate_by_name("crate_B").unwrap();

    assert_ne!(crate_c.id.workspace, crate_b.id.workspace);

    let c_deps = graph.get_dependencies(&crate_c.id);
    assert!(c_deps.contains(&&crate_b.id));
}

#[test]
fn test_complex_scenario() {
    let repo = TestScenario::Complex.build().unwrap();
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    assert_eq!(graph.crates.len(), 10);
    assert!(!graph.has_cycles());
}

#[test]
fn test_custom_repository_builder() {
    let repo = TestRepoBuilder::new()
        .workspace(
            TestWorkspace::new("backend")
                .crate_entry(TestCrate::new("database").version("1.0.0"))
                .crate_entry(
                    TestCrate::new("api")
                        .version("2.0.0")
                        .dependency("database"),
                ),
        )
        .build()
        .unwrap();

    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    assert_eq!(graph.crates.len(), 2);

    let database = graph.find_crate_by_name("database").unwrap();
    let api = graph.find_crate_by_name("api").unwrap();

    assert_eq!(database.version, "1.0.0");
    assert_eq!(api.version, "2.0.0");

    let api_deps = graph.get_dependencies(&api.id);
    assert_eq!(api_deps.len(), 1);
    assert_eq!(api_deps[0], &database.id);
}

#[test]
fn test_repository_modifications() {
    let repo = TestScenario::SimpleChain.build().unwrap();

    // create a branch
    repo.create_branch("feature").unwrap();
    assert_eq!(repo.current_branch().unwrap(), "feature");

    // modify a file
    repo.modify_file(
        "workspace",
        "crate_A",
        "src/lib.rs",
        "// modified content\n",
    )
    .unwrap();

    // update version
    repo.update_version("workspace", "crate_A", "0.2.0")
        .unwrap();

    // commit
    repo.stage_all().unwrap();
    repo.commit("Test changes").unwrap();

    // verify changes were committed
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();
    let crate_a = graph.find_crate_by_name("crate_A").unwrap();
    assert_eq!(crate_a.version, "0.2.0");
}

#[test]
fn test_change_impact_analysis_with_generated_repo() {
    let repo = TestScenario::Complex.build().unwrap();
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    // create feature branch and make changes
    repo.create_branch("feature").unwrap();
    repo.modify_file("workspace_1", "crate_D", "src/lib.rs", "// changed\n")
        .unwrap();
    repo.stage_all().unwrap();
    repo.commit("Modify crate_D").unwrap();

    // analyze changes
    let base_ref = GitRef::Branch("master".to_string());
    let current_ref = GitRef::Branch("feature".to_string());

    let impact = graph
        .analyze_git_changes(repo.path(), &base_ref, &current_ref)
        .unwrap();

    assert!(!impact.changed_files.is_empty());
    assert!(!impact.directly_affected_crates.is_empty());

    // crate_D was changed, so it should be directly affected
    let crate_d = graph.find_crate_by_name("crate_D").unwrap();
    assert!(impact.directly_affected_crates.contains(&crate_d.id));
}

#[test]
fn test_version_bump_analysis_with_generated_repo() {
    let repo = TestScenario::SimpleChain.build().unwrap();

    // make changes on feature branch
    repo.create_branch("feature").unwrap();

    // modify crate_A and bump version
    repo.modify_file("workspace", "crate_A", "src/lib.rs", "// modified\n")
        .unwrap();
    repo.update_version("workspace", "crate_A", "0.1.1")
        .unwrap();

    // also bump dependent crate_B
    repo.update_version("workspace", "crate_B", "0.1.1")
        .unwrap();

    repo.stage_all().unwrap();
    repo.commit("Update versions").unwrap();

    // rebuild graph to get new versions
    let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();

    // analyze changes
    let base_ref = GitRef::Branch("master".to_string());
    let current_ref = GitRef::Branch("feature".to_string());

    let impact = graph
        .analyze_git_changes(repo.path(), &base_ref, &current_ref)
        .unwrap();

    // only check version bumps if we have affected crates
    if !impact.all_affected_crates.is_empty() {
        let direct_severity = SeverityConfig::default_direct();
        let transitive_severity = SeverityConfig::default_transitive();
        let version_analysis = graph
            .analyze_version_bumps(
                repo.path(),
                &base_ref,
                &impact.all_affected_crates,
                &impact.directly_affected_crates,
                &direct_severity,
                &transitive_severity,
            )
            .unwrap();

        // verify version bumps were detected
        assert!(
            !version_analysis.crates_bumped.is_empty(),
            "Should detect version bumps"
        );
    }
}

#[test]
fn test_git_integration_with_generated_repo() {
    let repo = TestScenario::SimpleChain.build().unwrap();

    // verify git repository was created
    assert!(GitOps::is_repository_root(repo.path()).unwrap());

    let repo_info = GitOps::get_repository_info(repo.path()).unwrap();
    assert_eq!(repo_info.root_path, repo.path());
}
