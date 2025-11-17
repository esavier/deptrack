// pre-defined test scenarios

use super::repo_builder::{TestCrate, TestRepoBuilder, TestRepository, TestWorkspace};

/// pre-defined test scenarios
pub enum TestScenario {
    /// simple single workspace with 3 crates in a chain: A -> B -> C
    SimpleChain,
    /// two workspaces with cross-workspace dependencies
    CrossWorkspace,
    /// complex scenario with multiple workspaces and cross-dependencies
    Complex,
}

impl TestScenario {
    /// build a repository from a predefined scenario
    pub fn build(self) -> Result<TestRepository, Box<dyn std::error::Error>> {
        match self {
            TestScenario::SimpleChain => Self::build_simple_chain(),
            TestScenario::CrossWorkspace => Self::build_cross_workspace(),
            TestScenario::Complex => Self::build_complex(),
        }
    }

    /// simple chain: crate_A -> crate_B -> crate_C
    fn build_simple_chain() -> Result<TestRepository, Box<dyn std::error::Error>> {
        TestRepoBuilder::new()
            .workspace(
                TestWorkspace::new("workspace")
                    .crate_entry(TestCrate::new("crate_A"))
                    .crate_entry(TestCrate::new("crate_B").dependency("crate_A"))
                    .crate_entry(TestCrate::new("crate_C").dependency("crate_B")),
            )
            .build()
    }

    /// cross-workspace dependencies
    fn build_cross_workspace() -> Result<TestRepository, Box<dyn std::error::Error>> {
        TestRepoBuilder::new()
            .workspace(
                TestWorkspace::new("workspace_1")
                    .crate_entry(TestCrate::new("crate_A"))
                    .crate_entry(TestCrate::new("crate_B").dependency("crate_A")),
            )
            .workspace(
                TestWorkspace::new("workspace_2")
                    .crate_entry(
                        TestCrate::new("crate_C").dependency("crate_B"), // cross-workspace
                    )
                    .crate_entry(TestCrate::new("crate_D").dependency("crate_C")),
            )
            .build()
    }

    /// complex scenario similar to our test_repo
    fn build_complex() -> Result<TestRepository, Box<dyn std::error::Error>> {
        TestRepoBuilder::new()
            .workspace(
                TestWorkspace::new("workspace_1")
                    .crate_entry(TestCrate::new("crate_D").dependency("crate_E"))
                    .crate_entry(
                        TestCrate::new("crate_E").dependency("crate_H"), // cross-workspace
                    )
                    .crate_entry(
                        TestCrate::new("crate_F")
                            .dependency("crate_D")
                            .dependency("crate_I"), // cross-workspace
                    )
                    .crate_entry(TestCrate::new("crate_G").dependency("crate_F")),
            )
            .workspace(
                TestWorkspace::new("workspace_2")
                    .crate_entry(TestCrate::new("crate_H"))
                    .crate_entry(TestCrate::new("crate_I").dependency("crate_J"))
                    .crate_entry(TestCrate::new("crate_J").dependency("crate_K"))
                    .crate_entry(TestCrate::new("crate_K"))
                    .crate_entry(
                        TestCrate::new("crate_L")
                            .dependency("crate_D") // cross-workspace
                            .dependency("crate_M"),
                    )
                    .crate_entry(
                        TestCrate::new("crate_M").dependency("crate_G"), // cross-workspace
                    ),
            )
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::cargo_ops::CrateDependencyGraph;

    #[test]
    fn test_simple_chain_scenario() {
        let repo = TestScenario::SimpleChain.build().unwrap();

        // verify repository structure
        assert!(repo.path().exists());

        // build dependency graph
        let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();
        assert_eq!(graph.crates.len(), 3);

        // verify dependency chain
        let crate_a = graph.find_crate_by_name("crate_A").unwrap();
        let crate_b = graph.find_crate_by_name("crate_B").unwrap();
        let crate_c = graph.find_crate_by_name("crate_C").unwrap();

        let b_deps = graph.get_dependencies(&crate_b.id);
        assert_eq!(b_deps.len(), 1);
        assert_eq!(b_deps[0], &crate_a.id);

        let c_deps = graph.get_dependencies(&crate_c.id);
        assert_eq!(c_deps.len(), 1);
        assert_eq!(c_deps[0], &crate_b.id);
    }

    #[test]
    fn test_cross_workspace_scenario() {
        let repo = TestScenario::CrossWorkspace.build().unwrap();

        let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();
        assert_eq!(graph.crates.len(), 4);

        // verify cross-workspace dependency
        let crate_c = graph.find_crate_by_name("crate_C").unwrap();
        let crate_b = graph.find_crate_by_name("crate_B").unwrap();

        let c_deps = graph.get_dependencies(&crate_c.id);
        assert_eq!(c_deps.len(), 1);
        assert_eq!(c_deps[0], &crate_b.id);

        // verify they are in different workspaces
        assert_ne!(crate_c.id.workspace, crate_b.id.workspace);
    }

    #[test]
    fn test_complex_scenario() {
        let repo = TestScenario::Complex.build().unwrap();

        let graph = CrateDependencyGraph::build_from_repository(repo.path()).unwrap();
        assert_eq!(graph.crates.len(), 10);

        // verify some cross-workspace dependencies
        let crate_e = graph.find_crate_by_name("crate_E").unwrap();
        let crate_h = graph.find_crate_by_name("crate_H").unwrap();

        let e_deps = graph.get_dependencies(&crate_e.id);
        assert!(e_deps.contains(&&crate_h.id));
    }

    #[test]
    fn test_repo_modification() {
        let repo = TestScenario::SimpleChain.build().unwrap();

        // create a branch
        repo.create_branch("feature").unwrap();
        assert_eq!(repo.current_branch().unwrap(), "feature");

        // modify a file
        repo.modify_file("workspace", "crate_A", "src/lib.rs", "// modified")
            .unwrap();

        // update version
        repo.update_version("workspace", "crate_A", "0.2.0")
            .unwrap();

        // commit changes
        repo.stage_all().unwrap();
        repo.commit("Test changes").unwrap();
    }
}
