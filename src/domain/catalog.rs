use crate::domain::error::Result;
use crate::domain::resource::Resource;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// A representation of the system configuration as a directed acyclic graph (DAG) of resources.
///
/// `Catalog` manages the relationships and dependency order between different system components
/// (files, directories, packages, executions) to ensure safe and predictable application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    /// The target hostname/node name this catalog is intended for.
    pub node_name: String,
    /// The deployment environment (e.g., `"production"`, `"staging"`).
    pub environment: String,
    /// The directed graph containing the resources.
    pub graph: DiGraph<Resource, ()>,
    /// UNIX timestamp when the catalog was compiled.
    pub timestamp: i64,
    /// Helper map to find node index by resource ID.
    /// Excluded from serialization and rebuilt dynamically on load.
    #[serde(skip)]
    id_map: HashMap<String, NodeIndex>,
}

impl Catalog {
    /// Creates a new, empty `Catalog` for the given node and environment.
    pub fn new(node_name: String, environment: String) -> Self {
        Self {
            node_name,
            environment,
            graph: DiGraph::new(),
            timestamp: chrono::Utc::now().timestamp(),
            id_map: HashMap::new(),
        }
    }

    /// Adds a new resource to the catalog graph.
    pub fn add_resource(&mut self, resource: Resource) {
        let id = resource.id().to_string();
        let idx = self.graph.add_node(resource);
        self.id_map.insert(id, idx);
    }

    /// Explicitly declares a dependency between two resources.
    ///
    /// The resource with ID `to_id` will depend on the resource with ID `from_id`
    /// (i.e. `from_id` must execute BEFORE `to_id`).
    pub fn add_dependency(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        let from_idx = *self
            .id_map
            .get(from_id)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {}", from_id))?;
        let to_idx = *self
            .id_map
            .get(to_id)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {}", to_id))?;

        self.graph.add_edge(from_idx, to_idx, ());

        // Sync with resource struct to ensure persistence/rebuild works
        if let Some(resource) = self.graph.node_weight_mut(to_idx) {
            resource.add_dependency(from_id.to_string());
        }

        Ok(())
    }

    /// Rebuilds the internal lookup map after deserializing the catalog.
    pub fn rebuild_id_map(&mut self) {
        self.id_map.clear();
        for idx in self.graph.node_indices() {
            let id = self.graph[idx].id().to_string();
            self.id_map.insert(id, idx);
        }
    }

    /// Retrieves a reference to a resource by its unique identifier.
    pub fn get_resource(&self, id: &str) -> Option<&Resource> {
        self.id_map.get(id).map(|&idx| &self.graph[idx])
    }

    /// Populates directed graph edges based on embedded resource dependencies.
    ///
    /// Iterates over all resources in the catalog and establishes graph connections
    /// based on the IDs returned by `dependencies()`. Missing dependencies result in warnings
    /// but do not halt the process.
    pub fn build_edges(&mut self) {
        // Clear existing edges to avoid duplicates.
        // This makes the operation idempotent and safe to call multiple times,
        // e.g. after adding more resources.
        self.graph.clear_edges();

        let mut edges_to_add = Vec::new();
        let mut missing_deps_count = 0;

        debug!("Building graph edges from resource dependencies...");

        for idx in self.graph.node_indices() {
            let resource = &self.graph[idx];
            let resource_id = resource.id().to_string();

            for dep_id in resource.dependencies() {
                if let Some(&dep_idx) = self.id_map.get(dep_id) {
                    // Dependency: resource -> dep (resource depends on dep)
                    // Edge direction: dep -> resource
                    // topological sort usually gives order where dep comes BEFORE resource.
                    // If edge is dep -> resource, then dep comes first.
                    edges_to_add.push((dep_idx, idx));
                } else {
                    // Log warning for missing dependency.
                    // This often happens if dependencies are conditional or external.
                    warn!(
                        resource_id = %resource_id,
                        missing_dependency = %dep_id,
                        "Skipping missing dependency for resource"
                    );
                    missing_deps_count += 1;
                }
            }
        }

        let edge_count = edges_to_add.len();
        for (from, to) in edges_to_add {
            self.graph.add_edge(from, to, ());
        }

        debug!(
            edges_added = edge_count,
            missing_dependencies = missing_deps_count,
            "Graph edges built successfully"
        );
    }

    /// Returns a flat vector of all resources defined in this catalog.
    pub fn resources(&self) -> Vec<Resource> {
        self.graph
            .node_indices()
            .map(|idx| self.graph[idx].clone())
            .collect()
    }

    /// Sorts the resources topologically, producing a valid linear execution order.
    ///
    /// Returns an error if a circular dependency is detected.
    pub fn topological_sort(&self) -> Result<Vec<Resource>> {
        match toposort(&self.graph, None) {
            Ok(indices) => Ok(indices
                .into_iter()
                .map(|idx| self.graph[idx].clone())
                .collect()),
            Err(cycle) => {
                let resource_id = self.graph[cycle.node_id()].id();
                Err(anyhow::anyhow!(
                    "Circular dependency detected involving: {}",
                    resource_id
                ))
            }
        }
    }

    /// Returns a sub-catalog (subgraph branch) containing the specified root resource and all its dependencies.
    pub fn get_branch(&self, root_id: &str) -> Result<Catalog> {
        let root_idx = *self
            .id_map
            .get(root_id)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {}", root_id))?;

        let mut branch = Catalog::new(self.node_name.clone(), self.environment.clone());
        let mut visited = HashMap::new();

        self.copy_recursive(root_idx, &mut branch, &mut visited)?;

        Ok(branch)
    }

