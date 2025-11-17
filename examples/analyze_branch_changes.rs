// analyze all changes against master branch including uncommitted work

use deptrack::{ChangeImpactAnalysis, CrateDependencyGraph, Result};
use std::collections::HashSet;

fn main() -> Result<()> {
    let repo_path = "test_repo";

    println!("analyzing changes in repository: {}", repo_path);
    println!("{}", "=".repeat(70));

    // build the dependency graph
    let graph = CrateDependencyGraph::build_from_repository(repo_path)?;

    println!("\ntotal crates: {}", graph.crates.len());

    // analyze committed changes on current branch vs master
    println!("\n{}", "=".repeat(70));
    println!("COMMITTED CHANGES (feature-branch vs master)");
    println!("{}", "=".repeat(70));

    let committed_analysis = graph.analyze_git_changes(
        repo_path,
        &deptrack::GitRef::Branch("master".to_string()),
        &deptrack::GitRef::Branch("HEAD".to_string()),
    )?;

    print_analysis("committed changes", &committed_analysis, &graph);

    // analyze working directory changes (staged + unstaged)
    println!("\n{}", "=".repeat(70));
    println!("WORKING DIRECTORY CHANGES (staged + unstaged)");
    println!("{}", "=".repeat(70));

    let working_dir_analysis = graph.analyze_working_directory_changes(repo_path)?;

    print_analysis("working directory changes", &working_dir_analysis, &graph);

    // combine all changes for total impact
    println!("\n{}", "=".repeat(70));
    println!("TOTAL IMPACT (all changes vs master)");
    println!("{}", "=".repeat(70));

    let mut all_changed_files: HashSet<_> = committed_analysis.changed_files.iter().collect();
    all_changed_files.extend(working_dir_analysis.changed_files.iter());

    let mut all_directly_affected: HashSet<_> =
        committed_analysis.directly_affected_crates.iter().collect();
    all_directly_affected.extend(working_dir_analysis.directly_affected_crates.iter());

    let mut all_affected: HashSet<_> = committed_analysis.all_affected_crates.iter().collect();
    all_affected.extend(working_dir_analysis.all_affected_crates.iter());

    let mut all_needs_rebuild: HashSet<_> = committed_analysis.needs_rebuild.iter().collect();
    all_needs_rebuild.extend(working_dir_analysis.needs_rebuild.iter());

    println!("\ntotal changed files: {}", all_changed_files.len());
    for file in &all_changed_files {
        println!("  - {}", file.display());
    }

    println!(
        "\ndirectly affected crates: {}",
        all_directly_affected.len()
    );
    for crate_id in &all_directly_affected {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            println!("  - {} ({})", crate_info.id.name, crate_info.id.workspace);
        }
    }

    println!(
        "\nall affected crates (including transitive): {}",
        all_affected.len()
    );
    for crate_id in &all_affected {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            let is_direct = all_directly_affected.contains(crate_id);
            let marker = if is_direct { "DIRECT" } else { "transitive" };
            println!(
                "  - {} ({}) [{}]",
                crate_info.id.name, crate_info.id.workspace, marker
            );
        }
    }

    println!(
        "\ncrates needing rebuild (topological order): {}",
        all_needs_rebuild.len()
    );
    for crate_id in &all_needs_rebuild {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            println!("  - {}", crate_info.id.name);
        }
    }

    Ok(())
}

fn print_analysis(title: &str, analysis: &ChangeImpactAnalysis, graph: &CrateDependencyGraph) {
    println!("\n{}", title);
    println!("{}", "-".repeat(70));

    println!("\nchanged files: {}", analysis.changed_files.len());
    for file in &analysis.changed_files {
        println!("  - {}", file.display());
    }

    println!(
        "\ndirectly affected crates: {}",
        analysis.directly_affected_crates.len()
    );
    for crate_id in &analysis.directly_affected_crates {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            println!("  - {} ({})", crate_info.id.name, crate_info.id.workspace);
        }
    }

    println!(
        "\nall affected crates (including transitive): {}",
        analysis.all_affected_crates.len()
    );
    for crate_id in &analysis.all_affected_crates {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            let is_direct = analysis.directly_affected_crates.contains(crate_id);
            let marker = if is_direct { "DIRECT" } else { "transitive" };
            println!(
                "  - {} ({}) [{}]",
                crate_info.id.name, crate_info.id.workspace, marker
            );
        }
    }

    println!(
        "\ncrates needing rebuild (topological order): {}",
        analysis.needs_rebuild.len()
    );
    for crate_id in &analysis.needs_rebuild {
        if let Some(crate_info) = graph.crates.get(crate_id) {
            println!("  - {}", crate_info.id.name);
        }
    }
}
