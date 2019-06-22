use crate::buffer::*;
use crate::shader::*;
use ash::{
    version::DeviceV1_0,
    vk,
    vk::{
        DescriptorPool, DescriptorSet, DescriptorSetAllocateInfo, DescriptorSetLayout,
        DescriptorSetLayoutBinding, DescriptorType, ShaderStageFlags,
    },
    Device,
};
use spirv_cross::{glsl, *};
use std::fmt::Debug;
use std::iter::FromIterator;

#[derive(Copy, Clone, Debug)]
pub enum DescWriteInfo {
    Buf(vk::DescriptorBufferInfo),
    Img(vk::DescriptorImageInfo),
    Tex(vk::BufferView),
}

#[derive(Debug, Clone)]
pub struct Descriptor {
    //pub size: vk::DeviceSize,
    pub set: u32,
    pub binding: u32,
    pub name: String,
    pub ty: DescriptorType,
    pub count: usize,
    pub stage: ShaderStageFlags,
    pub fields: HashMap<String, DescField>,
}

#[derive(Debug, Clone)]
pub struct DescField {
    pub index: usize,
    pub offset: vk::DeviceSize,
    pub size: vk::DeviceSize,
    pub ty: DescriptorType,
    pub count: usize,
}

impl DescField {
    fn from_desc_res<T>(
        ast: &spirv::Ast<T>,
        block_type: &spirv::Type,
        res: &spirv::Resource,
        min_offset: u64,
    ) -> HashMap<String, Self>
    where
        T: spirv::Target,
        spirv::Ast<T>: spirv::Compile<T> + spirv::Parse<T>,
    {
        let mut fields = HashMap::new();
        if let spirv::Type::Struct { member_types, .. } = block_type {
            //dbg!(ast.get_declared_struct_size(res.type_id).unwrap());
            //dbg!(ast.get_declared_struct_size(res.base_type_id).unwrap());
            for (i, type_id) in member_types.iter().enumerate() {
                fields.insert(
                    ast.get_member_name(res.base_type_id, i as _).unwrap(),
                    Self::new(ast, res, *type_id, i, min_offset),
                );
            }
        }
        fields
    }

    fn new<T>(
        ast: &spirv::Ast<T>,
        res: &spirv::Resource,
        type_id: u32,
        index: usize,
        min_offset: u64,
    ) -> Self
    where
        T: spirv::Target,
        spirv::Ast<T>: spirv::Compile<T> + spirv::Parse<T>,
    {
        //dbg!(ast.get_member_decoration(block_id, index as _, spirv::Decoration::));
        let (ty, count) = cross_to_ash(&ast.get_type(type_id).unwrap());
        //dbg!(ast.get_declared_struct_member_size(res.type_id, index as _));
        let mut offset = u64::from(
            ast.get_member_decoration(res.base_type_id, index as _, spirv::Decoration::Offset)
                .unwrap(),
        );
        let size = ast
            .get_declared_struct_member_size(res.type_id, index as _)
            .unwrap();
        // Offset must be multiple of min_offset
        let modulo = offset % min_offset;
        if modulo != 0 {
            offset += min_offset - modulo;
        }
        //dbg!(ast.get_member_decoration(res.base_type_id, index as _, spirv::Decoration::MatrixStride));
        //dbg!(offset);
        DescField {
            index,
            offset,
            size: size.into(),
            ty,
            count,
        }
    }
}

impl Into<DescriptorSetLayoutBinding> for Descriptor {
    fn into(self) -> DescriptorSetLayoutBinding {
        DescriptorSetLayoutBinding {
            binding: self.binding,
            descriptor_type: self.ty,
            descriptor_count: self.count as _,
            stage_flags: self.stage,
            p_immutable_samplers: std::ptr::null(), // TODO - Whatever this is
        }
    }
}

impl Descriptor {
    pub fn make_buffer(
        &self,
        dev: Device,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
    ) -> Buffer {
        use Buffer::*;
        if self.fields.is_empty() {
            panic!("Not Implemented: Buffer from non-struct descriptor");
        // Raw(BufToken::with_size(
        //     dev,
        //     vk::BufferUsageFlags::UNIFORM_BUFFER,
        //     vk::SharingMode::EXCLUSIVE,
        //     mem_prop,
        //     self.size,
        // ))
        } else {
            Struct(BufStruct::from_fields(
                dev,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::SharingMode::EXCLUSIVE,
                mem_prop,
                self.fields
                    .iter()
                    .map(|(id, field)| (id.clone(), field.offset, field.size)),
            ))
        }
    }

