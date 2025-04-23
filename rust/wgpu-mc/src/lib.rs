/*!
# wgpu-mc
wgpu-mc is a pure-Rust crate which is designed to be usable by anyone who needs to render
Minecraft-style scenes using Rust. The main user of this crate at this time is the Minecraft mod
Electrum which replaces Minecraft's official renderer with wgpu-mc.
However, anyone is able to use this crate, and the API is designed to be completely independent
of any single project, allowing anyone to use it. It is mostly batteries-included, except for a
few things.

# Considerations

This crate is unstable and subject to change. The basic structure for features such
as terrain rendering and entity rendering are already in-place but could very well change significantly
in the future.

# Setup

wgpu-mc, as you could have probably guessed, uses the [wgpu](https://github.com/gfx-rs/wgpu) crate
for communicating with the GPU. Assuming you aren't running wgpu-mc headless (if you are, I assume
you already know what you're doing), wgpu-mc can handle surface and device setup for you, as long
as you pass in a valid window handle. See [init_wgpu]

# Rendering

wgpu-mc makes use of a trait called `WmPipeline` to describe any struct which is used for
rendering. There are multiple built in pipelines, but they aren't required to use while rendering.

## Terrain Rendering

The first step to begin terrain rendering is to implement [BlockStateProvider](cr).
This is a trait that provides a block state key for a given coordinate.

## Entity Rendering

To render entities, you need an entity model. wgpu-mc makes no assumptions about how entity models are defined,
so it's up to you to provide them to wgpu-mc.

See the [render::entity] module for an example of rendering an example entity.
 */

use std::borrow::Borrow;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use glam::IVec3;
use mc::chunk::BakedLayer;
use mc::Scene;
pub use minecraft_assets;
use parking_lot::{Mutex, RwLock};
pub use wgpu;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BufferDescriptor, Surface};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::mc::resource::ResourceProvider;
use crate::mc::MinecraftState;
use crate::render::atlas::Atlas;
use crate::render::pipeline::{create_bind_group_layouts, BLOCK_ATLAS, ENTITY_ATLAS};

pub mod mc;
pub mod render;
pub mod texture;
pub mod util;

pub use treeculler::Frustum;

/// Provides access to wgpu
pub struct Display {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub surface: Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: RwLock<wgpu::SurfaceConfiguration>,
}

/// Tuple of chunk positions and baked layers
pub type ChunkUpdateData = (IVec3, Vec<BakedLayer>);

/// The main wgpu-mc renderer struct
/// Resources pertaining to Minecraft go in `MinecraftState`.
///
/// `RenderGraph` is used in tandem with `World` to render scenes.
pub struct WmRenderer {
    pub gpu: Display,
    pub bind_group_layouts: Arc<HashMap<String, BindGroupLayout>>,
    pub mc: MinecraftState,
    pub chunk_update_queue: (Sender<ChunkUpdateData>, Mutex<Receiver<ChunkUpdateData>>),
}

#[derive(Copy, Clone)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

pub trait HasWindowSize {
    fn get_window_size(&self) -> WindowSize;
}

impl WmRenderer {
    pub fn new(display: Display, resource_provider: Arc<dyn ResourceProvider>) -> WmRenderer {
        let mc = MinecraftState::new(&display, resource_provider);
        let (sender, receiver) = channel();
        Self {
            bind_group_layouts: Arc::new(create_bind_group_layouts(&display.device)),
            gpu: display,
            mc,
            chunk_update_queue: (sender, Mutex::new(receiver)),
        }
    }

    pub fn init(&self) {
        let atlases = [BLOCK_ATLAS, ENTITY_ATLAS]
            .iter()
            .map(|&name| (name.into(), Atlas::new(&self.gpu, false)))
            .collect();

        *self.mc.texture_manager.atlases.write() = atlases;
    }

    pub fn upload_animated_block_buffer(&self, data: Vec<f32>) {
        let d = data.as_slice();

        let buf = self.mc.animated_block_buffer.borrow().load_full();

        if buf.is_none() {
            let animated_block_buffer = self.gpu.device.create_buffer(&BufferDescriptor {
                label: None,
                size: (d.len() * 8) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let animated_block_bind_group =
                self.gpu.device.create_bind_group(&BindGroupDescriptor {
                    label: None,
                    layout: self.bind_group_layouts.get("ssbo").unwrap(),
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(
                            animated_block_buffer.as_entire_buffer_binding(),
                        ),
                    }],
                });

            self.mc
                .animated_block_buffer
                .store(Arc::new(Some(animated_block_buffer)));
            self.mc
                .animated_block_bind_group
                .store(Arc::new(Some(animated_block_bind_group)));
        }

        self.gpu.queue.write_buffer(
            (**self.mc.animated_block_buffer.load()).as_ref().unwrap(),
            0,
            bytemuck::cast_slice(d),
        );
    }

    pub fn submit_chunk_updates(&self, scene: &Scene) {
        let receiver = self.chunk_update_queue.1.lock();
        let updates = receiver.try_iter();

        updates.for_each(|(pos, layers)| {
            let mut storage = scene.section_storage.write();
            let section = storage.replace(pos, &layers);
            for (i, ranges) in section.layers.iter().enumerate() {
                if let Some(ranges) = ranges {
                    self.gpu.queue.write_buffer(
                        &scene.chunk_buffer.buffer,
                        ranges.vertex_range.start as u64 * 4,
                        &layers[i].vertices,
                    );
                    self.gpu.queue.write_buffer(
                        &scene.chunk_buffer.buffer,
                        ranges.index_range.start as u64 * 4,
                        &layers[i].indices,
                    );
                }
            }
        });
    }

    pub fn get_backend_description(&self) -> String {
        format!(
            "wgpu {} ({})",
            env!("WGPUMC_WGPU_VER"),
            self.gpu.adapter.get_info().backend.to_str()
        )
    }
}
