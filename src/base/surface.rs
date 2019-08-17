use crate::*;
use ash::version::{EntryV1_0, InstanceV1_0};
use ash::vk;

use std::ops::Drop;

pub struct SurfToken {
    pub surface: vk::SurfaceKHR,
    pub loader: Surface,
}

impl Drop for SurfToken {
    fn drop(&mut self) {
        eprintln!("Dropping SurfToken");
        unsafe { self.loader.destroy_surface(self.surface, None) }
        //eprintln!("Dropped SurfToken");
    }
}

impl SurfToken {
    pub fn new<E: EntryV1_0, I: InstanceV1_0>(
        entry: &E,
        instance: &I,
        window: &winit::Window,
    ) -> Self {
        let surface = unsafe { create_surface(entry, instance, window).unwrap() };
        let loader = Surface::new(entry, instance);
        Self { surface, loader }
    }
}