    pub fn new<T>(
        ast: &spirv::Ast<T>,
        stage: ShaderStageFlags,
        res: &spirv::Resource,
        min_offset: vk::DeviceSize,
    ) -> Self
    where
        T: spirv::Target,
        spirv::Ast<T>: spirv::Compile<T> + spirv::Parse<T>,
    {
        // TODO :: Repack UBOs to optimize memory (Because min_offset might not be a factor of the sum of sizes of UBO fields)
        let ty = ast.get_type(res.type_id).unwrap();
        let (desc_type, count) = cross_to_ash(&ty);
        let set = ast
            .get_decoration(res.id, spirv::Decoration::DescriptorSet)
            .unwrap();
        Descriptor {
            //size: size.into(),
            set,
            name: res.name.clone(),
            binding: ast
                .get_decoration(res.id, spirv::Decoration::Binding)
                .unwrap(),
            ty: desc_type,
            count,
            stage,
            fields: DescField::from_desc_res(&ast, &ty, &res, min_offset),
        }
    }

    pub fn size(&self) -> vk::DescriptorPoolSize {
        vk::DescriptorPoolSize {
            ty: self.ty,
            descriptor_count: self.count as _,
        }
    }

    pub fn make_write(&self, set: DescriptorSet, info: &DescWriteInfo) -> vk::WriteDescriptorSet {
        use DescWriteInfo::*;
        let mut res = vk::WriteDescriptorSet {
            dst_set: set,
            dst_binding: self.binding,
            descriptor_count: self.count as _,
            descriptor_type: self.ty,
            ..Default::default()
        };
        match info {
            Buf(buf) => res.p_buffer_info = buf,
            Img(img) => res.p_image_info = img,
            Tex(tex) => res.p_texel_buffer_view = tex,
        }
        res
    }
}

pub fn cross_to_ash(c: &spirv::Type) -> (DescriptorType, usize) {
    use spirv::Type::*;
    //dbg!(c);
    let mut res = match c {
        Image { array } => (DescriptorType::SAMPLED_IMAGE, array.len()),
        SampledImage { array } => (DescriptorType::COMBINED_IMAGE_SAMPLER, array.len()),
        Sampler { array } => (DescriptorType::SAMPLER, array.len()),
        Struct { array, .. } => (DescriptorType::UNIFORM_BUFFER, array.len()),
        Unknown | Void => (DescriptorType::UNIFORM_BUFFER, 1),
        Boolean { array }
        | Char { array }
        | Int { array }
        | UInt { array }
        | Int64 { array }
        | UInt64 { array }
        | AtomicCounter { array }
        | Half { array }
        | Float { array }
        | Double { array }
        | SByte { array }
        | UByte { array }
        | Short { array }
        | UShort { array } => (DescriptorType::UNIFORM_BUFFER, array.len()),
        _ => unimplemented!(),
    };
    //dbg!(&res);
    res.1 = std::cmp::max(res.1, 1);
    res
}

#[derive(Debug)]
pub struct PushConstant {
    pub range: vk::PushConstantRange,
    pub fields: HashMap<String, DescField>,
}

impl PushConstant {
    pub fn write<D>(
        &self,
        dev: &Device,
        cmd_buf: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        field: &str,
        data: &[D],
    ) {
        let desc = &self.fields[field];
        let data = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
        };
        //dbg!(&data);
        {
            let data_size = std::mem::size_of_val(data);
            if data_size != desc.size as usize {
                panic!(
                    "PushConstant::write() called with size(data) != size({}): {} != {}",
                    field, data_size, desc.size
                );
            }
        }
        unsafe {
            dev.cmd_push_constants(
                cmd_buf,
                layout,
                self.range.stage_flags,
                self.range.offset + desc.offset as u32,
                data,
            )
        }
    }

    fn new<T>(
        ast: &spirv::Ast<T>,
        stage: ShaderStageFlags,
        res: &spirv::Resource,
        min_offset: vk::DeviceSize,
    ) -> Self
    where
        T: spirv::Target,
        spirv::Ast<T>: spirv::Compile<T> + spirv::Parse<T>,
    {
        let ty = ast.get_type(res.type_id).unwrap();
        let fields = DescField::from_desc_res(&ast, &ty, &res, min_offset);
        Self {
            range: vk::PushConstantRange {
                stage_flags: stage,
                size: fields
                    .iter()
                    .fold(0, |acc, (_, field)| acc + field.size as u32),
                offset: 0, // TODO :: Demagic this
            },
            fields,
        }
    }
}

pub struct DescPoolBuilder {
    pub min_offset: vk::DeviceSize,
    pub data: HashMap<u32, Vec<Descriptor>>,
    pub push_consts: HashMap<String, PushConstant>,
}

impl DescPoolBuilder {
    pub fn add(&mut self, bin: &[u32]) -> &mut Self {
        let module = spirv::Module::from_words(bin);
        let ast = spirv::Ast::<glsl::Target>::parse(&module).unwrap();

        let stage: ShaderStageFlags = {
            let native: ShaderStage = ast.get_entry_points().unwrap()[0].execution_model.into();
            native.into()
        };
        let resources = ast.get_shader_resources().unwrap();
        //dbg!(module.enumerate_descriptor_sets(None).unwrap());
        //dbg!(&resources);
        //panic!("Pause");

        for res in resources
            .uniform_buffers
            .iter()
            .chain(resources.sampled_images.iter())
            .chain(resources.separate_images.iter())
            .chain(resources.separate_samplers.iter())
            .chain(resources.storage_buffers.iter())
            .chain(resources.storage_images.iter())
        {
            let desc = Descriptor::new(&ast, stage, res, self.min_offset);
            self.data
                .entry(desc.set)
                .or_insert_with(Vec::new)
                .push(desc);
        }

        for push in resources.push_constant_buffers {
            let p = PushConstant::new(&ast, stage, &push, self.min_offset);
            dbg!(&p);
            self.push_consts.insert(push.name, p);
        }

        //dbg!(&self.data);

        self
    }

