use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use parking_lot::Mutex;

use crate::{graph::*, types::*};

pub struct Builder {
    // XXX: Split input and output resources
    resource_cache: ResourceCache,
    node_cache: NodeCache,
    render_pass_cache: RenderPassCache,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            resource_cache: ResourceCache {
                resources: ResourcePool::new(),
                resource_map: HashMap::new(),
            },
            node_cache: NodeCache {
                nodes: ResourcePool::new(),
                node_map: HashMap::new(),
            },
            render_pass_cache: RenderPassCache {
                render_pass_map: HashMap::new(),
            },
        }
    }

    pub fn register_render_pass(&mut self, render_pass: Box<dyn RenderPass>) {
        self.render_pass_cache
            .render_pass_map
            .insert(render_pass.name().to_string(), render_pass);
    }

    pub fn create_input(&mut self, desc: InputDesc) -> ResourceHandle {
        let (resource, resource_index) = self.resource_cache.resources.get_new();
        let resource_handle = ResourceHandle::new(resource_index);

        resource
            .set_type(desc.resource_type)
            .set_name(desc.name)
            .set_producer(ResourceHandle::invalid())
            .set_output(ResourceHandle::invalid())
            .set_ref_count(0);

        resource_handle
    }

    pub fn create_output(&mut self, desc: OutputDesc, producer: NodeHandle) -> ResourceHandle {
        let (resource, resource_index) = self.resource_cache.resources.get_new();
        let resource_handle = ResourceHandle::new(resource_index);

        resource.set_type(desc.resource_type).set_name(desc.name);

        if desc.resource_type != ResourceType::Reference {
            resource
                .set_info(desc.info)
                .set_output(resource_handle)
                .set_producer(producer);

            self.resource_cache
                .resource_map
                .insert(resource.name.clone(), resource_handle.index);
        }

        resource_handle
    }

    fn access_node_mut(&mut self, handle: NodeHandle) -> &mut Node {
        self.node_cache.nodes.get_mut(handle.index).unwrap()
    }

    pub fn create_node(&mut self, desc: NodeDesc) -> NodeHandle {
        let node_index = self.node_cache.nodes.new_index();
        let node_handle = NodeHandle::new(node_index);

        self.access_node_mut(node_handle)
            .set_name(desc.name.clone())
            .set_enable(desc.enabled);

        self.node_cache
            .node_map
            .insert(desc.name, node_handle.index);

        for output_desc in desc.outputs {
            let output_resource_handle = self.create_output(output_desc, node_handle.clone());
            self.access_node_mut(node_handle)
                .outputs
                .push(output_resource_handle);
        }

        for input_desc in desc.inputs {
            let input_resource_handle = self.create_input(input_desc);
            self.access_node_mut(node_handle)
                .inputs
                .push(input_resource_handle);
        }

        node_handle
    }

    pub fn access_resource_by_handle(&self, handle: &ResourceHandle) -> Result<&Resource> {
        self.resource_cache.resources.get(handle.index).map_or(
            Err(anyhow::anyhow!("Failed to access resource")),
            |resource| Ok(resource),
        )
    }

    pub fn access_resource_mut_by_handle(
        &mut self,
        handle: &ResourceHandle,
    ) -> Result<&mut Resource> {
        self.resource_cache.resources.get_mut(handle.index).map_or(
            Err(anyhow::anyhow!("Failed to access resource")),
            |resource| Ok(resource),
        )
    }

    pub fn access_resource_by_name(&self, name: &str) -> Result<&Resource> {
        if let Some(index) = self.resource_cache.resource_map.get(name) {
            self.access_resource_by_handle(&ResourceHandle::new(*index))
        } else {
            Err(anyhow::anyhow!("Failed to retrieve node by name"))
        }
    }

    pub fn access_resource_mut_by_name(&mut self, name: &str) -> Result<&mut Resource> {
        if let Some(index) = self.resource_cache.resource_map.get(name) {
            self.access_resource_mut_by_handle(&ResourceHandle::new(*index))
        } else {
            Err(anyhow::anyhow!("Failed to retrieve node by name"))
        }
    }

    pub fn access_node_by_handle(&self, handle: &NodeHandle) -> Result<&Node> {
        self.node_cache
            .nodes
            .get(handle.index)
            .map_or(Err(anyhow::anyhow!("Failed to access resource")), |node| {
                Ok(node)
            })
    }

    pub fn access_node_mut_by_handle(&mut self, handle: &NodeHandle) -> Result<&mut Node> {
        self.node_cache
            .nodes
            .get_mut(handle.index)
            .map_or(Err(anyhow::anyhow!("Failed to access resource")), |node| {
                Ok(node)
            })
    }

    pub fn access_node_by_name(&self, name: &str) -> Result<&Node> {
        if let Some(index) = self.node_cache.node_map.get(name) {
            self.access_node_by_handle(&NodeHandle::new(*index))
        } else {
            Err(anyhow::anyhow!("Failed to retrieve node by name"))
        }
    }

    pub fn access_node_mut_by_name(&mut self, name: &str) -> Result<&mut Node> {
        if let Some(index) = self.node_cache.node_map.get(name) {
            self.access_node_mut_by_handle(&NodeHandle::new(*index))
        } else {
            Err(anyhow::anyhow!("Failed to retrieve node by name"))
        }
    }

    pub fn build(self, nodes: Vec<NodeHandle>) -> Graph {
        Graph::new(self, nodes)
    }
}
