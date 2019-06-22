use ash::vk;
use lightcycle::na::{Matrix4, Point2, Point3, Vector2};
use std::mem;
use std::ops::Add;
use std::ops::Mul;

pub mod model;

pub trait VertInfo {
    fn bind_descs() -> Vec<vk::VertexInputBindingDescription>;
    fn attr_descs() -> Vec<vk::VertexInputAttributeDescription>;
    fn input_state_info(
        attr: &[vk::VertexInputAttributeDescription],
        bind: &[vk::VertexInputBindingDescription],
    ) -> vk::PipelineVertexInputStateCreateInfo;
    fn asm_state_info() -> vk::PipelineInputAssemblyStateCreateInfo;
}

#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pos: Point3<f32>,
    uv: Point2<f32>,
}

impl Mul<Matrix4<f32>> for Vertex {
    type Output = Self;
    fn mul(mut self, m: Matrix4<f32>) -> Self {
        self.pos = Point3::from_homogeneous(m * self.pos.to_homogeneous()).unwrap();
        self
    }
}

impl Add<Vector2<f32>> for Vertex {
    type Output = Self;
    fn add(mut self, v: Vector2<f32>) -> Self {
        self.uv += v;
        self
    }
}

impl From<[f32; 5]> for Vertex {
    fn from(arr: [f32; 5]) -> Self {
        Self {
            pos: Point3::new(arr[0], arr[1], arr[2]),
            uv: Point2::new(arr[3], arr[4]),
        }
    }
}

// #[derive(Copy, Clone, PartialEq, Debug)]
// pub struct Vertex {
//     pos: [f32; 3],
//     uv: [f32; 2],
// }

impl VertInfo for Vertex {
    fn bind_descs() -> Vec<vk::VertexInputBindingDescription> {
        vec![vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }]
    }

    fn attr_descs() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                // Format looks like a color, but it's actually a
                // description of how the attribute is formatted.
                // R32G32B32_SFLOAT because it's a Vec3 of f32
                format: vk::Format::R32G32B32_SFLOAT,
                offset: offset_of!(Self, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Self, uv) as u32,
            },
        ]
    }

    fn input_state_info(
        attr: &[vk::VertexInputAttributeDescription],
        bind: &[vk::VertexInputBindingDescription],
    ) -> vk::PipelineVertexInputStateCreateInfo {
        vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_attribute_descriptions(attr)
            .vertex_binding_descriptions(bind)
            .build()
    }

    fn asm_state_info() -> vk::PipelineInputAssemblyStateCreateInfo {
        vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        }
    }
}

#[rustfmt::skip]
pub fn quad() -> [Vertex; 4] {
    [
        Vertex { pos: [ -1.0,  1.0, 0.0 ].into(), uv: [0.0, 0.044].into() },
        Vertex { pos: [  1.0,  1.0, 0.0 ].into(), uv: [0.044, 0.044].into() },
        Vertex { pos: [  1.0, -1.0, 0.0 ].into(), uv: [0.044, 0.0].into() },
        Vertex { pos: [ -1.0, -1.0, 0.0 ].into(), uv: [0.0, 0.0].into() },
    ]
}

#[rustfmt::skip]
pub const QUAD_INDICES: [u16; 6] = [
    0, 1, 2,
    2, 3, 0
];
