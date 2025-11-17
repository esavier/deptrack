use petgraph::visit::EdgeRef;
use petgraph::{Directed, Graph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateId {
    pub workspace: String,
    pub name: String,
}

impl CrateId {
    pub fn new(workspace: String, name: String) -> Self {
        Self { workspace, name }
    }

    /// Create a human-readable identifier
    pub fn display_name(&self) -> String {
        format!("{}::{}", self.workspace, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateInfo {
    pub id: CrateId,
    pub version: String,
    pub path: PathBuf,
    pub cargo_toml_path: PathBuf,
}

impl CrateInfo {
    pub fn new(id: CrateId, version: String, path: PathBuf) -> Self {
        let cargo_toml_path = path.join("Cargo.toml");
        Self {
            id,
            version,
            path,
            cargo_toml_path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub root_path: PathBuf,
    pub cargo_toml_path: PathBuf,
    pub members: Vec<String>,
}

impl Workspace {
    pub fn new(name: String, root_path: PathBuf, members: Vec<String>) -> Self {
        let cargo_toml_path = root_path.join("Cargo.toml");
        Self {
            name,
            root_path,
            cargo_toml_path,
            members,
        }
    }
}

/// Simple edge data for the dependency graph
/// We remove LocalDependency struct and just use unit type or simple enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DependencyType {
    #[default]
    Normal,
    Dev,
    Build,
}

/// The main dependency graph structure
pub struct CrateDependencyGraph {
    /// All discovered crates indexed by their ID
    pub crates: HashMap<CrateId, CrateInfo>,
    /// Petgraph directed graph where nodes are CrateId and edges represent dependencies
    pub graph: Graph<CrateId, DependencyType, Directed>,
    /// Mapping from CrateId to NodeIndex for efficient graph operations
    pub node_indices: HashMap<CrateId, petgraph::graph::NodeIndex>,
    /// Root workspaces in the repository
    pub workspaces: Vec<Workspace>,
}

impl CrateDependencyGraph {
    pub fn new() -> Self {
        Self {
            crates: HashMap::new(),
            graph: Graph::new(),
            node_indices: HashMap::new(),
            workspaces: Vec::new(),
        }
    }

    /// Add a crate to the graph
    pub fn add_crate(&mut self, crate_info: CrateInfo) {
        let crate_id = crate_info.id.clone();

        // Add node to graph if not already present
        if !self.node_indices.contains_key(&crate_id) {
            let node_index = self.graph.add_node(crate_id.clone());
            self.node_indices.insert(crate_id.clone(), node_index);
        }

        // Store crate info
        self.crates.insert(crate_id, crate_info);
    }

    /// Add a dependency edge between two crates
    pub fn add_dependency(&mut self, from: &CrateId, to: &CrateId, dep_type: DependencyType) {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.node_indices.get(from), self.node_indices.get(to))
        {
            self.graph.add_edge(from_idx, to_idx, dep_type);
        }
    }

    /// Get all crates that depend on the given crate (reverse dependencies)
    pub fn get_dependents(&self, crate_id: &CrateId) -> Vec<&CrateId> {
        if let Some(&node_idx) = self.node_indices.get(crate_id) {
            self.graph
                .edges_directed(node_idx, petgraph::Direction::Incoming)
                .map(|edge| &self.graph[edge.source()])
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all crates that this crate depends on (forward dependencies)
    pub fn get_dependencies(&self, crate_id: &CrateId) -> Vec<&CrateId> {
        if let Some(&node_idx) = self.node_indices.get(crate_id) {
            self.graph
                .edges_directed(node_idx, petgraph::Direction::Outgoing)
                .map(|edge| &self.graph[edge.target()])
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if the graph has any dependency cycles
    pub fn has_cycles(&self) -> bool {
        petgraph::algo::is_cyclic_directed(&self.graph)
    }

    /// Get topological ordering of crates (for build order)
    pub fn topological_order(&self) -> Option<Vec<&CrateId>> {
        petgraph::algo::toposort(&self.graph, None)
            .ok()
            .map(|mut indices| {
                // Reverse the order so dependencies come before dependents
                indices.reverse();
                indices.iter().map(|&idx| &self.graph[idx]).collect()
            })
    }

    /// Get all crates in the graph
    pub fn all_crates(&self) -> Vec<&CrateId> {
        self.crates.keys().collect()
    }

    /// Find a crate by name (searches across all workspaces)
    pub fn find_crate_by_name(&self, name: &str) -> Option<&CrateInfo> {
        self.crates
            .values()
            .find(|crate_info| crate_info.id.name == name)
    }

    /// Get crates in a specific workspace
    pub fn get_workspace_crates(&self, workspace_name: &str) -> Vec<&CrateInfo> {
        self.crates
            .values()
            .filter(|crate_info| crate_info.id.workspace == workspace_name)
            .collect()
    }
}

impl Default for CrateDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
