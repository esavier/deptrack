// check if affected crates have been version-bumped

use deptrack::{CrateDependencyGraph, GitRef, Result, SeverityConfig};

fn main() -> Result<()> {
    let repo_path = "test_repo";
    let base_branch = "master";

    println!("checking version bumps in repository: {}", repo_path);
    println!("{}", "=".repeat(70));

    // build the dependency graph
    let graph = CrateDependencyGraph::build_from_repository(repo_path)?;

    println!("\ntotal crates: {}", graph.crates.len());

    // analyze committed changes on current branch vs master
    println!("\n{}", "=".repeat(70));
    println!("analyzing changes: {} -> HEAD", base_branch);
    println!("{}", "=".repeat(70));

    let base_ref = GitRef::Branch(base_branch.to_string());
    let current_ref = GitRef::Branch("HEAD".to_string());

    let impact_analysis = graph.analyze_git_changes(repo_path, &base_ref, &current_ref)?;

    println!("\nchanged files: {}", impact_analysis.changed_files.len());
    for file in &impact_analysis.changed_files {
        println!("  - {}", file.display());
    }

    println!(
        "\ndirectly affected crates: {}",
        impact_analysis.directly_affected_crates.len()
    );
    for crate_id in &impact_analysis.directly_affected_crates {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            println!("  - {} ({})", crate_info.id.name, crate_info.id.workspace);
        }
    }

    println!(
        "\nall affected crates (including transitive): {}",
        impact_analysis.all_affected_crates.len()
    );

    // analyze version bumps
    println!("\n{}", "=".repeat(70));
    println!("VERSION BUMP ANALYSIS");
    println!("{}", "=".repeat(70));

    let direct_severity = SeverityConfig::default_direct();
    let transitive_severity = SeverityConfig::default_transitive();

    let version_analysis = graph.analyze_version_bumps(
        repo_path,
        &base_ref,
        &impact_analysis.all_affected_crates,
        &impact_analysis.directly_affected_crates,
        &direct_severity,
        &transitive_severity,
    )?;

    println!(
        "\ntotal affected crates: {}",
        version_analysis.crate_versions.len()
    );
    println!(
        "crates with version bumps: {}",
        version_analysis.crates_bumped.len()
    );
    println!(
        "crates needing version bumps: {}",
        version_analysis.crates_needing_bump.len()
    );
    println!(
        "bump percentage: {:.1}%",
        version_analysis.bump_percentage()
    );

    // show version details for all affected crates
    println!("\n{}", "-".repeat(70));
    println!("version details:");
    println!("{}", "-".repeat(70));

    for crate_id in &impact_analysis.all_affected_crates {
        if let Some(status) = version_analysis.crate_versions.get(crate_id)
            && let Some(crate_info) = graph.crates.get(crate_id)
        {
            let change_type = if status.is_directly_changed {
                "DIRECT"
            } else {
                "transitive"
            };

            let bump_status = if status.is_bumped {
                format!(
                    "BUMPED: {} -> {}",
                    status.base_version, status.current_version
                )
            } else {
                format!("NEEDS BUMP: {} (unchanged)", status.base_version)
            };

            println!(
                "\n{} ({}) [{}]",
                crate_info.id.name, crate_info.id.workspace, change_type
            );
            println!("  {}", bump_status);
        }
    }

    // summary
    println!("\n{}", "=".repeat(70));
    println!("SUMMARY");
    println!("{}", "=".repeat(70));

    if version_analysis.all_bumped() {
        println!("\nall affected crates have been version-bumped!");
    } else {
        println!("\nsome crates need version bumps:");
        for crate_id in &version_analysis.crates_needing_bump {
            if let Some(crate_info) = graph.crates.get(crate_id)
                && let Some(status) = version_analysis.crate_versions.get(crate_id)
            {
                let change_type = if status.is_directly_changed {
                    "DIRECT"
                } else {
                    "transitive"
                };
                println!(
                    "  - {} ({}) [{}] - version: {}",
                    crate_info.id.name,
                    crate_info.id.workspace,
                    change_type,
                    status.current_version
                );
            }
        }
    }

    Ok(())
}
