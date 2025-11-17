use super::discovery::CargoDiscovery;
use super::types::{CrateDependencyGraph, CrateId, CrateInfo, DependencyType};
use crate::error::{Error, Result};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::path::Path;

impl CrateDependencyGraph {
    /// Build a complete dependency graph from a repository
    pub fn build_from_repository<P: AsRef<Path>>(repo_root: P) -> Result<Self> {
        let repo_root = repo_root.as_ref();
        let mut graph = Self::new();

        // Step 1: Discover all workspaces
        let workspaces = CargoDiscovery::discover_workspaces(repo_root)?;
        graph.workspaces = workspaces.clone();

        // Step 2: Discover all crates across all workspaces
        let mut all_crates = Vec::new();
        for workspace in &workspaces {
            let workspace_crates = CargoDiscovery::discover_crates_in_workspace(workspace)?;
            all_crates.extend(workspace_crates);
        }

        // Step 3: Add all crates to the graph
        for crate_info in &all_crates {
            graph.add_crate(crate_info.clone());
        }

        // Step 4: Build dependency relationships
        for crate_info in &all_crates {
            let local_deps =
                CargoDiscovery::parse_local_dependencies_with_types(crate_info, &all_crates)?;

            for (dep_name, dep_type) in local_deps {
                // Find the target crate
                if let Some(target_crate) = all_crates.iter().find(|c| c.id.name == dep_name) {
                    graph.add_dependency(&crate_info.id, &target_crate.id, dep_type);
                }
            }
        }

        Ok(graph)
    }

    /// Rebuild the graph from current crate data
    pub fn rebuild_dependencies(&mut self) -> Result<()> {
        // Clear existing edges but keep nodes
        self.graph.clear_edges();

        let all_crates: Vec<CrateInfo> = self.crates.values().cloned().collect();

        // Rebuild all dependency edges
        for crate_info in &all_crates {
            let local_deps =
                CargoDiscovery::parse_local_dependencies_with_types(crate_info, &all_crates)?;

            for (dep_name, dep_type) in local_deps {
                if let Some(target_crate) = all_crates.iter().find(|c| c.id.name == dep_name) {
                    self.add_dependency(&crate_info.id, &target_crate.id, dep_type);
                }
            }
        }

        Ok(())
    }

    /// Find all dependency cycles in the graph
    ///
    /// returns a vector of cycles, where each cycle is a vector of CrateIds forming a circular dependency
    pub fn find_cycles(&self) -> Vec<Vec<&CrateId>> {
        self.find_cycles_filtered(&[DependencyType::Normal, DependencyType::Dev, DependencyType::Build])
    }

    /// Find dependency cycles considering only specific dependency types
    ///
    /// this is useful for finding only production cycles (excluding dev and build dependencies)
    pub fn find_cycles_filtered(&self, allowed_types: &[DependencyType]) -> Vec<Vec<&CrateId>> {
        use petgraph::algo::tarjan_scc;
        use petgraph::Graph;

        // Create a filtered graph with only allowed dependency types
        let mut filtered_graph = Graph::<CrateId, DependencyType, petgraph::Directed>::new();
        let mut filtered_indices = HashMap::new();

        // Add all nodes
        for crate_id in self.crates.keys() {
            let node_idx = filtered_graph.add_node(crate_id.clone());
            filtered_indices.insert(crate_id, node_idx);
        }

        // Add only edges with allowed dependency types
        for edge_ref in self.graph.edge_references() {
            let edge_weight = edge_ref.weight();
            if allowed_types.contains(edge_weight) {
                let from_id = &self.graph[edge_ref.source()];
                let to_id = &self.graph[edge_ref.target()];

                if let (Some(&from_idx), Some(&to_idx)) =
                    (filtered_indices.get(from_id), filtered_indices.get(to_id))
                {
                    filtered_graph.add_edge(from_idx, to_idx, *edge_weight);
                }
            }
        }

        let mut cycles = Vec::new();

        // Use Tarjan's algorithm to find strongly connected components
        let sccs = tarjan_scc(&filtered_graph);

        // Each SCC with more than one node represents a cycle
        // (or a single node with a self-loop, but that's rare in practice)
        for scc in sccs {
            if scc.len() > 1 {
                // Convert node indices to CrateIds
                let cycle: Vec<&CrateId> = scc
                    .iter()
                    .map(|&node_idx| {
                        self.crates
                            .get(&filtered_graph[node_idx])
                            .map(|info| &info.id)
                            .unwrap()
                    })
                    .collect();

                cycles.push(cycle);
            } else if scc.len() == 1 {
                // Check for self-loop
                let node_idx = scc[0];
                if filtered_graph.contains_edge(node_idx, node_idx) {
                    let cycle = vec![self
                        .crates
                        .get(&filtered_graph[node_idx])
                        .map(|info| &info.id)
                        .unwrap()];
                    cycles.push(cycle);
                }
            }
        }

        cycles
    }

