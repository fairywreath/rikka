use anyhow::Result;

use rikka_core::nalgebra::Matrix4;

pub const INVALID_INDEX: usize = usize::MAX;
const MAX_SCENE_LEVEL: usize = 32;

#[derive(Clone, Copy)]
pub struct Hierarchy {
    pub parent: usize,
    pub level: usize,
    pub first_child: usize,
    pub next_sibling: usize,
    pub last_sibling: usize,
}

impl Hierarchy {
    pub fn new(parent: usize, level: usize) -> Self {
        Self {
            parent,
            level,
            first_child: INVALID_INDEX,
            next_sibling: INVALID_INDEX,
            last_sibling: INVALID_INDEX,
        }
    }
}

impl Default for Hierarchy {
    fn default() -> Self {
        Self {
            parent: INVALID_INDEX,
            level: 0,
            first_child: INVALID_INDEX,
            next_sibling: INVALID_INDEX,
            last_sibling: INVALID_INDEX,
        }
    }
}

/// Scene transformation graph.
pub struct Graph {
    pub local_matrices: Vec<Matrix4<f32>>,
    pub global_matrices: Vec<Matrix4<f32>>,
    pub nodes_hierarchy: Vec<Hierarchy>,
    pub changed_nodes: [Vec<usize>; MAX_SCENE_LEVEL],
}

impl Graph {
    pub fn new() -> Self {
        Self {
            local_matrices: Vec::new(),
            global_matrices: Vec::new(),
            nodes_hierarchy: Vec::new(),
            changed_nodes: Default::default(),
        }
    }

    pub fn with_num_nodes(num_nodes: usize) -> Self {
        Self {
            local_matrices: vec![Matrix4::identity(); num_nodes],
            global_matrices: vec![Matrix4::identity(); num_nodes],
            nodes_hierarchy: vec![Hierarchy::default(); num_nodes],
            changed_nodes: Default::default(),
        }
    }

    pub fn calculate_transforms(&mut self) -> Result<()> {
        let mut num_changed_nodes = 0;
        for level in 0..MAX_SCENE_LEVEL {
            num_changed_nodes += self.changed_nodes[level].len();
        }
        log::info!("Scene graph number of changed nodes: {}", num_changed_nodes);

        for level in 0..MAX_SCENE_LEVEL {
            // if !self.changed_nodes[level].is_empty() {
            for changed_node in self.changed_nodes[level].drain(..) {
                let changed_node = changed_node;
                let parent_node = self.nodes_hierarchy[changed_node].parent;

                if parent_node != INVALID_INDEX {
                    self.global_matrices[changed_node] =
                        self.global_matrices[parent_node] * self.local_matrices[changed_node];
                } else {
                    // Root node case
                    self.global_matrices[changed_node] = self.local_matrices[changed_node];
                }
            }
            // } else {
            // break;
            // }
        }

        Ok(())
    }

    fn mark_changed(&mut self, node: usize) {
        let hierarchy = self.nodes_hierarchy[node];
        self.changed_nodes[hierarchy.level].push(node);

        let mut nodes_to_change = Vec::new();
        if hierarchy.first_child != INVALID_INDEX {
            nodes_to_change.push(hierarchy.first_child);
        }

        while let Some(current_node) = nodes_to_change.pop() {
            let current_hierarchy = self.nodes_hierarchy[current_node];

            self.changed_nodes[current_hierarchy.level].push(current_node);

            if current_hierarchy.next_sibling != INVALID_INDEX {
                nodes_to_change.push(current_hierarchy.next_sibling);
            }

            if current_hierarchy.first_child != INVALID_INDEX {
                nodes_to_change.push(current_hierarchy.first_child);
            }
        }
    }

    // XXX: Verify parent and level matches
    pub fn add_node(&mut self, parent: usize, level: usize) -> usize {
        let node = self.nodes_hierarchy.len();

        self.global_matrices.push(Matrix4::identity());
        self.local_matrices.push(Matrix4::identity());
        self.nodes_hierarchy.push(Hierarchy {
            parent,
            level,
            ..Default::default()
        });

        self.set_child_and_sibling_indices(node, parent);

        node
    }

    /// Needs to be done after all hierarchies are set
    pub fn set_local_matrix(&mut self, node: usize, matrix: Matrix4<f32>) {
        self.local_matrices[node] = matrix;
        self.mark_changed(node);
    }

    // XXX: Verify parent and level matches
    pub fn set_hierarchy(&mut self, node: usize, parent: usize, level: usize) {
        self.nodes_hierarchy[node].parent = parent;
        self.nodes_hierarchy[node].level = level;
        self.set_child_and_sibling_indices(node, parent);
    }

    /// Set parent's child and sibling indices
    fn set_child_and_sibling_indices(&mut self, node: usize, parent: usize) {
        self.nodes_hierarchy[node].next_sibling = INVALID_INDEX;

        if parent != INVALID_INDEX {
            let first_child = self.nodes_hierarchy[parent].first_child;

            if first_child == INVALID_INDEX {
                self.nodes_hierarchy[parent].first_child = node;
                self.nodes_hierarchy[node].last_sibling = node;
            } else {
                let last_sibling = self.nodes_hierarchy[first_child].last_sibling;
                assert!(last_sibling != INVALID_INDEX);

                self.nodes_hierarchy[last_sibling].next_sibling = node;

                // Update last siblings
                let mut current_node = first_child;
                while current_node != INVALID_INDEX {
                    self.nodes_hierarchy[current_node].last_sibling = node;
                    current_node = self.nodes_hierarchy[current_node].next_sibling;
                }
            }
        }
    }
}
