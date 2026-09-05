//! The wgpu side: build the pipeline once, then draw three vertices a frame.

use std::num::NonZeroU64;

use egui_wgpu::CallbackTrait;

/// What the shader reads, laid out the way WGSL expects it.
///
/// `vec2<f32>` is 8-byte aligned in a uniform block, so `resolution` lands at
/// offset 8 and `mouse` at 16; the tail padding takes the block to a multiple
/// of 16, which is what a uniform binding has to be.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    time: f32,
    speed: f32,
    resolution: [f32; 2],
    mouse: [f32; 2],
    _padding: [f32; 2],
}

/// The pipeline and its uniform buffer, parked in `callback_resources`.
///
/// One per app, built in [`setup`] and found again by type. Nothing else in
/// the process has this type, so a gallery running several examples cannot
/// collide with it.
pub struct ShaderResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
}

/// Build the pipeline and hand it to the renderer. Call once, from
/// `Options::setup`.
///
/// `cc.wgpu_render_state` is `None` when eframe was built without wgpu (or
/// started on glow), and there is nothing to build against; the app still runs,
/// the canvas is simply empty.
pub fn setup(cc: &eframe::CreationContext<'_>) {
    let Some(render_state) = cc.wgpu_render_state.as_ref() else {
        log::warn!("shader: no wgpu render state, the canvas will stay blank");
        return;
    };
    let device = &render_state.device;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader_example"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shader_example_uniform"),
        size: size_of::<Uniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shader_example_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(size_of::<Uniform>() as u64),
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shader_example_bind_group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shader_example_pipeline_layout"),
        bind_group_layouts: &[Some(&layout)],
        // `var<immediate>` is wgpu 30's push constants; the shader uses none.
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shader_example_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            // No vertex buffer: the triangle comes out of `vertex_index`.
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            // The same format egui itself draws into, because this pipeline
            // shares egui's render pass.
            targets: &[Some(render_state.target_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(ShaderResources {
            pipeline,
            bind_group,
            buffer,
        });
}

/// One frame's worth of state, on its way to the uniform buffer.
///
/// The values are read from hooks in `App` and copied in here, so the whole
/// path from a `Slider` to a pixel is: `use_state` -> this struct ->
/// `queue.write_buffer` -> WGSL.
pub struct ShaderCallback {
    /// Seconds, already multiplied by `speed` and frozen while paused.
    pub time: f32,
    /// The slider's value, so the shader can colour by it too.
    pub speed: f32,
    /// The canvas size in physical pixels.
    pub resolution: egui::Vec2,
    /// Where dragging has pushed the pattern, in pixels.
    pub mouse: egui::Vec2,
}

impl CallbackTrait for ShaderCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(res) = resources.get::<ShaderResources>() {
            queue.write_buffer(
                &res.buffer,
                0,
                bytemuck::bytes_of(&Uniform {
                    time: self.time,
                    speed: self.speed,
                    resolution: [self.resolution.x, self.resolution.y],
                    mouse: [self.mouse.x, self.mouse.y],
                    _padding: [0.0; 2],
                }),
            );
        }
        // Nothing of our own to submit: the write above rides egui's queue.
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        // Missing only if `setup` never ran; draw nothing rather than panic
        // inside egui's render pass.
        let Some(res) = resources.get::<ShaderResources>() else {
            return;
        };
        render_pass.set_pipeline(&res.pipeline);
        render_pass.set_bind_group(0, &res.bind_group, &[]);
        // Three vertices, one triangle, clipped to the callback's rect by the
        // viewport and scissor egui-wgpu has already set.
        render_pass.draw(0..3, 0..1);
    }
}