    /// Find only production dependency cycles (excluding dev and build dependencies)
    ///
    /// this is the most common use case as dev and build dependencies don't affect runtime
    pub fn find_production_cycles(&self) -> Vec<Vec<&CrateId>> {
        self.find_cycles_filtered(&[DependencyType::Normal])
    }

    /// Get the build order for all crates (topological sort)
    pub fn get_build_order(&self) -> Result<Vec<&CrateId>> {
        self.topological_order()
            .ok_or_else(|| Error::CyclicDependency {
                cycle: "Cyclic dependencies detected - cannot determine build order".to_string(),
            })
    }

    /// Find all crates that would be affected by changes to the given crates
    pub fn find_affected_crates<'a>(&'a self, changed_crates: &'a [CrateId]) -> Vec<&'a CrateId> {
        let mut affected = std::collections::HashSet::new();
        let mut to_visit: Vec<&CrateId> = changed_crates.iter().collect();

        while let Some(crate_id) = to_visit.pop() {
            if affected.insert(crate_id) {
                // Get all crates that depend on this one
                let dependents = self.get_dependents(crate_id);
                to_visit.extend(dependents);
            }
        }

        affected.into_iter().collect()
    }

    /// Get dependency path between two crates (if exists)
    pub fn find_dependency_path(&self, from: &CrateId, to: &CrateId) -> Option<Vec<&CrateId>> {
        let from_idx = self.node_indices.get(from)?;
        let to_idx = self.node_indices.get(to)?;

        // Use Dijkstra to find shortest path
        let node_map = petgraph::algo::dijkstra(&self.graph, *from_idx, Some(*to_idx), |_| 1);

        if node_map.contains_key(to_idx) {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current_idx = *to_idx;

            path.push(&self.graph[current_idx]);

            // This is a simplified path reconstruction
            // For a complete implementation, we'd need to track predecessors
            while current_idx != *from_idx {
                // Find a predecessor (this is simplified)
                if let Some(predecessor) = self
                    .graph
                    .edges_directed(current_idx, petgraph::Direction::Incoming)
                    .next()
                    .map(|edge| edge.source())
                {
                    current_idx = predecessor;
                    path.push(&self.graph[current_idx]);
                } else {
                    break;
                }
            }

            path.reverse();
            Some(path)
        } else {
            None
        }
    }

    /// Display repository structure with workspaces and crates hierarchy
    pub fn display_repository_structure(&self, repo_path: &std::path::Path) {
        let total_crates: usize = self.workspaces.iter().map(|w| w.members.len()).sum();

        println!("analyzing repository: {}", repo_path.display());
        println!();

        println!("repository structure:");
        println!(
            "  {} workspace(s), {} crate(s)",
            self.workspaces.len(),
            total_crates
        );
        println!();

        for workspace in &self.workspaces {
            println!("  workspace: {}", workspace.name);
            println!("    path: {}", workspace.root_path.display());
            println!("    crates: {}", workspace.members.len());
            println!();

            // get all crates for this workspace
            let workspace_crates = self.get_workspace_crates(&workspace.name);

            if workspace_crates.is_empty() {
                println!("      (no crates)");
                println!();
                continue;
            }

            // calculate column widths
            let name_width = workspace_crates
                .iter()
                .map(|c| c.id.name.len())
                .max()
                .unwrap_or(4)
                .max(4); // minimum width for "Name" header

            let version_width = workspace_crates
                .iter()
                .map(|c| c.version.len())
                .max()
                .unwrap_or(7)
                .max(7); // minimum width for "Version" header

            // print table header
            println!(
                "      {:<name_width$}  {:<version_width$}  {:>4}  {:>10}",
                "Name",
                "Version",
                "Deps",
                "Dependents",
                name_width = name_width,
                version_width = version_width
            );

            // print separator line
            println!(
                "      {}  {}  ----  ----------",
                "-".repeat(name_width),
                "-".repeat(version_width)
            );

            // print each crate
            for crate_info in workspace_crates {
                let dep_count = self.get_dependencies(&crate_info.id).len();
                let dependent_count = self.get_dependents(&crate_info.id).len();
                println!(
                    "      {:<name_width$}  {:<version_width$}  {:>4}  {:>10}",
                    crate_info.id.name,
                    crate_info.version,
                    dep_count,
                    dependent_count,
                    name_width = name_width,
                    version_width = version_width
                );
            }
            println!();
        }
    }

