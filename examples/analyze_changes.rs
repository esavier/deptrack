use deptrack::{CrateDependencyGraph, GitRef};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get repository path from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let repo_path = if args.len() > 1 {
        &args[1]
    } else {
        "/home/esavier/.repos/_fathom/platform-backend/"
    };

    println!("Analyzing repository: {}", repo_path);
    println!("{}", "=".repeat(60));

    // Build the dependency graph
    println!("\n1. Building dependency graph...");
    let graph = match CrateDependencyGraph::build_from_repository(repo_path) {
        Ok(g) => {
            println!("   ✓ Found {} crates", g.crates.len());
            g
        }
        Err(e) => {
            eprintln!("   ✗ Failed to build dependency graph: {}", e);
            return Err(e.into());
        }
    };

    // Analyze changes against main branch
    println!("\n2. Analyzing changes against main branch...");
    let from_ref = GitRef::Branch("main".to_string());
    let to_ref = GitRef::Head;

    let analysis = match graph.analyze_git_changes(repo_path, &from_ref, &to_ref) {
        Ok(a) => {
            println!("   ✓ Analysis complete");
            a
        }
        Err(e) => {
            eprintln!("   ✗ Failed to analyze changes: {}", e);
            return Err(e.into());
        }
    };

    // Display results
    println!("\n3. Change Impact Analysis Results:");
    println!("   {}", "-".repeat(58));
    println!("   Changed files:        {}", analysis.changed_files.len());
    println!(
        "   Directly affected:    {} crates",
        analysis.direct_impact_count()
    );
    println!(
        "   Total affected:       {} crates (including dependents)",
        analysis.total_impact_count()
    );
    println!(
        "   Needs rebuild:        {} crates",
        analysis.needs_rebuild.len()
    );

    // Show changed files (limited to first 10)
    if !analysis.changed_files.is_empty() {
        println!("\n4. Changed Files (showing first 10):");
        println!("   {}", "-".repeat(58));
        for (i, file) in analysis.changed_files.iter().take(10).enumerate() {
            println!("   {}. {}", i + 1, file.display());
        }
        if analysis.changed_files.len() > 10 {
            println!(
                "   ... and {} more files",
                analysis.changed_files.len() - 10
            );
        }
    }

    // Show directly affected crates
    if !analysis.directly_affected_crates.is_empty() {
        println!("\n5. Directly Affected Crates:");
        println!("   {}", "-".repeat(58));
        for (i, crate_id) in analysis.directly_affected_crates.iter().enumerate() {
            println!("   {}. {}", i + 1, crate_id.display_name());
        }
    }

    // Show all affected crates (including transitive dependencies)
    if !analysis.all_affected_crates.is_empty() {
        println!("\n6. All Affected Crates (including dependents):");
        println!("   {}", "-".repeat(58));
        for (i, crate_id) in analysis.all_affected_crates.iter().enumerate() {
            println!("   {}. {}", i + 1, crate_id.display_name());
        }
    }

    // Summary
    println!("\n{}", "=".repeat(60));
    println!("Summary:");
    if analysis.direct_impact_count() > 0 {
        println!(
            "  • {} crates were directly modified",
            analysis.direct_impact_count()
        );
        if analysis.total_impact_count() > analysis.direct_impact_count() {
            println!(
                "  • {} additional crates are affected through dependencies",
                analysis.total_impact_count() - analysis.direct_impact_count()
            );
        }
        println!(
            "  • All {} crates should be rebuilt",
            analysis.needs_rebuild.len()
        );
    } else {
        println!("  • No crate files were directly modified");
        println!("  • Changes may be in non-crate files (docs, configs, etc.)");
    }
    println!("{}", "=".repeat(60));

    Ok(())
}
