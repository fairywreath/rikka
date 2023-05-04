use rikka_gpu::{image::format_has_depth, types::RenderPassOperation};
use serde::{Deserialize, Serialize};

use anyhow::{Error, Result};
use serde_derive::{Deserialize, Serialize};

use rikka_core::vk;

use crate::{builder::*, graph, types::*};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub resource_type: ResourceType,
    pub name: String,
}

impl Into<InputDesc> for Input {
    fn into(self) -> InputDesc {
        InputDesc {
            resource_type: self.resource_type,
            name: self.name,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageDesc {
    // XXX: Change this to the actual VkFormat enum
    pub format: i32,
    pub resolution: [u32; 2],
    pub load_op: RenderPassOperation,
}

impl Into<ImageInfo> for ImageDesc {
    fn into(self) -> ImageInfo {
        let format = vk::Format::from_raw(self.format);
        let usage_flags = if format_has_depth(format) {
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
        } else {
            vk::ImageUsageFlags::COLOR_ATTACHMENT
        };

        ImageInfo {
            image: None,
            width: self.resolution[0],
            height: self.resolution[1],
            depth: 1,
            format,
            usage_flags,
            load_op: self.load_op,
        }
    }
}

impl Into<ResourceInfo> for ImageDesc {
    fn into(self) -> ResourceInfo {
        ResourceInfo {
            buffer: None,
            image: Some(self.into()),
            external: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Output {
    pub resource_type: ResourceType,
    pub name: String,
    pub image: Option<ImageDesc>,
}

impl Into<OutputDesc> for Output {
    fn into(self) -> OutputDesc {
        OutputDesc {
            resource_type: self.resource_type,
            name: self.name.clone(),
            info: if let Some(image) = self.image {
                image.into()
            } else {
                ResourceInfo::default()
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pass {
    pub name: String,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
}

impl Into<NodeDesc> for Pass {
    fn into(self) -> NodeDesc {
        NodeDesc {
            inputs: self.inputs.into_iter().map(Into::into).collect::<Vec<_>>(),
            outputs: self.outputs.into_iter().map(Into::into).collect::<Vec<_>>(),
            enabled: true,
            name: self.name,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Graph {
    pub name: String,
    pub passes: Vec<Pass>,
}

pub fn parse(graph: Graph) -> Result<graph::Graph> {
    let mut builder = Builder::new();
    let mut nodes = Vec::new();

    for pass in graph.passes {
        nodes.push(builder.create_node(pass.into()));
    }

    Ok(builder.build(nodes))
}

pub fn parse_from_string(string: &str) -> Result<graph::Graph> {
    parse(serde_json::from_str(string)?)
}

pub fn parse_from_file(file_name: &str) -> Result<graph::Graph> {
    let file_contents = std::fs::read_to_string(file_name)?;
    parse_from_string(&file_contents)
}
