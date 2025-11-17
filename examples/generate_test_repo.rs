// demonstrate test repository generation

use deptrack::utils::testing::{TestCrate, TestRepoBuilder, TestScenario, TestWorkspace};
use deptrack::{CrateDependencyGraph, GitRef, SeverityConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Test Repository Generation Demo ===\n");

    // example 1: use a predefined scenario
    println!("1. predefined scenario - complex multi-workspace:");
    println!("{}", "-".repeat(60));

    let repo = TestScenario::Complex.build()?;
    println!("created repository at: {}", repo.path().display());

    let graph = CrateDependencyGraph::build_from_repository(repo.path())?;
    println!("total crates: {}", graph.crates.len());
    println!("workspaces: {}", graph.workspaces.len());

    // demonstrate making changes
    repo.create_branch("feature")?;
    repo.modify_file(
        "workspace_1",
        "crate_D",
        "src/lib.rs",
        "// modified\npub fn new_function() {}\n",
    )?;
    repo.update_version("workspace_1", "crate_D", "0.1.1")?;
    repo.stage_all()?;
    repo.commit("Add new function to crate_D")?;

    println!("created branch 'feature' with changes");

    // analyze changes
    let base_ref = GitRef::Branch("master".to_string());
    let current_ref = GitRef::Branch("feature".to_string());

    let impact = graph.analyze_git_changes(repo.path(), &base_ref, &current_ref)?;
    println!("changed files: {}", impact.changed_files.len());
    println!("affected crates: {}", impact.all_affected_crates.len());

    // analyze version bumps
    let direct_severity = SeverityConfig::default_direct();
    let transitive_severity = SeverityConfig::default_transitive();
    let version_analysis = graph.analyze_version_bumps(
        repo.path(),
        &base_ref,
        &impact.all_affected_crates,
        &impact.directly_affected_crates,
        &direct_severity,
        &transitive_severity,
    )?;

    println!("crates bumped: {}", version_analysis.crates_bumped.len());
    println!(
        "crates needing bump: {}",
        version_analysis.crates_needing_bump.len()
    );

    println!("\n");

    // example 2: custom repository
    println!("2. custom repository:");
    println!("{}", "-".repeat(60));

    let custom_repo = TestRepoBuilder::new()
        .workspace(
            TestWorkspace::new("backend")
                .crate_entry(
                    TestCrate::new("api")
                        .version("1.0.0")
                        .dependency("database")
                        .file("src/lib.rs", "pub mod api;\npub use api::*;"),
                )
                .crate_entry(TestCrate::new("database").version("1.0.0")),
        )
        .workspace(TestWorkspace::new("frontend").crate_entry(
            TestCrate::new("ui").version("2.0.0").dependency("api"), // cross-workspace
        ))
        .build()?;

    println!(
        "created custom repository at: {}",
        custom_repo.path().display()
    );

    let custom_graph = CrateDependencyGraph::build_from_repository(custom_repo.path())?;
    println!("total crates: {}", custom_graph.crates.len());

    for (crate_id, crate_info) in &custom_graph.crates {
        println!(
            "  - {} v{} ({})",
            crate_info.id.name, crate_info.version, crate_info.id.workspace
        );

        let deps = custom_graph.get_dependencies(crate_id);
        if !deps.is_empty() {
            for dep_id in deps {
                if let Some(dep_info) = custom_graph.crates.get(dep_id) {
                    let is_cross_ws = dep_info.id.workspace != crate_info.id.workspace;
                    let marker = if is_cross_ws {
                        " (cross-workspace)"
                    } else {
                        ""
                    };
                    println!("      depends on: {}{}", dep_info.id.name, marker);
                }
            }
        }
    }

    println!("\n");

    // example 3: persistent repository at /tmp for debugging
    println!("3. persistent repository at /tmp (for debugging):");
    println!("{}", "-".repeat(60));

    let debug_repo = TestRepoBuilder::new_at_path()
        .workspace(
            TestWorkspace::new("core")
                .crate_entry(TestCrate::new("lib"))
                .crate_entry(TestCrate::new("app").dependency("lib")),
        )
        .build()?;

    println!("created at: {}", debug_repo.path().display());
    println!("this repository persists after the program exits");
    println!(
        "you can inspect it with: cd {}",
        debug_repo.path().display()
    );

    Ok(())
}
