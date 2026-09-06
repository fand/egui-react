//! The wgpu side: one uniform buffer that is written every frame, and one
//! pipeline that is built only when the program changed.
//!
//! The `shader` example builds its pipeline in `Options::setup` and never
//! touches it again. Here the shader does not exist yet when the app starts —
//! it is whatever the patch turns out to be — so `setup` only makes the things
//! that never change (the buffer, the bind group, the layout) and the pipeline
//! is built inside `prepare`, the first time a callback arrives carrying a
//! source hash the resources have not seen.
//!
//! That check is the second half of the two-stage derivation this example is
//! about. The first stage is a `use_memo` that regenerates WGSL when the
//! topology changes; this one is "and only if the text really came out
//! different, pay for a pipeline". Moving a slider passes through neither.

use std::num::NonZeroU64;
use std::sync::Arc;

use egui_wgpu::CallbackTrait;

use crate::codegen::{self, SLOTS};

/// The uniform block, laid out the way `prelude.wgsl` declares it.
///
/// `time` and `pad` fill the first eight bytes, `resolution` the next eight,
/// and the array starts at 16 because that is what a `vec4` in a uniform block
/// has to be aligned to.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub time: f32,
    pub pad: f32,
    pub resolution: [f32; 2],
    /// One `vec4` per node in the picture, in the order `codegen` handed out
    /// the slots.
    pub params: [[f32; 4]; SLOTS],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            pad: 0.0,
            resolution: [1.0, 1.0],
            params: [[0.0; 4]; SLOTS],
        }
    }
}

/// Everything the preview owns on the GPU, parked in `callback_resources`.
///
/// Found again by type, so the gallery can run this example next to `shader`
/// and neither knows the other is there.
pub struct PatchResources {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Kept because a pipeline is built later, from a program that does not
    /// exist yet.
    pipeline_layout: wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    /// `None` until the first program arrives, and never `None` again: a
    /// program that fails to build leaves the previous picture on screen.
    pipeline: Option<wgpu::RenderPipeline>,
    /// The hash of the source `pipeline` was built from.
    source_hash: u64,
}

/// Make the parts that outlive every program. Call once, from
/// `Options::setup`.
///
/// `cc.wgpu_render_state` is `None` when eframe was built without wgpu or
/// started on glow; the app still runs and the preview simply stays empty.
pub fn setup(cc: &eframe::CreationContext<'_>) {
    let Some(render_state) = cc.wgpu_render_state.as_ref() else {
        log::warn!("patch: no wgpu render state, the preview will stay blank");
        return;
    };
    let device = &render_state.device;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("patch_uniform"),
        size: size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("patch_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size_of::<Uniforms>() as u64),
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("patch_bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("patch_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        // wgpu 30's push constants; generated programs use none.
        immediate_size: 0,
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(PatchResources {
            buffer,
            bind_group,
            pipeline_layout,
            target_format: render_state.target_format,
            pipeline: None,
            source_hash: 0,
        });
}

/// One frame of the preview: which program to draw with, and what to put in
/// its uniform buffer.
///
/// The whole path from the editor to a pixel is in these three fields. `wgsl`
/// and `source_hash` come from the topology memo, `uniforms` from the
/// parameter memo, and they travel independently — which is the claim the
/// example makes.
pub struct PatchCallback {
    /// The program to draw with. An `Arc<str>` because this struct is built on
    /// every frame while the text changes on almost none.
    pub wgsl: Arc<str>,
    pub source_hash: u64,
    pub uniforms: Uniforms,
}

impl CallbackTrait for PatchCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(res) = resources.get_mut::<PatchResources>() else {
            return Vec::new();
        };

        if res.source_hash != self.source_hash || res.pipeline.is_none() {
            let built = build(device, &res.pipeline_layout, res.target_format, &self.wgsl);
            // The hash is recorded either way, so a program that could not be
            // built is not attempted again on every frame until the user
            // changes something. A failure keeps the previous pipeline, and
            // the preview keeps drawing the last patch that worked.
            res.source_hash = self.source_hash;
            if let Some(pipeline) = built {
                res.pipeline = Some(pipeline);
            }
        }

        queue.write_buffer(&res.buffer, 0, bytemuck::bytes_of(&self.uniforms));
        // Nothing of our own to submit: the write rides egui's queue.
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        // Missing if `setup` never ran, `None` before the first program: draw
        // nothing rather than panic inside egui's render pass.
        let Some(res) = resources.get::<PatchResources>() else {
            return;
        };
        let Some(pipeline) = res.pipeline.as_ref() else {
            return;
        };
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &res.bind_group, &[]);
        // Three vertices, one triangle, clipped to the callback's rect by the
        // viewport and scissor egui-wgpu has already set.
        render_pass.draw(0..3, 0..1);
    }
}

/// Compile one generated program.
///
/// naga has already accepted this source on the CPU — `codegen::generate` will
/// not hand out a program it could not validate — and it is asked again here
/// because this is the last place where a "no" is still a log line rather than
/// a device error inside egui's render pass.
fn build(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    wgsl: &str,
) -> Option<wgpu::RenderPipeline> {
    if let Err(message) = codegen::validate(wgsl) {
        log::error!("patch: refusing to build an invalid program:\n{message}");
        return None;
    }

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("patch_program"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });

    Some(
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("patch_pipeline"),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                // No vertex buffer: the triangle comes out of `vertex_index`.
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                // The format egui itself draws into: this pipeline shares
                // egui's render pass.
                targets: &[Some(target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        }),
    )
}
