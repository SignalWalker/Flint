use crate::buffer::BufToken;

use ash::{util::Align, version::DeviceV1_0, vk, Device};
use std::collections::HashMap;

#[derive(Debug)]
pub struct BufStruct {
    pub buf: BufToken,
    pub fields: HashMap<String, BufField>,
}

#[derive(Debug, Clone, Copy)]
pub struct BufField {
    pub offset: vk::DeviceSize,
    pub size: vk::DeviceSize,
}

impl BufStruct {
    pub fn map_write<F, D>(&self, field: &str, w: F)
    where
        F: Fn(Align<D>),
    {
        let field = self.fields[field];
        unsafe {
            w(Align::new(
                self.buf
                    .dev
                    .map_memory(
                        self.buf.mem,
                        field.offset,
                        field.size,
                        vk::MemoryMapFlags::empty(),
                    )
                    .unwrap(),
                std::mem::align_of::<D>() as u64,
                field.size,
            ));
            self.buf.dev.unmap_memory(self.buf.mem)
        }
    }

    pub fn write<D: Copy>(&self, field: &str, data: &[D]) {
        if std::mem::size_of_val(data) as u64 != self.fields[field].size {
            panic!("Input data size does not match size of field: {}.", field);
        }
        self.map_write(field, |mut align| align.copy_from_slice(data))
    }

    pub fn from_fields<I>(
        dev: Device,
        usage: vk::BufferUsageFlags,
        sharing_mode: vk::SharingMode,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
        field_iter: I,
    ) -> Self
    where
        I: Iterator<Item = (String, vk::DeviceSize, vk::DeviceSize)>,
    {
        let mut fields = HashMap::new();
        let mut far_offset = 0;
        let mut far_size = 0;
        for (id, offset, size) in field_iter {
            fields.insert(id, BufField { offset, size });
            if offset >= far_offset {
                far_offset = offset;
                far_size = size;
            }
        }
        let size = far_offset + far_size;
        let buf = BufToken::with_size(dev, usage, sharing_mode, mem_prop, size);
        BufStruct { buf, fields }
    }
}
