use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use deptrack::{CargoDiscovery, CrateDependencyGraph, GitOps};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "deptrack")]
#[command(version, about = "dependency tracking tool for Rust workspaces", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// path to the repository (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    path: PathBuf,

    /// output format (json or human)
    #[arg(short, long, default_value = "human", global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug)]
enum OutputFormat {
    Json,
    Human,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "human" => Ok(OutputFormat::Human),
            _ => Err(format!(
                "invalid output format: {}, use 'json' or 'human'",
                s
            )),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// analyze repository and show dependency information
    Analyze {
        /// path to the repository (optional, defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// show detailed dependency graph
        #[arg(short, long)]
        graph: bool,
    },

    /// check version bumps for changed crates between git refs
    CheckVersions {
        /// base reference (branch, tag, or commit)
        from: String,

        /// target reference (branch, tag, or commit)
        to: String,

        /// path to the repository (optional, defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// skip changelog validation
        #[arg(long)]
        skip_changelog: bool,

        /// check all crates (not just those with changes)
        #[arg(long)]
        all_crates: bool,

        /// show detailed issue tables (errors and warnings)
        #[arg(short, long)]
        verbose: bool,
    },

    /// git operations and change tracking
    Git {
        #[command(subcommand)]
        command: GitCommands,
    },

    #[cfg(debug_assertions)]
    /// [debug] list all workspaces in the repository
    Workspaces,

    #[cfg(debug_assertions)]
    /// [debug] list all crates in the repository
    Crates {
        /// show dependencies for each crate
        #[arg(short, long)]
        deps: bool,
    },

    #[cfg(debug_assertions)]
    /// [debug] show dependency graph in various formats
    Graph {
        /// output format: dot, json, or stats
        #[arg(short = 'o', long = "output", default_value = "stats")]
        output: GraphFormat,
    },

    #[cfg(debug_assertions)]
    /// [debug] show statistics about the repository
    Stats,
}

#[derive(Subcommand)]
enum GitCommands {
    /// list branches in the repository
    Branches {
        /// path to the repository (optional, defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// show current branch
    CurrentBranch {
        /// path to the repository (optional, defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// list files changed between two git references
    Changes {
        /// start reference (branch, tag, or commit)
        from: String,

        /// end reference (branch, tag, or commit)
        to: String,

        /// path to the repository (optional, defaults to current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// filter by file extension
        #[arg(short, long)]
        extension: Option<String>,

        /// show only changes unique to the 'to' branch (excluding merged changes from 'from')
        /// this uses "from...to" (three-dot) syntax to compare from merge-base
        #[arg(short = 'u', long)]
        unique: bool,
    },
}

#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
enum GraphFormat {
    Dot,
    Json,
    Stats,
}

#[cfg(debug_assertions)]
impl std::str::FromStr for GraphFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dot" => Ok(GraphFormat::Dot),
            "json" => Ok(GraphFormat::Json),
            "stats" => Ok(GraphFormat::Stats),
            _ => Err(format!(
                "invalid graph format: {}, use 'dot', 'json', or 'stats'",
                s
            )),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, graph } => {
            let repo_path = path.as_ref().unwrap_or(&cli.path);
            handle_analyze(repo_path, &cli.format, graph)?;
        }
        Commands::CheckVersions {
            from,
            to,
            path,
            skip_changelog,
            all_crates,
            verbose,
        } => {
            let repo_path = path.as_ref().unwrap_or(&cli.path);
            handle_check_versions(
                repo_path,
                &cli.format,
                &from,
                &to,
                skip_changelog,
                all_crates,
                verbose,
            )?;
        }
        Commands::Git { command } => {
            handle_git(&cli.path, &cli.format, command)?;
        }
        #[cfg(debug_assertions)]
        Commands::Workspaces => {
            handle_debug_workspaces(&cli.path, &cli.format)?;
        }
        #[cfg(debug_assertions)]
        Commands::Crates { deps } => {
            handle_debug_crates(&cli.path, &cli.format, deps)?;
        }
        #[cfg(debug_assertions)]
        Commands::Graph { output } => {
            handle_debug_graph(&cli.path, &cli.format, output)?;
        }
        #[cfg(debug_assertions)]
        Commands::Stats => {
            handle_debug_stats(&cli.path, &cli.format)?;
        }
    }

    Ok(())
}

