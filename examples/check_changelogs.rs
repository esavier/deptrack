// example: check changelog status for all affected crates
//
// demonstrates how to:
// 1. analyze git changes
// 2. check version bumps
// 3. verify changelog status
// 4. identify issues and missing changelogs

use deptrack::{ChangelogChecker, ChangelogConfig, CrateDependencyGraph, GitRef, SeverityConfig};

fn main() -> deptrack::Result<()> {
    let repo_path = std::env::current_dir()?;

    println!("=== Changelog Analysis for Repository ===\n");
    println!("repository: {}\n", repo_path.display());

    // build the dependency graph
    let graph = CrateDependencyGraph::build_from_repository(&repo_path)?;
    println!("found {} crates in repository\n", graph.crates.len());

    // analyze changes from master to current state
    let base_ref = GitRef::Branch("master".to_string());
    let current_ref = GitRef::Head;

    println!("analyzing changes from master to HEAD...\n");

    // get committed changes
    let committed_impact = graph.analyze_git_changes(&repo_path, &base_ref, &current_ref)?;

    // get working directory changes
    let working_impact = graph.analyze_working_directory_changes(&repo_path)?;

    // combine impacts
    let total_changed_files =
        committed_impact.changed_files.len() + working_impact.changed_files.len();
    let total_affected = {
        let mut affected: Vec<_> = committed_impact.all_affected_crates.clone();
        for crate_id in &working_impact.all_affected_crates {
            if !affected.contains(crate_id) {
                affected.push(crate_id.clone());
            }
        }
        affected
    };

    println!("change summary:");
    println!(
        "  committed changes:      {} files",
        committed_impact.changed_files.len()
    );
    println!(
        "  working dir changes:    {} files",
        working_impact.changed_files.len()
    );
    println!("  total changed files:    {}", total_changed_files);
    println!(
        "  directly affected:      {} crates",
        committed_impact.directly_affected_crates.len()
    );
    println!(
        "  total affected:         {} crates\n",
        total_affected.len()
    );

    // create severity configs
    let direct_severity = SeverityConfig::default_direct();
    let transitive_severity = SeverityConfig::default_transitive();

    // analyze version bumps
    println!("checking version bumps...");
    let version_analysis = graph.analyze_version_bumps(
        &repo_path,
        &base_ref,
        &total_affected,
        &committed_impact.directly_affected_crates,
        &direct_severity,
        &transitive_severity,
    )?;

    println!("version bump status:");
    println!(
        "  properly bumped:  {}/{} ({:.1}%)",
        version_analysis.crates_bumped.len(),
        version_analysis.crate_versions.len(),
        version_analysis.bump_percentage()
    );
    println!(
        "  need bumps:       {}",
        version_analysis.crates_needing_bump.len()
    );
    println!();

    // configure changelog verification
    let changelog_config = ChangelogConfig::default()
        .require_changelog(true)
        .enforce_format(true)
        .check_changelog_updated(true)
        .allow_missing_for_transitive(true);

    println!("analyzing changelogs with configuration:");
    println!(
        "  changelog file:           {}",
        changelog_config.changelog_file_name
    );
    println!(
        "  require changelog:        {}",
        changelog_config.require_changelog
    );
    println!(
        "  enforce format:           {}",
        changelog_config.enforce_format
    );
    println!(
        "  check updated:            {}",
        changelog_config.check_changelog_updated
    );
    println!(
        "  allow missing transitive: {}\n",
        changelog_config.allow_missing_for_transitive
    );

    // analyze changelogs
    let changelog_analysis = ChangelogChecker::analyze_for_changes(
        &graph,
        &repo_path,
        &changelog_config,
        &direct_severity,
        &transitive_severity,
        &version_analysis,
        &committed_impact,
    )?;

    println!("changelog analysis results:");
    println!(
        "  valid changelogs:         {}",
        changelog_analysis.crates_with_valid_changelog.len()
    );
    println!(
        "  missing changelogs:       {}",
        changelog_analysis.crates_missing_changelog.len()
    );
    println!(
        "  need updates:             {}",
        changelog_analysis.crates_needing_changelog_update.len()
    );
    println!(
        "  total issues:             {}\n",
        changelog_analysis.total_issues
    );

    // display detailed status for each affected crate
    println!("=== Detailed Changelog Status ===\n");

    for crate_id in &total_affected {
        if let Some(status) = changelog_analysis.statuses.get(crate_id) {
            let symbol = if status.has_changelog
                && status.format_valid
                && status.current_version_has_entry
            {
                "[OK]"
            } else if !status.has_changelog {
                "[MISS]"
            } else {
                "[WARN]"
            };

            println!("{} {}", symbol, crate_id.display_name());
            println!("   has changelog:          {}", status.has_changelog);

            if status.has_changelog {
                println!("   format valid:           {}", status.format_valid);
                println!(
                    "   version has entry:      {}",
                    status.current_version_has_entry
                );
                println!(
                    "   changelog updated:      {}",
                    status.changelog_was_updated
                );

                if let Some(changelog_path) = &status.changelog_path {
                    println!("   path:                   {}", changelog_path.display());
                }
            }

            if !status.issues.is_empty() {
                println!("   issues:");
                for issue in &status.issues {
                    println!("     - {}", issue);
                }
            }
            println!();
        }
    }

    // summary and recommendations
    println!("=== Summary and Recommendations ===\n");

    if changelog_analysis.total_issues == 0 {
        println!("all affected crates have valid changelogs!");
    } else {
        println!(
            "found {} issue(s) that need attention:\n",
            changelog_analysis.total_issues
        );

        if !changelog_analysis.crates_missing_changelog.is_empty() {
            println!("crates missing changelogs:");
            for crate_id in &changelog_analysis.crates_missing_changelog {
                if let Some(crate_info) = graph.crates.get(crate_id) {
                    let crate_root = crate_info.path.parent().unwrap_or(&crate_info.path);
                    println!(
                        "  - {} (create at: {}/{})",
                        crate_id.display_name(),
                        crate_root.display(),
                        changelog_config.changelog_file_name
                    );
                }
            }
            println!();
        }

        if !changelog_analysis
            .crates_needing_changelog_update
            .is_empty()
        {
            println!("crates needing changelog updates:");
            for crate_id in &changelog_analysis.crates_needing_changelog_update {
                if let Some(status) = changelog_analysis.statuses.get(crate_id)
                    && let Some(version_status) = version_analysis.crate_versions.get(crate_id)
                {
                    println!(
                        "  - {} (version: {})",
                        crate_id.display_name(),
                        version_status.current_version
                    );
                    if !status.issues.is_empty() {
                        for issue in &status.issues {
                            println!("    * {}", issue);
                        }
                    }
                }
            }
            println!();
        }

        println!("recommended changelog format:");
        println!("  # CHANGELOG");
        println!();
        println!("  # 0.2.0");
        println!();
        println!("  * feat(scope): add new feature");
        println!("  * fix(scope): resolve bug");
        println!();
        println!("  # 0.1.0");
        println!();
        println!("  * chore(scope): initial release");
    }

    Ok(())
}
