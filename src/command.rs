use ash::Device;
use ash::{version::DeviceV1_0, vk};

// Arch Test Topics:
// Sequential Logic
// Memory Design
// ICA Design (MIPS) :: Translate C to MIPS
// Computer Arithmetic

pub struct CmdPool {
    dev: Device,
    pub pool: vk::CommandPool,
    pub buffers: Vec<vk::CommandBuffer>,
}

impl Drop for CmdPool {
    fn drop(&mut self) {
        unsafe {
            self.dev.free_command_buffers(self.pool, &self.buffers);
            self.dev.destroy_command_pool(self.pool, None);
        }
    }
}

impl CmdPool {
    pub fn new(dev: Device, queue_fam: u32, buf_count: u32) -> Self {
        let pool = unsafe {
            let info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_fam);
            dev.create_command_pool(&info, None).unwrap()
        };
        let buffers = unsafe {
            let info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(buf_count)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);
            dev.allocate_command_buffers(&info).unwrap()
        };
        Self { dev, pool, buffers }
    }

    pub fn record<F: FnOnce(&Device, vk::CommandBuffer)>(
        &self,
        buffer: usize,
        usage: vk::CommandBufferUsageFlags,
        f: F,
    ) {
        let buffer = self.buffers[buffer];
        // self.dev
        //     .reset_command_buffer(buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES);
        let info = vk::CommandBufferBeginInfo::builder().flags(usage);
        unsafe {
            self.dev.begin_command_buffer(buffer, &info).unwrap();
            f(&self.dev, buffer);
            self.dev.end_command_buffer(buffer).unwrap();
        }
    }

    pub fn submit(
        &self,
        queue: vk::Queue,
        wait_mask: &[vk::PipelineStageFlags],
        wait_semaphores: &[vk::Semaphore],
        signal_semaphores: &[vk::Semaphore],
        buffers: &[usize],
    ) {
        let command_buffers = {
            let mut res = Vec::new();
            for i in buffers {
                res.push(self.buffers[*i]);
            }
            res
        };

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        unsafe {
            let submit_fence = self
                .dev
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .unwrap();

            self.dev
                .queue_submit(queue, &[submit_info.build()], submit_fence)
                .unwrap();
            self.dev
                .wait_for_fences(&[submit_fence], true, std::u64::MAX)
                .unwrap();
            self.dev.destroy_fence(submit_fence, None);
        }
    }
}

pub fn record_submit_commandbuffer<D: DeviceV1_0, F: FnOnce(&D, vk::CommandBuffer)>(
    device: &D,
    command_buffer: vk::CommandBuffer,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let submit_fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("Create fence failed.");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info.build()], submit_fence)
            .expect("queue submit failed.");
        device
            .wait_for_fences(&[submit_fence], true, std::u64::MAX)
            .expect("Wait for fence failed.");
        device.destroy_fence(submit_fence, None);
    }
}