fn handle_analyze(path: &PathBuf, format: &OutputFormat, show_graph: bool) -> Result<()> {
    // canonicalize path for display
    let repo_path = path.canonicalize().unwrap_or_else(|_| path.clone());

    let workspaces =
        CargoDiscovery::discover_workspaces(path).context("failed to discover cargo workspace")?;
    let graph = CrateDependencyGraph::build_from_repository(path)
        .context("failed to build dependency graph")?;

    let total_crates: usize = workspaces.iter().map(|w| w.members.len()).sum();

    match format {
        OutputFormat::Json => {
            let stats = graph.get_statistics();
            let output = serde_json::json!({
                "repository_path": repo_path,
                "workspaces": workspaces.len(),
                "crates": total_crates,
                "has_cycles": stats.has_cycles,
                "statistics": stats,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Human => {
            let stats = graph.get_statistics();

            // display repository structure
            graph.display_repository_structure(&repo_path);

            // display dependency summary
            graph.display_dependency_summary();

            // always display cycles if detected
            if stats.has_cycles {
                println!();
                println!("warning: cyclic dependencies detected");
                println!();
                graph.display_cycles();
            }

            if show_graph {
                println!();
                println!("additional graph statistics:");
                println!("  crate count: {}", stats.crate_count);
                println!("  cycle count: {}", stats.cycle_count);
            }
        }
    }

    Ok(())
}

fn handle_check_versions(
    path: &PathBuf,
    format: &OutputFormat,
    from_ref_str: &str,
    to_ref_str: &str,
    skip_changelog: bool,
    all_crates: bool,
    verbose: bool,
) -> Result<()> {
    use deptrack::{ChangelogChecker, DeptrackConfig, GitRef};

    // canonicalize path for display
    let repo_path = path.canonicalize().unwrap_or_else(|_| path.clone());

    // load configuration
    let config = DeptrackConfig::load_or_default(path);

    // build dependency graph
    let graph = CrateDependencyGraph::build_from_repository(path)
        .context("failed to build dependency graph")?;

    // analyze changes between refs
    let from_ref = GitRef::from_string(from_ref_str);
    let to_ref = GitRef::from_string(to_ref_str);

    let impact_analysis = graph
        .analyze_git_changes(path, &from_ref, &to_ref)
        .context("failed to analyze git changes")?;

    if impact_analysis.changed_files.is_empty() {
        match format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "from": from_ref_str,
                    "to": to_ref_str,
                    "repository_path": repo_path,
                    "changed_files": 0,
                    "directly_affected": 0,
                    "total_affected": 0,
                    "bumped": 0,
                    "needing_bump": 0,
                    "bump_percentage": 100.0,
                    "all_bumped": true,
                    "changelog_skipped": skip_changelog,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Human => {
                println!(
                    "checking version bumps between {} and {}",
                    from_ref_str, to_ref_str
                );
                println!("repository: {}", repo_path.display());
                println!();
                println!(
                    "no changes detected between {} and {}",
                    from_ref_str, to_ref_str
                );
            }
        }
        return Ok(());
    }

    // analyze version bumps
    let version_analysis = graph
        .analyze_version_bumps(
            path,
            &from_ref,
            &impact_analysis.all_affected_crates,
            &impact_analysis.directly_affected_crates,
            &config.direct_severity,
            &config.transitive_severity,
        )
        .context("failed to analyze version bumps")?;

    // analyze changelogs if not skipped
    let changelog_analysis = if !skip_changelog {
        let analysis = if all_crates {
            ChangelogChecker::analyze_all(&graph, path, &config.changelog, &config.direct_severity)
                .context("failed to analyze changelogs")?
        } else {
            ChangelogChecker::analyze_for_changes(
                &graph,
                path,
                &config.changelog,
                &config.direct_severity,
                &config.transitive_severity,
                &version_analysis,
                &impact_analysis,
            )
            .context("failed to analyze changelogs")?
        };
        Some(analysis)
    } else {
        None
    };

    match format {
        OutputFormat::Json => {
            let mut output = serde_json::json!({
                "from": from_ref_str,
                "to": to_ref_str,
                "repository_path": repo_path,
                "changed_files": impact_analysis.changed_files.len(),
                "directly_affected": impact_analysis.directly_affected_crates.len(),
                "total_affected": impact_analysis.all_affected_crates.len(),
                "bumped": version_analysis.crates_bumped.len(),
                "needing_bump": version_analysis.crates_needing_bump.len(),
                "bump_percentage": version_analysis.bump_percentage(),
                "all_bumped": version_analysis.all_bumped(),
            });

            // add version bump error/warning counts
            output["version_bump_errors"] = serde_json::json!(version_analysis.total_errors);
            output["version_bump_warnings"] = serde_json::json!(version_analysis.total_warnings);

            if let Some(ref analysis) = changelog_analysis {
                output["changelog"] = serde_json::json!({
                    "analyzed_crates": analysis.statuses.len(),
                    "valid_changelogs": analysis.crates_with_valid_changelog.len(),
                    "missing_changelogs": analysis.crates_missing_changelog.len(),
                    "needing_updates": analysis.crates_needing_changelog_update.len(),
                    "total_issues": analysis.total_issues,
                    "total_errors": analysis.total_errors,
                    "total_warnings": analysis.total_warnings,
                    "compliance_percentage": analysis.compliance_percentage(),
                    "all_valid": analysis.all_valid(),
                });
            } else {
                output["changelog_skipped"] = serde_json::json!(true);
            }

            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Human => {
            println!(
                "checking version bumps between {} and {}",
                from_ref_str, to_ref_str
            );
            println!("repository: {}", repo_path.display());
            println!();

            println!("detected changes:");
            println!("  files changed: {}", impact_analysis.changed_files.len());
            println!(
                "  directly affected crates: {}",
                impact_analysis.directly_affected_crates.len()
            );
            println!(
                "  total affected crates: {}",
                impact_analysis.all_affected_crates.len()
            );
            println!();

            version_analysis.display_table();

            if verbose && (version_analysis.total_errors > 0 || version_analysis.total_warnings > 0)
            {
                println!();
                version_analysis.display_issues();
            }

            if let Some(ref analysis) = changelog_analysis {
                println!();
                analysis.display_table();

                if verbose && analysis.total_issues > 0 {
                    println!();
                    analysis.display_issues();
                }
            }
        }
    }

    // check for errors and fail if any are present
    let total_errors = version_analysis.total_errors
        + changelog_analysis
            .as_ref()
            .map(|a| a.total_errors)
            .unwrap_or(0);

    if total_errors > 0 {
        let total_warnings = version_analysis.total_warnings
            + changelog_analysis
                .as_ref()
                .map(|a| a.total_warnings)
                .unwrap_or(0);

        if matches!(format, OutputFormat::Human) {
            println!();
            println!(
                "validation failed: {} error(s), {} warning(s)",
                total_errors, total_warnings
            );
        }

        // return error to cause non-zero exit code
        anyhow::bail!("validation failed with {} error(s)", total_errors);
    }

    Ok(())
}

fn handle_git(default_path: &PathBuf, format: &OutputFormat, command: GitCommands) -> Result<()> {
    match command {
        GitCommands::Branches { path } => {
            let repo_path = path.as_ref().unwrap_or(default_path);
            let _git_repo =
                GitOps::get_repository_info(repo_path).context("failed to open git repository")?;

            let branches = GitOps::list_branches(repo_path).context("failed to list branches")?;
            match format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&branches)?);
                }
                OutputFormat::Human => {
                    println!("branches:");
                    for branch in branches {
                        println!("  {}", branch);
                    }
                }
            }
        }
        GitCommands::CurrentBranch { path } => {
            let repo_path = path.as_ref().unwrap_or(default_path);
            let _git_repo =
                GitOps::get_repository_info(repo_path).context("failed to open git repository")?;

            let branch =
                GitOps::get_current_branch(repo_path).context("failed to get current branch")?;
            match format {
                OutputFormat::Json => {
                    let output = serde_json::json!({ "current_branch": branch });
                    println!("{}", serde_json::to_string_pretty(&output)?);
                }
                OutputFormat::Human => {
                    println!("current branch: {}", branch);
                }
            }
        }
        GitCommands::Changes {
            from,
            to,
            path,
            extension,
            unique,
        } => {
            use deptrack::GitRef;

            let repo_path = path.as_ref().unwrap_or(default_path);
            let _git_repo =
                GitOps::get_repository_info(repo_path).context("failed to open git repository")?;

            let from_ref = GitRef::from_string(&from);
            let to_ref = GitRef::from_string(&to);

            let changed_files = if unique {
                GitOps::list_unique_changes(repo_path, &from_ref, &to_ref)
                    .context("failed to list unique changes")?
            } else {
                GitOps::list_changed_files(repo_path, &from_ref, &to_ref)
                    .context("failed to list changes")?
            };

            let filtered_changes: Vec<_> = if let Some(ext) = extension {
                changed_files
                    .changes
                    .into_iter()
                    .filter(|c| {
                        c.path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e == ext)
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                changed_files.changes
            };

            match format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&filtered_changes)?);
                }
                OutputFormat::Human => {
                    println!("changes from {} to {}:", from, to);
                    for change in filtered_changes {
                        println!("  {:?}: {}", change.change_type, change.path.display());
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(debug_assertions)]
fn handle_debug_workspaces(path: &PathBuf, format: &OutputFormat) -> Result<()> {
    let workspaces =
        CargoDiscovery::discover_workspaces(path).context("failed to discover cargo workspace")?;

    match format {
        OutputFormat::Json => {
            let output: Vec<_> = workspaces
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "name": w.name,
                        "root": w.root_path,
                        "members": w.members.len(),
                        "member_paths": w.members,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Human => {
            println!("workspaces:");
            for workspace in &workspaces {
                println!(
                    "  {} ({} members)",
                    workspace.root_path.display(),
                    workspace.members.len()
                );
                for member in &workspace.members {
                    println!("    - {}", member);
                }
            }
        }
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn handle_debug_crates(path: &PathBuf, format: &OutputFormat, _deps: bool) -> Result<()> {
    let workspaces =
        CargoDiscovery::discover_workspaces(path).context("failed to discover cargo workspace")?;

    {
        let mut all_crates = Vec::new();
        for workspace in &workspaces {
            let crates = CargoDiscovery::discover_crates_in_workspace(workspace)
                .context("failed to discover crates")?;
            all_crates.extend(crates);
        }

        match format {
            OutputFormat::Json => {
                let output: Vec<_> = all_crates
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "name": c.id.name,
                            "workspace": c.id.workspace,
                            "version": c.version,
                            "path": c.path,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Human => {
                println!("crates:");
                for crate_info in &all_crates {
                    println!(
                        "  {}::{} v{} ({})",
                        crate_info.id.workspace,
                        crate_info.id.name,
                        crate_info.version,
                        crate_info.path.display()
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn handle_debug_graph(
    path: &PathBuf,
    format: &OutputFormat,
    graph_format: GraphFormat,
) -> Result<()> {
    {
        let graph =
            CrateDependencyGraph::build_from_repository(path).context("failed to build graph")?;

        match graph_format {
            GraphFormat::Stats => {
                let stats = graph.get_statistics();
                match format {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&stats)?);
                    }
                    OutputFormat::Human => {
                        println!("dependency graph statistics:");
                        println!("  crate count: {}", stats.crate_count);
                        println!("  dependency count: {}", stats.dependency_count);
                        println!("  has cycles: {}", stats.has_cycles);
                    }
                }
            }
            GraphFormat::Json => {
                // serialize the entire graph structure
                let output: Vec<_> = graph.crates
                        .values()
                        .map(|c| {
                            let deps = graph.get_dependencies(&c.id);
                            let dependents = graph.get_dependents(&c.id);

                            serde_json::json!({
                                "name": c.id.name,
                                "workspace": c.id.workspace,
                                "version": c.version,
                                "dependencies": deps.iter().map(|id| id.display_name()).collect::<Vec<_>>(),
                                "dependents": dependents.iter().map(|id| id.display_name()).collect::<Vec<_>>(),
                            })
                        })
                        .collect();
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            GraphFormat::Dot => {
                // generate DOT format for graphviz
                println!("digraph dependencies {{");
                println!("  rankdir=LR;");
                println!("  node [shape=box];");

                for crate_info in graph.crates.values() {
                    let deps = graph.get_dependencies(&crate_info.id);

                    for dep_id in deps {
                        println!(
                            "  \"{}\" -> \"{}\";",
                            crate_info.id.display_name(),
                            dep_id.display_name()
                        );
                    }
                }

                println!("}}");
            }
        }
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn handle_debug_stats(path: &PathBuf, format: &OutputFormat) -> Result<()> {
    let workspaces =
        CargoDiscovery::discover_workspaces(path).context("failed to discover cargo workspace")?;

    {
        let graph =
            CrateDependencyGraph::build_from_repository(path).context("failed to build graph")?;
        let stats = graph.get_statistics();

        match format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "workspaces": workspaces.len(),
                    "crates": graph.crates.len(),
                    "statistics": stats,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Human => {
                println!("repository statistics:");
                println!("  workspaces: {}", workspaces.len());
                println!("  crates: {}", graph.crates.len());
                println!("  dependency count: {}", stats.dependency_count);
                println!("  has cycles: {}", stats.has_cycles);
            }
        }
    }
    Ok(())
}
