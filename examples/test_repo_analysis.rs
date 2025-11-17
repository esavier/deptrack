// test deptrack on the test_repo

use deptrack::{CrateDependencyGraph, Result};

fn main() -> Result<()> {
    let repo_path = "test_repo";

    println!("analyzing repository: {}", repo_path);
    println!("{}", "=".repeat(60));

    let graph = CrateDependencyGraph::build_from_repository(repo_path)?;

    println!("\ntotal crates found: {}", graph.crates.len());
    println!("\ncrate details:");
    println!("{}", "-".repeat(60));

    for (crate_id, crate_info) in &graph.crates {
        println!("\n{}", crate_info.id.name);
        println!("  path: {}", crate_info.path.display());

        let deps = graph.get_dependencies(crate_id);
        if !deps.is_empty() {
            println!("  dependencies:");
            for dep_id in deps {
                if let Some(dep_info) = graph.crates.get(dep_id) {
                    println!("    - {}", dep_info.id.name);
                }
            }
        } else {
            println!("  dependencies: none");
        }

        let dependents = graph.get_dependents(crate_id);
        if !dependents.is_empty() {
            println!("  dependents:");
            for dep_id in dependents {
                if let Some(dep_info) = graph.crates.get(dep_id) {
                    println!("    - {}", dep_info.id.name);
                }
            }
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("dependency analysis:");
    println!("{}", "-".repeat(60));

    // check for cycles
    if graph.has_cycles() {
        println!("WARNING: cyclic dependencies detected!");
    } else {
        println!("no cyclic dependencies found");
    }

    // topological sort
    if let Some(sorted) = graph.topological_order() {
        println!("\nbuild order (topological sort):");
        for (i, crate_id) in sorted.iter().enumerate() {
            if let Some(crate_info) = graph.crates.get(crate_id) {
                println!("  {}. {}", i + 1, crate_info.id.name);
            }
        }
    } else {
        println!("ERROR: failed to compute build order (likely due to cycles)");
    }

    // verify cross-workspace dependencies
    println!("\n{}", "=".repeat(60));
    println!("cross-workspace dependencies:");
    println!("{}", "-".repeat(60));

    for (crate_id, crate_info) in &graph.crates {
        let deps = graph.get_dependencies(crate_id);
        for dep_id in deps {
            if let Some(dep_info) = graph.crates.get(dep_id) {
                // check if they're in different workspaces
                // path structure: test_repo/workspace_1/crate_D
                // we want to extract "workspace_1" vs "workspace_2"
                let crate_ws = crate_info
                    .path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str());
                let dep_ws = dep_info
                    .path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str());

                if crate_ws != dep_ws && crate_ws.is_some() && dep_ws.is_some() {
                    println!(
                        "  {} → {} ({} → {})",
                        crate_info.id.name,
                        dep_info.id.name,
                        crate_ws.unwrap(),
                        dep_ws.unwrap()
                    );
                }
            }
        }
    }

    Ok(())
}
