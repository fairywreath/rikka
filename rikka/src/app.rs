use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{Context, Result};

use rikka_core::nalgebra::{Matrix4, Vector3, Vector4};

use rikka_core::vk;
use rikka_gpu::{barriers::*, buffer::*, escape::*, gpu::*, image::*, types::*};
use rikka_graph::graph::Graph;

use rikka_renderer::{loader::asynchronous::AsynchronousLoader, scene_renderer::scene_renderer::*};

pub struct RikkaApp {
    scene_renderer: SceneRenderer,

    /// Flag to stop background thread pool
    gpu_transfers_thread_run: Arc<AtomicBool>,

    background_thread_pool: threadpool::ThreadPool,
}

impl RikkaApp {
    pub fn new(gpu_desc: GpuDesc, gltf_file_name: &str) -> Result<Self> {
        let gpu = Gpu::new(gpu_desc)?;

        let mut transfer_manager = gpu.new_transfer_manager()?;
        let mut async_loader =
            AsynchronousLoader::new(transfer_manager.new_image_upload_request_sender());

        let scene_renderer_config = Config {
            file_paths_config: FilePathsConfig {
                render_graph_file_path: String::from("data/simple_pbr_graph.json"),
                render_techniques_file_paths: Vec::new(),
                gtlf_model_file_path: String::from(gltf_file_name),
            },
            gpu,
            async_loader: &mut async_loader,
        };
        let scene_renderer = SceneRenderer::new_from_config(scene_renderer_config)?;

        let background_thread_pool = threadpool::ThreadPool::new(3);
        let gpu_transfers_thread_run = Arc::new(AtomicBool::new(true));

        let load_resources = gpu_transfers_thread_run.clone();
        background_thread_pool.execute(move || {
            while load_resources.load(Ordering::Relaxed) {
                async_loader
                    .update()
                    .expect("Async loader failed to update!");
            }
        });

        let run_transfers = gpu_transfers_thread_run.clone();
        background_thread_pool.execute(move || {
            while run_transfers.load(Ordering::Relaxed) {
                transfer_manager
                    .perform_transfers()
                    .expect("GPU transfer manager failed to update!");
            }

            log::info!("Transfer manager exeuction stopped");
            transfer_manager.destroy();
        });

        Ok(Self {
            scene_renderer,
            gpu_transfers_thread_run,
            background_thread_pool,
        })
    }

    pub fn render(&mut self) -> Result<()> {
        self.scene_renderer.render()?;
        Ok(())
    }

    pub fn prepare(&mut self) -> Result<()> {
        self.scene_renderer.upload_data_to_gpu()?;
        Ok(())
    }

    pub fn update_view(&mut self, view: &Matrix4<f32>, eye_position: &Vector3<f32>) {
        self.scene_renderer.scene_uniform_data.view = view.clone();
        self.scene_renderer.scene_uniform_data.eye_position =
            Vector4::new(eye_position.x, eye_position.y, eye_position.z, 1.0);
    }

    pub fn update_projection(&mut self, projection: &Matrix4<f32>) {
        self.scene_renderer.scene_uniform_data.projection = projection.clone();
    }
}

impl Drop for RikkaApp {
    fn drop(&mut self) {
        self.scene_renderer.wait_idle();

        self.gpu_transfers_thread_run
            .fetch_and(false, Ordering::Relaxed);

        self.background_thread_pool.join();

        log::info!("App dropped");
    }
}
