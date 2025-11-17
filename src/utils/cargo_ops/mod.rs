pub mod discovery;
pub mod graph;
pub mod integration;
pub mod types;

pub use discovery::CargoDiscovery;
pub use graph::GraphStatistics;
pub use integration::{ChangeImpactAnalysis, VersionBumpAnalysis, VersionBumpStatus};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_workspace_structure(
        base_dir: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create workspace root Cargo.toml
        let workspace_toml = r#"
[workspace]
members = ["crates/*", "tools/cli"]
"#;
        fs::write(base_dir.join("Cargo.toml"), workspace_toml)?;

        // Create crates directory
        fs::create_dir_all(base_dir.join("crates/lib1"))?;
        fs::create_dir_all(base_dir.join("crates/lib2"))?;
        fs::create_dir_all(base_dir.join("tools/cli"))?;

        // Create lib1 crate
        let lib1_toml = r#"
[package]
name = "lib1"
version = "0.1.0"

[dependencies]
"#;
        fs::write(base_dir.join("crates/lib1/Cargo.toml"), lib1_toml)?;

        // Create lib2 crate that depends on lib1
        let lib2_toml = r#"
[package]
name = "lib2"
version = "0.1.0"

[dependencies]
lib1 = { path = "../lib1" }
"#;
        fs::write(base_dir.join("crates/lib2/Cargo.toml"), lib2_toml)?;

        // Create CLI tool that depends on lib2
        let cli_toml = r#"
[package]
name = "cli"
version = "0.1.0"

[dependencies]
lib2 = { path = "../../crates/lib2" }
"#;
        fs::write(base_dir.join("tools/cli/Cargo.toml"), cli_toml)?;

        Ok(())
    }

    #[test]
    fn test_workspace_discovery() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let workspaces = CargoDiscovery::discover_workspaces(temp_dir.path()).unwrap();
        assert_eq!(workspaces.len(), 1);

        let workspace = &workspaces[0];
        assert_eq!(workspace.members.len(), 2);
        assert!(workspace.members.contains(&"crates/*".to_string()));
        assert!(workspace.members.contains(&"tools/cli".to_string()));
    }

    #[test]
    fn test_crate_discovery() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let workspaces = CargoDiscovery::discover_workspaces(temp_dir.path()).unwrap();
        let workspace = &workspaces[0];

        let crates = CargoDiscovery::discover_crates_in_workspace(workspace).unwrap();
        assert_eq!(crates.len(), 3);

        let crate_names: Vec<&str> = crates.iter().map(|c| c.id.name.as_str()).collect();
        assert!(crate_names.contains(&"lib1"));
        assert!(crate_names.contains(&"lib2"));
        assert!(crate_names.contains(&"cli"));
    }

    #[test]
    fn test_dependency_graph_construction() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        // Check that all crates are in the graph
        assert_eq!(graph.crates.len(), 3);

        // Check dependencies
        let lib1_id = CrateId::new(
            temp_dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "lib1".to_string(),
        );
        let lib2_id = CrateId::new(
            temp_dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "lib2".to_string(),
        );
        let cli_id = CrateId::new(
            temp_dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "cli".to_string(),
        );

        // lib2 should depend on lib1
        let lib2_deps = graph.get_dependencies(&lib2_id);
        assert_eq!(lib2_deps.len(), 1);
        assert_eq!(lib2_deps[0], &lib1_id);

        // cli should depend on lib2
        let cli_deps = graph.get_dependencies(&cli_id);
        assert_eq!(cli_deps.len(), 1);
        assert_eq!(cli_deps[0], &lib2_id);

        // lib1 should have no dependencies
        let lib1_deps = graph.get_dependencies(&lib1_id);
        assert_eq!(lib1_deps.len(), 0);
    }

    #[test]
    fn test_dependency_graph_analysis() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        // Test topological ordering
        let build_order = graph.get_build_order().unwrap();
        assert_eq!(build_order.len(), 3);

        // lib1 should come before lib2, lib2 should come before cli
        let lib1_pos = build_order.iter().position(|&c| c.name == "lib1").unwrap();
        let lib2_pos = build_order.iter().position(|&c| c.name == "lib2").unwrap();
        let cli_pos = build_order.iter().position(|&c| c.name == "cli").unwrap();

        assert!(lib1_pos < lib2_pos);
        assert!(lib2_pos < cli_pos);

        // Test cycle detection
        assert!(!graph.has_cycles());

        // Test statistics
        let stats = graph.get_statistics();
        assert_eq!(stats.crate_count, 3);
        assert_eq!(stats.dependency_count, 2);
        assert!(!stats.has_cycles);
    }

    #[test]
    fn test_affected_crates_analysis() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        let lib1_id = CrateId::new(
            temp_dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "lib1".to_string(),
        );

        // If lib1 changes, lib2 and cli should be affected
        let changed_crates = [lib1_id.clone()];
        let affected = graph.find_affected_crates(&changed_crates);
        assert_eq!(affected.len(), 3); // lib1 itself + lib2 + cli

        let affected_names: Vec<&str> = affected.iter().map(|c| c.name.as_str()).collect();
        assert!(affected_names.contains(&"lib1"));
        assert!(affected_names.contains(&"lib2"));
        assert!(affected_names.contains(&"cli"));
    }

    #[test]
    fn test_crate_id_display() {
        let crate_id = CrateId::new("workspace1".to_string(), "my-crate".to_string());
        assert_eq!(crate_id.display_name(), "workspace1::my-crate");
    }

    #[test]
    fn test_graph_queries() {
        let temp_dir = TempDir::new().unwrap();
        create_test_workspace_structure(temp_dir.path()).unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        // Test find_crate_by_name
        let lib1_crate = graph.find_crate_by_name("lib1");
        assert!(lib1_crate.is_some());
        assert_eq!(lib1_crate.unwrap().id.name, "lib1");

        // Test get_workspace_crates
        let workspace_name = temp_dir
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let workspace_crates = graph.get_workspace_crates(&workspace_name);
        assert_eq!(workspace_crates.len(), 3);
    }

    #[test]
    fn test_dev_dependency_cycle_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace root Cargo.toml
        let workspace_toml = r#"
