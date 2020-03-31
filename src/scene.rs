use std::{
    slice,
    mem,
};
use ultraviolet::{Mat4, Vec3, projection::perspective_gl};

use crate::icosphere::IcoSphere;

const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const SOBEL_FILTER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

/// This includes a file as a slice of `u32`s.
/// Useful for including compiled shaders.
macro_rules! include_shader_binary {
    ($path:literal) => {{
        struct AlignedAsU32<Bytes: ?Sized> {
            _align: [u32; 0],
            bytes: Bytes,
        }

        static ALIGNED: &AlignedAsU32<[u8]> = &AlignedAsU32 {
            _align: [],
            bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/shaders/", $path)),
        };

        unsafe {
            std::slice::from_raw_parts(ALIGNED.bytes.as_ptr() as *const u32, ALIGNED.bytes.len() / 4)
        }
    }};
}

pub unsafe trait Pod: Sized {
    fn bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self as *const Self as *const u8,
                mem::size_of::<Self>(),
            )
        }
    }
}
unsafe impl<T: Pod> Pod for &[T] {
    fn bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.as_ptr() as *const u8,
                mem::size_of::<T>() * self.len(),
            )
        }
    }
}
unsafe impl Pod for u16 {}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}
unsafe impl Pod for Vertex {}

pub struct Entity {
    vertex_buffer: wgpu::Buffer,
    index_buffer: Option<wgpu::Buffer>,

    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,

    vertex_num: usize,
}

pub struct Scene {
    global_bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    normals_fbo: wgpu::Texture,
    // sobel_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,

    icosphere: Entity,
}

fn generate_matrix(aspect_ratio: f32) -> Mat4 {
    let opengl_to_wgpu_matrix: Mat4 = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.0,
        0.0, 0.0, 0.5, 1.0,
    ].into();

    let mx_projection = perspective_gl(45_f32.to_radians(), aspect_ratio, 1.0, 10.0);
    let mx_view = Mat4::look_at(
        Vec3::new(1.5, -5.0, 3.0),
        Vec3::zero(),
        Vec3::unit_z(),
    );
    let mx_correction = opengl_to_wgpu_matrix;
    mx_correction * mx_projection * mx_view
}

impl Scene {
    pub fn new(device: &wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) -> (Self, wgpu::CommandBuffer) {
        let mut encoder = device.create_command_encoder(&Default::default());

        let mx_total = generate_matrix(sc_desc.width as f32 / sc_desc.height as f32);

        let uniform_buffer = device.create_buffer_with_data(
            mx_total.as_byte_slice(),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let global_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    }
                ],
            });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &global_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_buffer,
                        range: 0 .. mem::size_of::<Mat4>() as u64,
                    },
                }
            ]
        });

        let icosphere = create_unit_icosphere_entity(device, &mut encoder, &global_bind_group_layout);

        let normals_fbo = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: sc_desc.width,
                height: sc_desc.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SOBEL_FILTER_FORMAT,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: sc_desc.width,
                height: sc_desc.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        });

        // let sobel_bind_group = {
        //     let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        //         bindings: &[
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 0,
        //                 visibility: wgpu::ShaderStage::COMPUTE,
        //                 ty: wgpu::BindingType::StorageTexture {
        //                     dimension: wgpu::TextureViewDimension::D2,
        //                     format: SOBEL_FILTER_FORMAT,
        //                     readonly: true,
        //                 },
        //             },
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 1,
        //                 visibility: wgpu::ShaderStage::COMPUTE,
        //                 ty: wgpu::BindingType::StorageTexture {
        //                     dimension: wgpu::TextureViewDimension::D2,
        //                     format: DEFAULT_FORMAT,
        //                     readonly: false,
        //                 },
        //             },
        //         ]
        //     });

        //     device.create_bind_group(&wgpu::BindGroupDescriptor {
        //         layout: &bind_group_layout,
        //         bindings: &[
        //             wgpu::Binding {
        //                 binding: 0,
        //                 resource: wgpu::BindingResource::TextureView(&normals_fbo.create_default_view()),
        //             },
        //             wgpu::Binding {
                        
        //             },
        //         ],
        //     })
        // };

        (
            Self {
                global_bind_group,
                uniform_buffer,
                normals_fbo,
                depth_texture,

                icosphere,
            },
            encoder.finish(),
        )
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, attachment: &wgpu::TextureView) {
        // let normals_view = self.normals_fbo.create_default_view();
        let depth_texture_view = self.depth_texture.create_default_view();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    wgpu::RenderPassColorAttachmentDescriptor {
                        attachment,
                        resolve_target: None,
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: wgpu::Color::WHITE,
                    },
                    // wgpu::RenderPassColorAttachmentDescriptor {
                    //     attachment: &normals_view,
                    //     resolve_target: None,
                    //     load_op: wgpu::LoadOp::Clear,
                    //     store_op: wgpu::StoreOp::Store,
                    //     clear_color: wgpu::Color::TRANSPARENT,
                    // },
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &depth_texture_view,
                    depth_load_op: wgpu::LoadOp::Clear,
                    depth_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    stencil_load_op: wgpu::LoadOp::Clear,
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_stencil: 0,
                }),
            });

            render_pass.set_pipeline(&self.icosphere.render_pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);
            // render_pass.set_bind_group(1, &self.icosphere.bind_group, &[]);
            render_pass.set_vertex_buffer(0, &self.icosphere.vertex_buffer, 0, 0);
            // render_pass.set_bind_group(index, bind_group, offsets)
            render_pass.draw(0 .. self.icosphere.vertex_num as u32, 0..10_000);
        }

        // {
        //     let mut compute_pass = encoder.begin_compute_pass();

        // }
    }
}

