use crate::shader::*;

use crate::vertex::*;
use ash::vk::RenderPass;
use ash::{
    version::DeviceV1_0,
    vk,
    vk::{
        CommandBuffer, GraphicsPipelineCreateInfo, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineLayoutCreateInfo, PipelineViewportStateCreateInfo, Rect2D, Viewport,
    },
    Device,
};
use std::collections::HashMap;
use std::fmt::Debug;

pub struct PipeToken {
    dev: Device,
    pub desc_pool: DescPoolToken,
    pub pipe: Pipeline,
    pub layout: PipelineLayout,
    //pub push_consts: HashMap<String, PushConstant>,
    #[allow(dead_code)] // this exists just to keep artifacts from being dropped
    shaders: Vec<ShaderArtifact>,
    shader_info: Vec<vk::PipelineShaderStageCreateInfo>,
}

impl Debug for PipeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PipeToken")
            .field("desc_pool", &self.desc_pool)
            .field("pipe", &self.pipe)
            .field("layout", &self.layout)
            .finish()
    }
}

impl Drop for PipeToken {
    fn drop(&mut self) {
        eprintln!("Dropping Pipetoken");
        unsafe {
            self.dev.destroy_pipeline(self.pipe, None);
            self.dev.destroy_pipeline_layout(self.layout, None);
        }
    }
}

impl PipeToken {
    pub fn bind_sets(&self, buf: CommandBuffer) {
        unsafe {
            self.dev.cmd_bind_descriptor_sets(
                buf,
                PipelineBindPoint::GRAPHICS,
                self.layout,
                0,
                &self
                    .desc_pool
                    .sets
                    .iter()
                    .map(|s| s.set)
                    .collect::<Vec<_>>(),
                &[],
            );
        }
    }

    pub fn bind(&self, buf: CommandBuffer) {
        self.bind_sets(buf);
        unsafe {
            self.dev
                .cmd_bind_pipeline(buf, vk::PipelineBindPoint::GRAPHICS, self.pipe)
        };
    }

    pub fn recreate(&mut self, renderpass: RenderPass, viewport: Viewport, scissors: Rect2D) {
        unsafe {
            self.dev.destroy_pipeline(self.pipe, None);
        }
        self.pipe = Self::make_pipeline(
            &self.dev,
            viewport,
            scissors,
            &self.shader_info,
            self.layout,
            renderpass,
        );
    }

    pub fn build(
        dev: Device,
        pool: DescPoolToken,
        push_consts: Vec<&PushConstant>,
        renderpass: RenderPass,
        viewport: Viewport,
        scissors: Rect2D,
        shaders: HashMap<ShaderStage, ShaderArtifact>,
    ) -> Self {
        //println!("Begin pipetoken build");
        //println!("Building layouts...");
        let layout = {
            let layouts = pool.sets.iter().map(|desc| desc.layout).collect::<Vec<_>>();
            let p_consts = push_consts
                .iter()
                .map(|push| push.range)
                .collect::<Vec<_>>();
            let info = PipelineLayoutCreateInfo::builder()
                .set_layouts(&layouts)
                .push_constant_ranges(&p_consts);
            unsafe { dev.create_pipeline_layout(&info, None) }.unwrap()
        };

        //panic!("Pause");
        //dbg!(layout);
        //println!("Building shader info...");
        let shader_info = {
            // TODO - Find out whether sorting this is necessary
            let mut sort = shaders.iter().collect::<Vec<_>>();
            sort.sort_by(|(s, _a), (os, _oa)| s.cmp(os));
            sort.into_iter()
                .map(|(_s, a)| a.create_info())
                .collect::<Vec<_>>()
        };
        //dbg!(&shader_info);
        //println!("Building vertex info...");
        let pipe = Self::make_pipeline(&dev, viewport, scissors, &shader_info, layout, renderpass);

        //panic!("Pause");

        PipeToken {
            dev: dev.clone(),
            desc_pool: pool,
            //push_consts,
            pipe,
            layout,
            shaders: shaders.into_iter().map(|(_stage, shd)| shd).collect(),
            shader_info,
        }
        //panic!("Pause");
    }

    fn make_pipeline(
        dev: &Device,
        viewport: Viewport,
        scissors: Rect2D,
        shader_info: &[PipelineShaderStageCreateInfo],
        layout: PipelineLayout,
        renderpass: RenderPass,
    ) -> Pipeline {
        let (v_attr, v_bind) = (Vertex::attr_descs(), Vertex::bind_descs());
        let v_input_state = Vertex::input_state_info(&v_attr, &v_bind);
        //dbg!(unsafe{*v_input_state.p_vertex_binding_descriptions});
        let v_asm_state = Vertex::asm_state_info();
        //println!("Building other stuff...");
        let viewport_state = PipelineViewportStateCreateInfo::builder()
            .scissors(&[scissors])
            .viewports(&[viewport])
            .build();
        let raster_state = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::BACK,
            ..Default::default()
        };
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .build();
        let noop_depth = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            ..Default::default()
        };
        let depth_state = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            front: noop_depth,
            back: noop_depth,
            max_depth_bounds: 1.0,
            ..Default::default()
        };
        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: 0,
            src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ZERO,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::all(),
        }];
        let blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states)
            .build();
        let dyn_state_arr = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dyn_state = vk::PipelineDynamicStateCreateInfo::builder()
            .dynamic_states(&dyn_state_arr)
            .build();

        //dbg!(dyn_state);

        //println!("Building pipe_info...");
        let pipe_info = GraphicsPipelineCreateInfo::builder()
            .stages(&shader_info)
            .vertex_input_state(&v_input_state)
            .input_assembly_state(&v_asm_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&raster_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_state)
            .color_blend_state(&blend_state)
            .dynamic_state(&dyn_state)
            .layout(layout)
            .render_pass(renderpass)
            .build();

        //panic!("Pause");

        //dbg!(pipe_info);
        //println!("Creating pipeline...");
        unsafe {
            dev.create_graphics_pipelines(vk::PipelineCache::null(), &[pipe_info], None)
                .unwrap()
                .remove(0)
        }
    }
}
