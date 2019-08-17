use crate::buffer::BufToken;
use crate::vertex::Vertex;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use lightcycle::{na, volume::polyhedron::*};
use na::{Point2, Point3};
use std::path::Path;

pub struct Model {
    pub shape: Polyhedron<Vertex>,
    pub vert_buf: BufToken,
    pub ind_buf: BufToken,
}

impl From<collada::Vertex> for Vertex {
    fn from(c: collada::Vertex) -> Self {
        Self {
            pos: Point3::new(c.x as _, c.y as _, c.z as _),
            uv: Point2::origin(),
        }
    }
}

impl Model {
    pub fn new(
        dev: Device,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        shape: Polyhedron<Vertex>,
    ) -> Self {
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    pub unsafe fn draw(&self, cmd_buffer: vk::CommandBuffer) {
        let dev = &self.vert_buf.dev;
        dev.cmd_bind_vertex_buffers(cmd_buffer, 0, &[self.vert_buf.buf], &[0]);
        dev.cmd_bind_index_buffer(cmd_buffer, self.ind_buf.buf, 0, vk::IndexType::UINT16);
        dev.cmd_draw_indexed(cmd_buffer, (self.shape.faces.len() * 3) as u32, 1, 0, 0, 1);
    }

    pub fn load(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties, path: &Path) -> Self {
        use collada::{document, PrimitiveElement};
        let doc = document::ColladaDocument::from_path(path).unwrap();
        let mut obj = doc.get_obj_set().unwrap().objects.remove(0);
        let mut points: Vec<Vertex> = obj
            .vertices
            .drain(0..)
            .map(std::convert::Into::into)
            .collect();
        let uv: Vec<[f32; 2]> = obj
            .tex_vertices
            .drain(0..)
            .map(|tex| [tex.x as _, tex.y as _])
            .collect();
        let faces: Vec<[u16; 3]> = match obj.geometry.remove(0).mesh.remove(0) {
            PrimitiveElement::Polylist(_) => panic!("Not Implemented"),
            PrimitiveElement::Triangles(mut t) => {
                let mut res = Vec::new();
                for (a, b, c) in t.vertices.drain(0..) {
                    for (vert, tex, _) in &[&a, &b, &c] {
                        if let Some(i) = tex {
                            points[*vert].uv = uv[*i].into();
                        }
                    }
                    res.push([a.0 as _, b.0 as _, c.0 as _]);
                }
                res
            }
        };
        Self::new(dev, mem_prop, Polyhedron { points, faces })
    }

    pub fn cube(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let shape = Polyhedron {
            points: vec![
                // back
                [0.5, -0.5, -0.5, 0.044, 0.0].into(),
                [-0.5, -0.5, -0.5, 0.0, 0.0].into(),
                [-0.5, 0.5, -0.5, 0.0, 0.044].into(),
                [0.5, 0.5, -0.5, 0.044, 0.044].into(),
                // front
                [-0.5, -0.5, 0.5, 0.0, 0.0].into(),
                [0.5, -0.5, 0.5, 0.044, 0.0].into(),
                [0.5, 0.5, 0.5, 0.044, 0.044].into(),
                [-0.5, 0.5, 0.5, 0.0, 0.044].into(),
            ],
            faces: vec![
                // back
                [0, 1, 2],
                [2, 3, 0],
                // front
                [4, 5, 6],
                [6, 7, 4],
                // bottom
                [5, 4, 1],
                [1, 0, 5],
                // top
                [3, 2, 7],
                [7, 6, 3],
                // left
                [1, 4, 7],
                [7, 2, 1],
                // right
                [5, 0, 3],
                [3, 6, 5],
            ],
        };
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    pub fn quad(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let shape = Polyhedron {
            points: vec![
                [0.5, 0.5, -0.5, 0.044, 0.0].into(),
                [-0.5, 0.5, -0.5, 0.0, 0.0].into(),
                [-0.5, 0.5, 0.5, 0.0, 0.044].into(),
                [0.5, 0.5, 0.5, 0.044, 0.044].into(),
            ],
            faces: vec![[0, 1, 2], [2, 3, 0]],
        };
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    pub fn tri(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let shape = Polyhedron {
            points: vec![
                [0.5, -0.5, -0.5, 0.044, 0.0].into(),
                [-0.5, -0.5, -0.5, 0.0, 0.0].into(),
                [-0.5, -0.5, 0.5, 0.0, 0.044].into(),
            ],
            faces: vec![[0, 1, 2]],
        };
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    pub fn ramp_q(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let shape = Polyhedron {
            points: vec![
                [0.5, 0.5, -0.5, 0.044, 0.0].into(),
                [-0.5, 0.5, -0.5, 0.0, 0.0].into(),
                [-0.5, -0.5, 0.5, 0.0, 0.044].into(),
                [0.5, -0.5, 0.5, 0.044, 0.044].into(),
            ],
            faces: vec![[0, 1, 2], [2, 3, 0]],
        };
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    pub fn ramp_t(dev: Device, mem_prop: &vk::PhysicalDeviceMemoryProperties) -> Self {
        let shape = Polyhedron {
            points: vec![
                [0.5, 0.5, -0.5, 0.044, 0.0].into(),
                [-0.5, 0.5, -0.5, 0.0, 0.0].into(),
                [-0.5, -0.5, 0.5, 0.0, 0.044].into(),
            ],
            faces: vec![[0, 1, 2]],
        };
        let (vert_buf, ind_buf) = Self::hedron_buffers(dev, mem_prop, &shape);
        Self {
            shape,
            vert_buf,
            ind_buf,
        }
    }

    fn hedron_buffers(
        dev: Device,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        hedron: &Polyhedron<Vertex>,
    ) -> (BufToken, BufToken) {
        (
            // Vertex Buffer
            BufToken::with_data(
                dev.clone(),
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::SharingMode::EXCLUSIVE,
                mem_prop,
                &hedron.points,
            ),
            // Index
            BufToken::with_data(
                dev,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::SharingMode::EXCLUSIVE,
                mem_prop,
                &hedron.faces,
            ),
        )
    }
}