    pub fn build(self, dev: Device) -> (DescPoolToken, HashMap<String, PushConstant>) {
        let pool = unsafe {
            let sizes = self
                .data
                .iter()
                .flat_map(|(_s, b)| b.iter())
                .map(Descriptor::size)
                .collect::<Vec<_>>();
            let info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&sizes)
                .max_sets(self.data.len() as u32);
            dev.create_descriptor_pool(&info, None).unwrap()
        };

        // don't feel like explaining this; good luck
        let sets = unsafe {
            let layouts = self
                .data
                .into_iter()
                .map(|(_set_id, descs)| {
                    (
                        dev.create_descriptor_set_layout(
                            &vk::DescriptorSetLayoutCreateInfo::builder()
                                .bindings(
                                    &descs
                                        .iter()
                                        .cloned()
                                        .map(std::convert::Into::into)
                                        .collect::<Vec<DescriptorSetLayoutBinding>>(),
                                )
                                .build(),
                            None,
                        )
                        .unwrap(),
                        HashMap::from_iter(
                            descs.into_iter().map(|desc| (desc.name.to_string(), desc)),
                        ),
                    )
                })
                .collect::<Vec<_>>();
            dev.allocate_descriptor_sets(
                &DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(pool)
                    .set_layouts(
                        &layouts
                            .iter()
                            .cloned()
                            .map(|(layout, _d)| layout)
                            .collect::<Vec<_>>()[..],
                    )
                    .build(),
            )
            .unwrap()
            .into_iter()
            .zip(layouts.into_iter())
            .map(|(set, (layout, descriptors))| SetToken {
                dev: dev.clone(),
                set,
                layout,
                descriptors,
            })
            .collect::<Vec<_>>()
        };
        (DescPoolToken { dev, pool, sets }, self.push_consts)
    }
}

pub struct SetToken {
    dev: Device,
    pub set: DescriptorSet,
    pub layout: DescriptorSetLayout,
    pub descriptors: HashMap<String, Descriptor>,
}

impl Debug for SetToken {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("SetToken")
            .field("set", &self.set)
            .field("layout", &self.layout)
            .field("descriptors", &self.descriptors)
            .finish()
    }
}

impl Drop for SetToken {
    fn drop(&mut self) {
        eprintln!("Dropping set token layout {:?}", self.layout);
        unsafe {
            self.dev.destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

impl SetToken {
    pub fn make_writes(&self, info: &mut [(&str, DescWriteInfo)]) -> Vec<vk::WriteDescriptorSet> {
        info.iter_mut()
            .filter_map(|(name, info)| {
                if !self.descriptors.contains_key(*name) {
                    None
                } else {
                    Some(self.descriptors[*name].make_write(self.set, info))
                }
            })
            .collect::<Vec<_>>()
    }
}

pub struct DescPoolToken {
    dev: Device,
    pub pool: DescriptorPool,
    pub sets: Vec<SetToken>,
}

impl Debug for DescPoolToken {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("DescPoolToken")
            .field("pool", &self.pool)
            .field("sets", &self.sets)
            .finish()
    }
}

impl Drop for DescPoolToken {
    fn drop(&mut self) {
        eprintln!("Dropping descriptor pool {:?}", self.pool);
        unsafe {
            //self.pool.reset();
            self.dev.destroy_descriptor_pool(self.pool, None);
        }
    }
}

impl DescPoolToken {
    pub fn builder(min_offset: u64) -> DescPoolBuilder {
        DescPoolBuilder {
            min_offset,
            data: HashMap::new(),
            push_consts: HashMap::new(),
        }
    }

    pub fn make_buffers(
        &self,
        mem_prop: &vk::PhysicalDeviceMemoryProperties,
    ) -> HashMap<String, Buffer> {
        HashMap::from_iter(
            self.sets
                .iter()
                .flat_map(|s| s.descriptors.iter())
                .filter_map(|(id, desc)| {
                    if desc.fields.is_empty() {
                        None
                    } else {
                        Some((id.clone(), desc.make_buffer(self.dev.clone(), mem_prop)))
                    }
                }),
        )
    }

    pub fn update_desc_sets(&self, mut info: Vec<(&str, DescWriteInfo)>) {
        let writes = self
            .sets
            .iter()
            .flat_map(|set| set.make_writes(&mut info))
            .collect::<Vec<_>>();
        //dbg!(&writes);
        unsafe {
            self.dev.update_descriptor_sets(&writes, &[]);
        }
    }
}