    /// Display dependency analysis summary
    pub fn display_dependency_summary(&self) {
        let stats = self.get_statistics();

        println!("dependency analysis:");
        println!("  total dependencies: {}", stats.dependency_count);
        println!("  has cycles: {}", stats.has_cycles);
        println!("  max dependencies per crate: {}", stats.max_dependencies);
        println!("  max dependents per crate: {}", stats.max_dependents);
    }

    /// Display all cycles in a human-readable format
    ///
    /// by default, only shows production dependency cycles (excludes dev and build dependencies)
    /// to show all cycles including dev/build dependencies, use display_all_cycles()
    pub fn display_cycles(&self) {
        let production_cycles = self.find_production_cycles();
        let all_cycles = self.find_cycles();

        if production_cycles.is_empty() && all_cycles.is_empty() {
            println!("No cycles detected in the dependency graph.");
            return;
        }

        if production_cycles.is_empty() {
            println!("No production dependency cycles detected.");
            println!(
                "Note: {} cycle(s) detected in dev/build dependencies (not shown)",
                all_cycles.len()
            );
            println!("These are typically false positives and don't affect the build.");
            return;
        }

        println!("Found {} production dependency cycle(s):\n", production_cycles.len());

        for (i, cycle) in production_cycles.iter().enumerate() {
            println!("Cycle {}:", i + 1);

            // For each cycle, try to reconstruct the actual dependency chain
            let chain = self.reconstruct_cycle_chain(cycle);

            for (j, crate_id) in chain.iter().enumerate() {
                if j == 0 {
                    print!("  {}", crate_id.display_name());
                } else {
                    print!(" -> {}", crate_id.display_name());
                }
            }

            // Show the cycle completion (back to the first crate)
            if !chain.is_empty() {
                println!(" -> {}", chain[0].display_name());
            } else {
                println!();
            }
            println!();
        }

        if all_cycles.len() > production_cycles.len() {
            println!(
                "Note: {} additional cycle(s) in dev/build dependencies (not shown)",
                all_cycles.len() - production_cycles.len()
            );
            println!("These are typically false positives and don't affect the build.");
        }
    }

    /// Display all cycles including dev and build dependencies
    pub fn display_all_cycles(&self) {
        let cycles = self.find_cycles();

        if cycles.is_empty() {
            println!("No cycles detected in the dependency graph.");
            return;
        }

        println!("Found {} dependency cycle(s) (including dev/build):\n", cycles.len());

        for (i, cycle) in cycles.iter().enumerate() {
            println!("Cycle {}:", i + 1);

            // For each cycle, try to reconstruct the actual dependency chain
            let chain = self.reconstruct_cycle_chain(cycle);

            for (j, crate_id) in chain.iter().enumerate() {
                if j == 0 {
                    print!("  {}", crate_id.display_name());
                } else {
                    print!(" -> {}", crate_id.display_name());
                }
            }

            // Show the cycle completion (back to the first crate)
            if !chain.is_empty() {
                println!(" -> {}", chain[0].display_name());
            } else {
                println!();
            }
            println!();
        }
    }