fn create_unit_icosphere_entity(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, global_bind_group_layout: &wgpu::BindGroupLayout) -> Entity {
    let vert_shader = include_shader_binary!("icosphere.vert");
    let frag_shader = include_shader_binary!("icosphere.frag");

    let vert_module = device.create_shader_module(vert_shader);
    let frag_module = device.create_shader_module(frag_shader);

    let icosphere = IcoSphere::new();

    let vertex_buffer = {
        let staging_buffer = device
            .create_buffer_with_data(icosphere.vertices().bytes(), wgpu::BufferUsage::COPY_SRC);
        
        let buffer_size = icosphere.vertices().len() * mem::size_of::<Vertex>();

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: buffer_size as u64,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });
        
        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &vertex_buffer,
            0,
            buffer_size as u64,
        );

        vertex_buffer
    };
    
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[],
    });
    
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[global_bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vert_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &frag_module,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleStrip,
        color_states: &[
            wgpu::ColorStateDescriptor {
                format: DEFAULT_FORMAT,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            },
            // wgpu::ColorStateDescriptor {
            //     format: SOBEL_FILTER_FORMAT,
            //     color_blend: wgpu::BlendDescriptor::REPLACE,
            //     alpha_blend: wgpu::BlendDescriptor::REPLACE,
            //     write_mask: wgpu::ColorWrite::ALL,
            // },
        ],
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil_front: wgpu::StencilStateFaceDescriptor::default(),
            stencil_back: wgpu::StencilStateFaceDescriptor::default(),
            stencil_read_mask: !0,
            stencil_write_mask: !0,
        }),
        index_format: wgpu::IndexFormat::Uint16,
        vertex_buffers: &[wgpu::VertexBufferDescriptor {
            stride: mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    format: wgpu::VertexFormat::Float3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttributeDescriptor {
                    format: wgpu::VertexFormat::Float3,
                    offset: 4 * 3,
                    shader_location: 1,
                },
            ],
        }],
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    Entity {
        vertex_buffer,
        index_buffer: None,

        bind_group,
        render_pipeline,

        vertex_num: icosphere.vertices().len(),
    }
}

// fn create_cube_entity(device: &wgpu::Device) -> Entity {
    

//     let vertex_buffer = device
//         .create_buffer_with_data(vertex_data.as_slice().bytes(), wgpu::BufferUsage::VERTEX);
//     let index_buffer = device
//         .create_buffer_with_data(index_data.as_slice().bytes(), wgpu::BufferUsage::INDEX);
    
//     let bind_group_layout =
//         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//             bindings: &[wgpu::BindGroupLayoutEntry {
//                 binding: 0,
//                 visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
//                 ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//             }],
//         });
    

//     Entity {
//         vertex_buffer,
//         index_buffer,


//     }
// }