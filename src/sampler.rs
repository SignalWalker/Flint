use ash::{version::DeviceV1_0, vk, Device};

pub struct SamplerToken {
    dev: Device,
    pub sampler: vk::Sampler,
}

impl Drop for SamplerToken {
    fn drop(&mut self) {
        eprintln!("Dropping Sampler");
        unsafe {
            self.dev.destroy_sampler(self.sampler, None);
        }
    }
}

impl SamplerToken {
    pub fn new(dev: Device) -> Self {
        let sampler_info = vk::SamplerCreateInfo {
            mag_filter: vk::Filter::NEAREST,
            min_filter: vk::Filter::NEAREST,
            mipmap_mode: vk::SamplerMipmapMode::NEAREST,
            address_mode_u: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_v: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_w: vk::SamplerAddressMode::MIRRORED_REPEAT,
            max_anisotropy: 1.0,
            border_color: vk::BorderColor::FLOAT_OPAQUE_WHITE,
            compare_op: vk::CompareOp::NEVER,
            ..Default::default()
        };
        Self {
            sampler: unsafe { dev.create_sampler(&sampler_info, None).unwrap() },
            dev,
        }
    }
}
