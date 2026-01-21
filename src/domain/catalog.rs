use crate::domain::error::Result;
use crate::domain::resource::Resource;
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub node_name: String,
    pub environment: String,
    pub graph: DiGraph<Resource, ()>,
    pub timestamp: i64,
    /// Helper to find node index by resource ID (not serialized, rebuilt on load)
    #[serde(skip)]
    id_map: HashMap<String, NodeIndex>,
}

impl Catalog {
    pub fn new(node_name: String, environment: String) -> Self {
        Self {
            node_name,
            environment,
            graph: DiGraph::new(),
            timestamp: chrono::Utc::now().timestamp(),
            id_map: HashMap::new(),
        }
    }

    pub fn add_resource(&mut self, resource: Resource) {
        let id = resource.id().to_string();
        let idx = self.graph.add_node(resource);
        self.id_map.insert(id, idx);
    }

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

    /// Rebuilds the ID map after deserialization
    pub fn rebuild_id_map(&mut self) {
        self.id_map.clear();
        for idx in self.graph.node_indices() {
            let id = self.graph[idx].id().to_string();
            self.id_map.insert(id, idx);
        }
    }

    pub fn get_resource(&self, id: &str) -> Option<&Resource> {
        self.id_map.get(id).map(|&idx| &self.graph[idx])
    }

    /// Populates graph edges based on resource dependencies
    pub fn build_edges(&mut self) {
        // Clear existing edges to avoid duplicates? Or assume clean slate?
        // Safest is to collect all desired edges, and then checking if they exist is expensive O(E).
        // For now, let's assume this is called once after loading resources.
        // We can clear edges first if we want to be idempotent.
        self.graph.clear_edges();

        let mut edges_to_add = Vec::new();

        for idx in self.graph.node_indices() {
            let resource = &self.graph[idx];
            for dep_id in resource.dependencies() {
                if let Some(&dep_idx) = self.id_map.get(dep_id) {
                    // Dependency: resource -> dep (resource depends on dep)
                    // Edge direction: resource -> dep or dep -> resource?
                    // topological sort usually gives order where dep comes BEFORE resource.
                    // If edge is dep -> resource, then dep comes first.
                    // petgraph::algo::toposort returns nodes in order such that for edge a -> b, a comes before b.
                    // So if we want dep before resource, we need edge dep -> resource.
                    edges_to_add.push((dep_idx, idx));
                } else {
                    // Log warning or error? For now, we tolerate missing deps (maybe optional?)
                    // But in Puppet, missing dependency is usually an error.
                    // Let's log it if we had a logger, but we don't here.
                }
            }
        }

        for (from, to) in edges_to_add {
            self.graph.add_edge(from, to, ());
        }
    }

    pub fn resources(&self) -> Vec<Resource> {
        self.graph
            .node_indices()
            .map(|idx| self.graph[idx].clone())
            .collect()
    }

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

    /// Returns a subgraph (sub-catalog) containing the resource and all its dependencies
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
        let new_idx = *target.id_map.get(resource.id()).unwrap();
        visited.insert(idx, new_idx);

        // Copy edges (dependencies)
        for edge in self.graph.edges(idx) {
            let target_node_idx = edge.target();
            let new_target_idx = self.copy_recursive(target_node_idx, target, visited)?;
            target.graph.add_edge(new_idx, new_target_idx, ());
        }

        Ok(new_idx)
    }
}
