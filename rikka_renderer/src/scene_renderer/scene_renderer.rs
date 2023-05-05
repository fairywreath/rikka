use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use serde_derive::{Deserialize, Serialize};

use rikka_gpu::descriptor_set::*;
use rikka_graph::graph::Graph;

use crate::{gltf::GltfScene, renderer::*, scene, scene_renderer::types::*};

#[derive(Serialize, Deserialize)]
pub struct config {
    pub render_graph: String,
    pub render_techniques: Vec<String>,
    pub gtlf_model: String,
}

pub struct SceneRenderer {
    renderer: Renderer,
    render_graph: Graph,

    scene_graph: scene::Graph,

    render_techniques: HashMap<String, Arc<RenderTechnique>>,

    meshes: Vec<Mesh>,

    fullscreen_technique: Arc<RenderTechnique>,
    fullscreen_descriptor_set: Handle<DescriptorSet>,

    gltf_scene: GltfScene,
}

impl SceneRenderer {
    pub fn new() -> Result<Self> {
        todo!()
    }

    pub fn prepare(&mut self) -> Result<()> {
        Ok(())
    }

    /// Assigns render passes to the render graph
    fn register_render_passes(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn upload_materials(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        Ok(())
    }
}
