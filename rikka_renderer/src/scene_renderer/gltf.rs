use std::{collections::VecDeque, mem::size_of, path::PathBuf, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use ddsfile::DxgiFormat;
use gltf::{material::AlphaMode, Gltf};

use rikka_core::{
    nalgebra::{Matrix4, Vector3, Vector4},
    vk,
};
use rikka_gpu::{buffer::*, descriptor_set::*, escape::Handle, gpu::Gpu, image::*, sampler::*};

use crate::{loader::asynchronous::*, renderer::*, scene, scene_renderer::types::*};

pub struct GltfScene {
    pub meshes: Vec<Mesh>,
    pub scene_graph: scene::Graph,
}

fn dxgi_format_to_vulkan_format(dxgi_format: DxgiFormat) -> vk::Format {
    match dxgi_format {
        DxgiFormat::BC1_UNorm => vk::Format::BC1_RGBA_UNORM_BLOCK,
        DxgiFormat::BC1_UNorm_sRGB => vk::Format::BC1_RGBA_SRGB_BLOCK,
        DxgiFormat::BC3_UNorm => vk::Format::BC3_UNORM_BLOCK,
        DxgiFormat::BC3_UNorm_sRGB => vk::Format::BC3_SRGB_BLOCK,
        DxgiFormat::BC5_UNorm => vk::Format::BC5_UNORM_BLOCK,
        _ => todo!(),
    }
}

fn gltf_min_filter_to_vulkan_filter(gltf_filter: gltf::texture::MinFilter) -> vk::Filter {
    match gltf_filter {
        gltf::texture::MinFilter::Linear
        | gltf::texture::MinFilter::LinearMipmapLinear
        | gltf::texture::MinFilter::LinearMipmapNearest => vk::Filter::LINEAR,

        gltf::texture::MinFilter::Nearest
        | gltf::texture::MinFilter::NearestMipmapLinear
        | gltf::texture::MinFilter::NearestMipmapNearest => vk::Filter::NEAREST,
    }
}

fn gltf_mag_filter_to_vulkan_filter(gltf_filter: gltf::texture::MagFilter) -> vk::Filter {
    match gltf_filter {
        gltf::texture::MagFilter::Linear => vk::Filter::LINEAR,
        gltf::texture::MagFilter::Nearest => vk::Filter::NEAREST,
    }
}

impl GltfScene {
    fn create_image(
        renderer: &mut Renderer,
        file_name: &str,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Handle<Image>> {
        let mut data = std::io::Cursor::new(std::fs::read(file_name)?);
        let mut image_desc = ImageDesc::new(0, 0, 0);

        // XXX: How slow is this read?
        if let Ok(dds) = ddsfile::Dds::read(&mut data) {
            let mut vulkan_format = vk::Format::UNDEFINED;

            if let Some(format) = dds.get_dxgi_format() {
                vulkan_format = dxgi_format_to_vulkan_format(format);
            } else if let Some(format) = dds.get_d3d_format() {
                todo!()
            }

            image_desc = ImageDesc::new(dds.get_width(), dds.get_height(), 1)
                .set_format(vulkan_format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
        } else {
            let reader = image::io::Reader::open(file_name)?;

            // XXX: Use proper format instead of always converting to R8G8B8A_UNORM?
            // let format = reader.format()?;
            let format = vk::Format::R8G8B8A8_UNORM;

            let (width, height) = reader.into_dimensions()?;

            image_desc = ImageDesc::new(width, height, 1)
                .set_format(format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
        }

        let image = renderer.create_image(image_desc)?;
        // XXX: Do this internally in the GPU
        renderer
            .gpu_mut()
            .add_bindless_image_update(rikka_gpu::types::ImageResourceUpdate {
                frame: 0,
                image: Some(image.clone()),
                sampler: None,
            });
        async_loader.request_image_file_load(file_name, image.clone());
        Ok(image)
    }

    fn load_images(
        renderer: &mut Renderer,
        root_path_buf: &PathBuf,
        images: gltf::iter::Images,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Vec<Handle<Image>>> {
        let mut gpu_images = Vec::with_capacity(images.len());

        let image_loading_start_time = Instant::now();

        for image in images {
            let gpu_image = match image.source() {
                gltf::image::Source::Uri { uri, .. } => {
                    let mut uri_path = root_path_buf.clone();
                    uri_path.push(uri);
                    Self::create_image(renderer, uri_path.to_str().unwrap(), async_loader)
                }
                gltf::image::Source::View { view, .. } => {
                    panic!("glTF image loading from view not implemented!");
                }
            }?;

            gpu_images.push(gpu_image);
        }

        let image_loading_end_time = Instant::now();
        let image_loading_dt = image_loading_end_time - image_loading_start_time;
        log::info!("Image loading total time: {:?}", image_loading_dt);

        Ok(gpu_images)
    }

    fn load_samplers(
        renderer: &Renderer,
        samplers: gltf::iter::Samplers,
    ) -> Result<Vec<Handle<Sampler>>> {
        let mut gpu_samplers = Vec::with_capacity(samplers.len());

        for sampler in samplers {
            let sampler_desc = SamplerDesc::new()
                .set_min_filter(gltf_min_filter_to_vulkan_filter(
                    sampler
                        .min_filter()
                        .unwrap_or(gltf::texture::MinFilter::Linear),
                ))
                .set_mag_filter(gltf_mag_filter_to_vulkan_filter(
                    sampler
                        .mag_filter()
                        .unwrap_or(gltf::texture::MagFilter::Linear),
                ));

            let gpu_sampler = renderer.create_sampler(sampler_desc)?;
            gpu_samplers.push(gpu_sampler);
        }

        Ok(gpu_samplers)
    }

    fn load_buffers_data(
        root_path_buf: &PathBuf,
        buffers: gltf::iter::Buffers,
        blob: Option<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>> {
        let mut buffers_data = Vec::with_capacity(buffers.len());
        let mut blob_index = None;

        log::info!("Gltf buffers length: {}", buffers.len());

        for buffer in buffers {
            let data = match buffer.source() {
                gltf::buffer::Source::Bin => {
                    blob_index = Some(buffer.index());
                    Vec::<u8>::new()
                }
                gltf::buffer::Source::Uri(uri) => {
                    let mut uri_path = root_path_buf.clone();
                    uri_path.push(uri);

                    let binary_data = std::fs::read(uri_path).context("Failed to read gltf uri")?;
                    binary_data
                }
            };

            buffers_data.push(data);
        }

        if let Some(blob_index) = blob_index {
            buffers_data[blob_index] = blob.expect("Global blob not found");
        }

        Ok(buffers_data)
    }

    /// Creates GPU buffers based on buffer views
    fn load_buffer_views(
        renderer: &mut Renderer,
        buffer_views: gltf::iter::Views,
        buffers_data: &[Vec<u8>],
    ) -> Result<Vec<Handle<Buffer>>> {
        let mut gpu_buffers = Vec::with_capacity(buffer_views.len());

        log::info!("Buffer views length {}", buffer_views.len());

        for buffer_view in buffer_views {
            let length = buffer_view.length();
            let range_start = buffer_view.offset();
            let range_end = range_start + length;

            let data = &buffers_data[buffer_view.buffer().index()][range_start..range_end];

            let staging_buffer = renderer.create_buffer(
                BufferDesc::new()
                    .set_size(length as _)
                    .set_device_only(false),
            )?;
            staging_buffer.copy_data_to_buffer(data)?;

            let gpu_buffer = renderer.create_buffer(
                BufferDesc::new()
                    .set_size(length as _)
                    .set_usage_flags(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    )
                    .set_device_only(true),
            )?;
            renderer
                .gpu_mut()
                .copy_buffer(&staging_buffer, &gpu_buffer)?;

            gpu_buffers.push(Handle::from(gpu_buffer));
        }

        Ok(gpu_buffers)
    }

    fn create_default_pbr_material(
        renderer: &Renderer,
        render_technique: Arc<RenderTechnique>,
        uniform_buffer: Handle<Buffer>,
    ) -> Result<PBRMaterial> {
        let material_desc =
            MaterialDesc::new(0, render_technique.clone(), String::from("pbr_lighting"));
        let material = renderer.create_material(material_desc)?;

        // XXX: Use dynamic uniform buffer?
        let material_buffer_desc = BufferDesc::new()
            .set_usage_flags(vk::BufferUsageFlags::UNIFORM_BUFFER)
            .set_size(size_of::<GPUMeshData>() as _)
            .set_device_only(false);
        // log::info!("Material buffer size: {}", material_buffer_desc.size);
        let material_buffer = renderer.create_buffer(material_buffer_desc)?;

        // let mesh_data = GPUMeshData {
        //     diffuse_texture_index: 3,
        //     metallic_roughness_texture_index: 3,
        //     normal_texture_index: 3,
        //     base_color_factor: Vector4::new(0.67, 0.67, 0.67, 1.0),
        // };
        // material_buffer.copy_data_to_buffer(&[mesh_data])?;

        // XXX: Use accessprs fpr a lot of the structs instead of public mbembers
        let descriptor_set_layout = render_technique.passes[0]
            .graphics_pipeline
            .descriptor_set_layouts()[0]
            .clone();
        let descriptor_set_desc = DescriptorSetDesc::new(descriptor_set_layout)
            .add_buffer_resource(uniform_buffer, 0)
            .add_buffer_resource(material_buffer.clone(), 1);
        let descriptor_set = renderer.create_descriptor_set(descriptor_set_desc)?;

        Ok(PBRMaterial::new(material, material_buffer, descriptor_set))
    }

    fn get_material_texture_image(
        gltf_texture: gltf::Texture,
        gpu_images: &Vec<Handle<Image>>,
        gpu_samplers: &Vec<Handle<Sampler>>,
    ) -> Handle<Image> {
        let image = gpu_images[gltf_texture.source().index()].clone();

        if let Some(sampler_index) = gltf_texture.sampler().index() {
            image.set_linked_sampler(gpu_samplers[sampler_index].clone());
        } else {
            // XXX: Create mew sampler here? or use default GPU sampler?
            todo!()
        }

        image
    }

    fn create_pbr_material(
        gltf_material: gltf::Material,
        gpu_images: &Vec<Handle<Image>>,
        gpu_samplers: &Vec<Handle<Sampler>>,
        renderer: &Renderer,
        render_technique: Arc<RenderTechnique>,
        uniform_buffer: Handle<Buffer>,
    ) -> Result<PBRMaterial> {
        let mut pbr_material =
            Self::create_default_pbr_material(renderer, render_technique, uniform_buffer)?;

        // Alpha mode
        match gltf_material.alpha_mode() {
            AlphaMode::Mask => {
                pbr_material.draw_flags |= DrawFlags::ALPHA_MASK;
            }
            AlphaMode::Blend => {
                pbr_material.draw_flags |= DrawFlags::TRANSPARENT;
            }
            AlphaMode::Opaque => {}
        }

        // Alpha cutoff
        if let Some(alpha_cutoff) = gltf_material.alpha_cutoff() {
            pbr_material.alpha_cutoff = alpha_cutoff;
        } else {
            pbr_material.alpha_cutoff = INVALID_FLOAT_VALUE;
        }

        // Double sideness
        if gltf_material.double_sided() {
            pbr_material.draw_flags |= DrawFlags::DOUBLE_SIDED;
        }

        let gltf_pbr_material = gltf_material.pbr_metallic_roughness();

        // Base color, metallic and roughness factors
        pbr_material.base_color_factor = gltf_pbr_material.base_color_factor().into();
        pbr_material.metallic_roughness_occlusion_factor.x = gltf_pbr_material.metallic_factor();
        pbr_material.metallic_roughness_occlusion_factor.y = gltf_pbr_material.roughness_factor();

        // Occlusion texture and factor
        if let Some(occlusion_info) = gltf_material.occlusion_texture() {
            let image = Self::get_material_texture_image(
                occlusion_info.texture(),
                gpu_images,
                gpu_samplers,
            );
            pbr_material.occlusion_image = Some(image);

            pbr_material.metallic_roughness_occlusion_factor.z = occlusion_info.strength();
        } else {
            // log::warn!(
            //     "Material {} has no occlusion texture",
            //     gltf_material.name().unwrap()
            // );
            pbr_material.metallic_roughness_occlusion_factor.z = INVALID_FLOAT_VALUE;
        }

        // Diffuse or base color texture
        if let Some(diffuse_info) = gltf_pbr_material.base_color_texture() {
            let image =
                Self::get_material_texture_image(diffuse_info.texture(), gpu_images, gpu_samplers);
            pbr_material.diffuse_image = Some(image);
        } else {
            log::warn!(
                "Material {} has no base color texture",
                gltf_material.name().unwrap()
            );
        }

        // Metallic roughness texture
        if let Some(metallic_roughness_info) = gltf_pbr_material.metallic_roughness_texture() {
            let image = Self::get_material_texture_image(
                metallic_roughness_info.texture(),
                gpu_images,
                gpu_samplers,
            );
            pbr_material.metallic_roughness_image = Some(image);
        } else {
            // log::warn!(
            //     "Material {} has no metallic roughness texture",
            //     gltf_material.name().unwrap()
            // );
        }

        // Normal texture
        if let Some(normal_info) = gltf_material.normal_texture() {
            let image =
                Self::get_material_texture_image(normal_info.texture(), gpu_images, gpu_samplers);
            pbr_material.normal_image = Some(image);
        } else {
            // log::warn!(
            //     "Material {} has no normal texture",
            //     gltf_material.name().unwrap()
            // );
        }

        Ok(pbr_material)
    }

    pub fn new_from_file(
        renderer: &mut Renderer,
        file_name: &str,
        uniform_buffer: &Handle<Buffer>,
        render_technique: &Arc<RenderTechnique>,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Self> {
        let mut root_path_buf = PathBuf::from(file_name);
        // XXX: Assume asset paths are exactly on the same directory from the `.gLTF` file
        //      Handle this more gracefully
        root_path_buf.pop();

        let mut gltf_file = Gltf::open(file_name)?;

        let gpu_images =
            Self::load_images(renderer, &root_path_buf, gltf_file.images(), async_loader)?;

        let gpu_samplers = Self::load_samplers(renderer, gltf_file.samplers())?;

        let gltf_blob = gltf_file.blob.take();
        let buffers_data =
            GltfScene::load_buffers_data(&root_path_buf, gltf_file.buffers(), gltf_blob)?;

        log::info!("Buffers data length {}", buffers_data[0].len());

        let gpu_buffers = Self::load_buffer_views(renderer, gltf_file.views(), &buffers_data)?;

        let gltf_meshes = gltf_file.meshes();
        let mut meshes = Vec::with_capacity(gltf_meshes.len());

        log::info!("Meshes count: {}", gltf_meshes.len());

        // gLTF model can have multiple scene, but right now only 1 scene (the default one) is used
        // let gltf_scenes = gltf_file.scenes().collect::<Vec<_>>();

        // XXX: Do topological sort approach?
        let gltf_nodes = gltf_file.nodes();
        log::debug!("gLTF number of nodes: {}", gltf_nodes.len());

        let mut scene_graph = scene::Graph::with_num_nodes(gltf_nodes.len());

        // Set scene graph hierarchy with level order traversal/BFS
        let root_scene = gltf_file.default_scene().unwrap();
        log::debug!("gLTF default scene: {}", root_scene.index());

        let mut nodes_to_visit = VecDeque::new();
        for node in root_scene.nodes() {
            scene_graph.set_hierarchy(node.index(), scene::INVALID_INDEX, 0);
            nodes_to_visit.push_back(node);
        }

        // Create mesh data while traversing the scene graph
        while !nodes_to_visit.is_empty() {
            let node = nodes_to_visit.pop_front().unwrap();

            // Find to set this now as all nodes are traversed in a BFS manner
            let transform_matrix = Matrix4::from(node.transform().matrix());
            scene_graph.set_local_matrix(node.index(), transform_matrix);

            for child in node.children() {
                scene_graph.set_hierarchy(
                    child.index(),
                    node.index(),
                    scene_graph.nodes_hierarchy[node.index()].level + 1,
                );
                nodes_to_visit.push_back(child);
            }

            let gltf_mesh = node.mesh().unwrap();
            for primitive in gltf_mesh.primitives() {
                let pbr_material = Self::create_pbr_material(
                    primitive.material(),
                    &gpu_images,
                    &gpu_samplers,
                    renderer,
                    render_technique.clone(),
                    uniform_buffer.clone(),
                )?;

                let mut mesh = Mesh::new_with_pbr_material(pbr_material);

                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(anyhow!(
                        "glTF primitive mode is not TRIANGLES, only TRIANGLES is supported"
                    ));
                }

                if let Some(positions_accessor) = primitive.get(&gltf::Semantic::Positions) {
                    let buffer_view = positions_accessor.view().unwrap();
                    mesh.position_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh.position_offset = positions_accessor.offset() as _;
                } else {
                    return Err(anyhow!("glTF positions accessor does not exist!"));
                }

                if let Some(indices_accessor) = primitive.indices() {
                    let buffer_view = indices_accessor.view().unwrap();
                    mesh.index_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh.index_offset = indices_accessor.offset() as _;
                    mesh.primitive_count = indices_accessor.count() as _;
                } else {
                    return Err(anyhow!("glTF indices accessor does not exist!"));
                }

                if let Some(tex_coords_accessor) = primitive.get(&gltf::Semantic::TexCoords(0)) {
                    let buffer_view = tex_coords_accessor.view().unwrap();
                    mesh.tex_coords_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh.tex_coords_offset = tex_coords_accessor.offset() as _;
                } else {
                    return Err(anyhow!(
                        "glTF texture coordinates 0 accessor does not exist!"
                    ));
                }

                if let Some(normals_accessor) = primitive.get(&gltf::Semantic::Normals) {
                    let buffer_view = normals_accessor.view().unwrap();
                    mesh.normal_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh.normal_offset = normals_accessor.offset() as _;
                } else {
                    return Err(anyhow!("glTF normals accessor does not exist!"));
                }

                if let Some(tangents_accessor) = primitive.get(&gltf::Semantic::Tangents) {
                    let buffer_view = tangents_accessor.view().unwrap();
                    mesh.tangent_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh.tangent_offset = tangents_accessor.offset() as _;
                } else {
                    log::info!("Does not contain tangents! index {}", primitive.index());
                }

                mesh.scene_graph_node_index = node.index();

                meshes.push(mesh);
            }
        }

        Ok(Self {
            meshes,
            scene_graph,
        })
    }
}