    fn copy_recursive(
        &self,
        idx: NodeIndex,
        target: &mut Catalog,
        visited: &mut HashMap<NodeIndex, NodeIndex>,
    ) -> Result<NodeIndex> {
        if let Some(&new_idx) = visited.get(&idx) {
            return Ok(new_idx);
        }

        let resource = &self.graph[idx];
        target.add_resource(resource.clone());
        let new_idx = *target.id_map.get(resource.id()).ok_or_else(|| {
            anyhow::anyhow!(
                "Invariant failed: resource {} not found after addition",
                resource.id()
            )
        })?;
        visited.insert(idx, new_idx);

        // Copy edges (dependencies)
        for edge in self.graph.edges_directed(idx, petgraph::Direction::Incoming) {
            let source_node_idx = edge.source();
            let new_source_idx = self.copy_recursive(source_node_idx, target, visited)?;
            target.graph.add_edge(new_source_idx, new_idx, ());
        }

        Ok(new_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::resource::{MetaKind, MetaResource, Resource};

    fn make_meta_resource(id: &str, deps: Vec<&str>) -> Resource {
        Resource::Meta(MetaResource {
            id: id.to_string(),
            kind: MetaKind::ModuleStart,
            dependencies: deps.into_iter().map(String::from).collect(),
        })
    }

    #[test]
    fn test_topological_sort_success() -> crate::domain::error::Result<()> {
        let mut catalog = Catalog::new("test-node".to_string(), "test-env".to_string());

        // Setup resources: res1 (no deps), res2 (depends on res1), res3 (depends on res2), res4 (depends on res1)
        catalog.add_resource(make_meta_resource("res1", vec![]));
        catalog.add_resource(make_meta_resource("res2", vec!["res1"]));
        catalog.add_resource(make_meta_resource("res3", vec!["res2"]));
        catalog.add_resource(make_meta_resource("res4", vec!["res1"]));

        catalog.build_edges();

        let sorted = catalog.topological_sort()?;
        assert_eq!(sorted.len(), 4);

        // Helper to find index in sorted vector without unwrap
        let pos = |id: &str| -> Result<usize> {
            sorted.iter()
                .position(|r| r.id() == id)
                .ok_or_else(|| anyhow::anyhow!("Resource {} not found", id))
        };

        // res1 must be evaluated before res2 and res4
        assert!(pos("res1")? < pos("res2")?);
        assert!(pos("res1")? < pos("res4")?);

        // res2 must be evaluated before res3
        assert!(pos("res2")? < pos("res3")?);

        Ok(())
    }

    #[test]
    fn test_circular_dependency_detection() -> crate::domain::error::Result<()> {
        let mut catalog = Catalog::new("test-node".to_string(), "test-env".to_string());

        // Cyclic dependency: A depends on B, B depends on A
        catalog.add_resource(make_meta_resource("resA", vec!["resB"]));
        catalog.add_resource(make_meta_resource("resB", vec!["resA"]));

        catalog.build_edges();

        let sorted = catalog.topological_sort();
        assert!(sorted.is_err());
        if let Err(e) = sorted {
            let err_msg = e.to_string();
            assert!(err_msg.contains("Circular dependency detected"));
        } else {
            return Err(anyhow::anyhow!("Expected topological sort to fail with circular dependency error"));
        }
        Ok(())
    }

    #[test]
    fn test_get_branch_recursive() -> crate::domain::error::Result<()> {
        let mut catalog = Catalog::new("test-node".to_string(), "test-env".to_string());

        // A depends on B and C, B depends on D. E is isolated.
        catalog.add_resource(make_meta_resource("A", vec!["B", "C"]));
        catalog.add_resource(make_meta_resource("B", vec!["D"]));
        catalog.add_resource(make_meta_resource("C", vec![]));
        catalog.add_resource(make_meta_resource("D", vec![]));
        catalog.add_resource(make_meta_resource("E", vec![]));

        catalog.build_edges();

        // Extract branch for A (should include A, B, C, D, but NOT E)
        let branch = catalog.get_branch("A")?;

        assert_eq!(branch.resources().len(), 4);
        assert!(branch.get_resource("A").is_some());
        assert!(branch.get_resource("B").is_some());
        assert!(branch.get_resource("C").is_some());
        assert!(branch.get_resource("D").is_some());
        assert!(branch.get_resource("E").is_none());

        // Verify that dependencies are properly reconstructed in the branch catalog
        let b_sorted = branch.topological_sort()?;
        let pos = |id: &str| -> Result<usize> {
            b_sorted.iter()
                .position(|r| r.id() == id)
                .ok_or_else(|| anyhow::anyhow!("Resource {} not found", id))
        };

        assert!(pos("D")? < pos("B")?);
        assert!(pos("B")? < pos("A")?);
        assert!(pos("C")? < pos("A")?);

        Ok(())
    }

    #[test]
    fn test_build_edges_missing_dependency() -> crate::domain::error::Result<()> {
        let mut catalog = Catalog::new("test-node".to_string(), "test-env".to_string());

        // A depends on missing resource B
        catalog.add_resource(make_meta_resource("A", vec!["B"]));

        // Should not fail, just soft warn and ignore
        catalog.build_edges();

        let sorted = catalog.topological_sort()?;
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].id(), "A");
        Ok(())
    }
}