    /// Reconstruct the actual dependency chain within a cycle
    ///
    /// given a set of crates that form a strongly connected component,
    /// this method finds an actual path that demonstrates the cycle
    fn reconstruct_cycle_chain<'a>(&'a self, cycle_crates: &[&'a CrateId]) -> Vec<&'a CrateId> {
        if cycle_crates.is_empty() {
            return Vec::new();
        }

        // Start with the first crate
        let mut chain = vec![cycle_crates[0]];
        let mut visited = std::collections::HashSet::new();
        visited.insert(cycle_crates[0]);

        // Try to build a path through all crates in the cycle
        let mut current = cycle_crates[0];

        while chain.len() < cycle_crates.len() {
            // Find a dependency of current that is in the cycle and not yet visited
            let next = self
                .get_dependencies(current)
                .into_iter()
                .find(|&dep| cycle_crates.contains(&dep) && !visited.contains(dep));

            if let Some(next_crate) = next {
                chain.push(next_crate);
                visited.insert(next_crate);
                current = next_crate;
            } else {
                // If we can't find an unvisited dependency, we might have a complex cycle
                // Just return what we have
                break;
            }
        }

        chain
    }

    /// Get statistics about the dependency graph
    pub fn get_statistics(&self) -> GraphStatistics {
        let node_count = self.graph.node_count();
        let edge_count = self.graph.edge_count();

        // Use production cycles for the main statistics
        let production_cycles = self.find_production_cycles();
        let has_cycles = !production_cycles.is_empty();
        let cycle_count = production_cycles.len();

        // Also track total cycles for informational purposes
        let all_cycles = self.find_cycles();
        let total_cycles_including_dev = all_cycles.len();

        // Calculate some basic metrics
        let mut in_degrees = HashMap::new();
        let mut out_degrees = HashMap::new();

        for crate_id in self.crates.keys() {
            let dependents = self.get_dependents(crate_id).len();
            let dependencies = self.get_dependencies(crate_id).len();

            in_degrees.insert(crate_id.clone(), dependents);
            out_degrees.insert(crate_id.clone(), dependencies);
        }

        let max_in_degree = in_degrees.values().max().copied().unwrap_or(0);
        let max_out_degree = out_degrees.values().max().copied().unwrap_or(0);

        GraphStatistics {
            crate_count: node_count,
            dependency_count: edge_count,
            has_cycles,
            cycle_count,
            total_cycles_including_dev,
            max_dependents: max_in_degree,
            max_dependencies: max_out_degree,
            workspace_count: self.workspaces.len(),
        }
    }

    /// Export graph in DOT format for visualization
    pub fn to_dot(&self) -> String {
        use std::fmt::Write;

        let mut dot = String::new();
        writeln!(&mut dot, "digraph dependency_graph {{").unwrap();
        writeln!(&mut dot, "  rankdir=LR;").unwrap();
        writeln!(&mut dot, "  node [shape=box];").unwrap();

        // Add nodes
        for crate_id in self.crates.keys() {
            let label = format!("{}\\n({})", crate_id.name, crate_id.workspace);
            writeln!(
                &mut dot,
                "  \"{}\" [label=\"{}\"];",
                crate_id.display_name(),
                label
            )
            .unwrap();
        }

        // Add edges
        for edge in self.graph.edge_references() {
            let from = &self.graph[edge.source()];
            let to = &self.graph[edge.target()];
            writeln!(
                &mut dot,
                "  \"{}\" -> \"{}\";",
                from.display_name(),
                to.display_name()
            )
            .unwrap();
        }

        writeln!(&mut dot, "}}").unwrap();
        dot
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphStatistics {
    pub crate_count: usize,
    pub dependency_count: usize,
    /// indicates if there are production dependency cycles (excludes dev/build)
    pub has_cycles: bool,
    /// number of production dependency cycles (excludes dev/build)
    pub cycle_count: usize,
    /// total cycles including dev and build dependencies
    pub total_cycles_including_dev: usize,
    pub max_dependents: usize,
    pub max_dependencies: usize,
    pub workspace_count: usize,
}

impl GraphStatistics {
    pub fn print_summary(&self) {
        println!("Dependency Graph Statistics:");
        println!("  Crates: {}", self.crate_count);
        println!("  Dependencies: {}", self.dependency_count);
        println!("  Workspaces: {}", self.workspace_count);
        println!("  Has cycles: {}", self.has_cycles);
        println!("  Cycle count: {}", self.cycle_count);
        println!("  Max dependents: {}", self.max_dependents);
        println!("  Max dependencies: {}", self.max_dependencies);
    }
}
