use ash::{
    version::DeviceV1_0,
    vk::{PipelineShaderStageCreateInfo, ShaderModule, ShaderModuleCreateInfo, ShaderStageFlags},
    Device,
};
use shaderc::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CString;
use std::iter::FromIterator;

mod descriptor;
mod pipeline;

pub use descriptor::*;
pub use pipeline::*;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum ShaderStage {
    Vert,
    Geom,
    Frag,
}

impl Ord for ShaderStage {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialOrd for ShaderStage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use ShaderStage::*;
        Some(match self {
            Vert => {
                if let Vert = other {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            }
            Frag => match other {
                Vert => Ordering::Greater,
                Frag => Ordering::Equal,
                Geom => Ordering::Less,
            },
            Geom => {
                if let Geom = other {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
        })
    }
}

impl From<ShaderKind> for ShaderStage {
    fn from(k: ShaderKind) -> Self {
        match k {
            ShaderKind::Vertex => ShaderStage::Vert,
            ShaderKind::Geometry => ShaderStage::Geom,
            ShaderKind::Fragment => ShaderStage::Frag,
            _ => panic!("Not Implemented - ShaderKind -> ShaderStage"),
        }
    }
}

impl From<spirv_cross::spirv::ExecutionModel> for ShaderStage {
    fn from(s: spirv_cross::spirv::ExecutionModel) -> Self {
        use spirv_cross::spirv::ExecutionModel::*;
        use ShaderStage::*;
        match s {
            Vertex => Vert,
            Geometry => Geom,
            Fragment => Frag,
            _ => panic!("Not Implemented :: {:?} -> ShaderStage", s),
        }
    }
}

impl Into<ShaderKind> for ShaderStage {
    fn into(self) -> ShaderKind {
        match self {
            ShaderStage::Vert => ShaderKind::Vertex,
            ShaderStage::Geom => ShaderKind::Geometry,
            ShaderStage::Frag => ShaderKind::Fragment,
        }
    }
}

impl Into<ShaderStageFlags> for ShaderStage {
    fn into(self) -> ShaderStageFlags {
        match self {
            ShaderStage::Vert => ShaderStageFlags::VERTEX,
            ShaderStage::Geom => ShaderStageFlags::GEOMETRY,
            ShaderStage::Frag => ShaderStageFlags::FRAGMENT,
        }
    }
}

impl From<&str> for ShaderStage {
    fn from(s: &str) -> Self {
        match &s.to_lowercase()[..] {
            "vert" => ShaderStage::Vert,
            "frag" => ShaderStage::Frag,
            "geom" => ShaderStage::Geom,
            _ => panic!("Not Recognized: {}", s),
        }
    }
}

pub struct MetaShader {
    pub stage: ShaderStage,
    pub entry: String,
    pub bin: Vec<u32>,
}

impl MetaShader {
    pub fn new(
        compiler: &mut Compiler,
        path: &str,
        entry: &str,
        opt: Option<&CompileOptions>,
    ) -> Self {
        let stage: ShaderStage = path.split_at(path.rfind('.').unwrap() + 1).1.into();
        let bin = {
            let name = path.split_at(path.rfind('/').unwrap_or(0)).1;
            let src = std::fs::read_to_string(path).unwrap();
            match compiler.compile_into_spirv(&src, stage.into(), name, entry, opt) {
                Ok(b) => b,
                Err(e) => panic!("{}", e),
            }
            .as_binary()
            .to_vec()
        };
        Self {
            stage,
            entry: entry.to_string(),
            bin,
        }
    }

    pub fn new_chain(
        compiler: &mut Compiler,
        info: Vec<(&str, &str)>,
        opt: Option<&CompileOptions>,
    ) -> Vec<Self> {
        info.iter()
            .map(|(path, entry)| MetaShader::new(compiler, path, entry, opt))
            .collect()
    }

    fn build(self, dev: Device) -> ShaderArtifact {
        ShaderArtifact {
            module: unsafe {
                dev.create_shader_module(&ShaderModuleCreateInfo::builder().code(&self.bin), None)
            }
            .unwrap(),
            dev,
            stage: self.stage,
            entry: CString::new(self.entry).unwrap(),
        }
    }

    pub fn build_chain(
        dev: Device,
        meta: Vec<MetaShader>,
        min_offset: u64,
    ) -> (
        HashMap<ShaderStage, ShaderArtifact>,
        DescPoolToken,
        HashMap<String, PushConstant>,
    ) {
        let (pool, push_consts) = {
            let mut builder = DescPoolToken::builder(min_offset);
            for m in meta.iter() {
                builder.add(&m.bin);
            }
            builder.build(dev.clone())
        };
        (
            HashMap::from_iter(meta.into_iter().map(|m| (m.stage, m.build(dev.clone())))),
            pool,
            push_consts,
        )
    }
}

pub struct ShaderArtifact {
    dev: Device,
    pub stage: ShaderStage,
    pub entry: CString,
    pub module: ShaderModule,
}

impl Drop for ShaderArtifact {
    fn drop(&mut self) {
        eprintln!("Dropping shader artifact {:?}", self.module);
        unsafe { self.dev.destroy_shader_module(self.module, None) };
    }
}

impl ShaderArtifact {
    pub fn create_info(&self) -> PipelineShaderStageCreateInfo {
        PipelineShaderStageCreateInfo::builder()
            .module(self.module)
            .name(&self.entry)
            .stage(self.stage.into())
            .build()
    }
}