[workspace]
members = ["crates/*"]
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), workspace_toml).unwrap();

        // Create crates directory
        fs::create_dir_all(temp_dir.path().join("crates/lib_main")).unwrap();
        fs::create_dir_all(temp_dir.path().join("crates/lib_derive")).unwrap();

        // Create lib_main crate that depends on lib_derive (normal dependency)
        let lib_main_toml = r#"
[package]
name = "lib_main"
version = "0.1.0"

[dependencies]
lib_derive = { path = "../lib_derive", optional = true }

[features]
default = []
derive = ["lib_derive"]
"#;
        fs::write(
            temp_dir.path().join("crates/lib_main/Cargo.toml"),
            lib_main_toml,
        )
        .unwrap();

        // Create lib_derive crate that depends on lib_main as dev-dependency (creates cycle)
        let lib_derive_toml = r#"
[package]
name = "lib_derive"
version = "0.1.0"

[dependencies]

[dev-dependencies]
lib_main = { path = "../lib_main" }
"#;
        fs::write(
            temp_dir.path().join("crates/lib_derive/Cargo.toml"),
            lib_derive_toml,
        )
        .unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        // Find all cycles (including dev dependencies)
        let all_cycles = graph.find_cycles();
        assert_eq!(
            all_cycles.len(),
            1,
            "Should detect 1 cycle when including dev-dependencies"
        );

        // Find production cycles only (excluding dev dependencies)
        let production_cycles = graph.find_production_cycles();
        assert_eq!(
            production_cycles.len(),
            0,
            "Should detect 0 production cycles (dev-dependency cycle should be excluded)"
        );

        // Test statistics
        let stats = graph.get_statistics();
        assert!(!stats.has_cycles, "has_cycles should be false (only dev-dependency cycle)");
        assert_eq!(stats.cycle_count, 0, "cycle_count should be 0 (production cycles only)");
        assert_eq!(
            stats.total_cycles_including_dev, 1,
            "total_cycles_including_dev should be 1"
        );
    }

    #[test]
    fn test_production_dependency_cycle_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace root Cargo.toml
        let workspace_toml = r#"
[workspace]
members = ["crates/*"]
"#;
        fs::write(temp_dir.path().join("Cargo.toml"), workspace_toml).unwrap();

        // Create crates directory
        fs::create_dir_all(temp_dir.path().join("crates/crate_a")).unwrap();
        fs::create_dir_all(temp_dir.path().join("crates/crate_b")).unwrap();

        // Create crate_a that depends on crate_b (normal dependency)
        let crate_a_toml = r#"
[package]
name = "crate_a"
version = "0.1.0"

[dependencies]
crate_b = { path = "../crate_b" }
"#;
        fs::write(
            temp_dir.path().join("crates/crate_a/Cargo.toml"),
            crate_a_toml,
        )
        .unwrap();

        // Create crate_b that depends on crate_a (normal dependency - creates real cycle)
        let crate_b_toml = r#"
[package]
name = "crate_b"
version = "0.1.0"

[dependencies]
crate_a = { path = "../crate_a" }
"#;
        fs::write(
            temp_dir.path().join("crates/crate_b/Cargo.toml"),
            crate_b_toml,
        )
        .unwrap();

        let graph = CrateDependencyGraph::build_from_repository(temp_dir.path()).unwrap();

        // Find all cycles
        let all_cycles = graph.find_cycles();
        assert_eq!(
            all_cycles.len(),
            1,
            "Should detect 1 cycle"
        );

        // Find production cycles only
        let production_cycles = graph.find_production_cycles();
        assert_eq!(
            production_cycles.len(),
            1,
            "Should detect 1 production cycle (real cycle in normal dependencies)"
        );

        // Test statistics
        let stats = graph.get_statistics();
        assert!(stats.has_cycles, "has_cycles should be true (production cycle exists)");
        assert_eq!(stats.cycle_count, 1, "cycle_count should be 1");
        assert_eq!(
            stats.total_cycles_including_dev, 1,
            "total_cycles_including_dev should be 1"
        );
    }
}
