use std::{any::TypeId, marker::PhantomData, mem};
use wgpu::util::DeviceExt as _;

pub struct Prototype {
    pub reference: super::MeshRef,
    type_ids: Box<[TypeId]>,
    type_infos: Box<[hecs::TypeInfo]>,
}

pub struct IndexStream {
    pub offset: wgpu::BufferAddress,
    pub format: wgpu::IndexFormat,
    pub count: u32,
}

pub struct VertexStream {
    type_id: TypeId,
    pub offset: wgpu::BufferAddress,
    pub stride: wgpu::BufferAddress,
}

//HACK: `hecs` doesn't want anybody to implement this, but we have no choice.
unsafe impl<'a> hecs::DynamicBundle for &'a Prototype {
    fn with_ids<T>(&self, f: impl FnOnce(&[TypeId]) -> T) -> T {
        f(&self.type_ids)
    }
    fn type_info(&self) -> Vec<hecs::TypeInfo> {
        self.type_infos.to_vec()
    }
    unsafe fn put(self, mut f: impl FnMut(*mut u8, hecs::TypeInfo)) {
        const DUMMY_SIZE: usize = 1;
        let mut v = [0u8; DUMMY_SIZE];
        assert!(mem::size_of::<Vertex<()>>() <= DUMMY_SIZE);
        for ts in self.type_infos.iter() {
            f(v.as_mut_ptr(), ts.clone());
        }
    }
}

pub struct Mesh {
    pub buffer: wgpu::Buffer,
    pub index_stream: Option<IndexStream>,
    vertex_streams: Box<[VertexStream]>,
    pub vertex_count: u32,
}

impl Mesh {
    pub fn vertex_stream<T: 'static>(&self) -> Option<&VertexStream> {
        self.vertex_streams
            .iter()
            .find(|vs| vs.type_id == TypeId::of::<T>())
    }
}

pub struct Vertex<T>(PhantomData<T>);

pub struct MeshBuilder<'a> {
    context: &'a mut super::Context,
    name: String,
    data: Vec<u8>, // could be moved up to the context
    index_stream: Option<IndexStream>,
    vertex_streams: Vec<VertexStream>,
    type_infos: Vec<hecs::TypeInfo>,
    vertex_count: usize,
}

impl<'a> MeshBuilder<'a> {
    pub fn new(context: &'a mut super::Context) -> Self {
        Self {
            context,
            name: String::new(),
            data: Vec::new(),
            vertex_count: 0,
            index_stream: None,
            vertex_streams: Vec::new(),
            type_infos: Vec::new(),
        }
    }

    pub fn name(self, name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..self
        }
    }

    fn append<T: bytemuck::Pod>(&mut self, data: &[T]) -> wgpu::BufferAddress {
        let offset = self.data.len();
        self.data.extend(bytemuck::cast_slice(data));
        offset as _
    }

    pub fn index(mut self, data: &[u16]) -> Self {
        assert!(self.index_stream.is_none());
        let offset = self.append(data);
        Self {
            index_stream: Some(IndexStream {
                offset,
                format: wgpu::IndexFormat::Uint16,
                count: data.len() as u32,
            }),
            ..self
        }
    }

    pub fn vertex<T: bytemuck::Pod>(mut self, data: &[T]) -> Self {
        let offset = self.append(data);
        if self.vertex_count == 0 {
            self.vertex_count = data.len();
        } else {
            assert_eq!(self.vertex_count, data.len());
        }
        self.vertex_streams.push(VertexStream {
            type_id: TypeId::of::<T>(),
            offset,
            stride: mem::size_of::<T>() as _,
        });
        self.type_infos.push(hecs::TypeInfo::of::<Vertex<T>>());
        self
    }

    pub fn build(self) -> Prototype {
        let index = self.context.meshes.len();

        let mut usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX;
        usage.set(wgpu::BufferUsages::INDEX, self.index_stream.is_some());
        let buffer = self
            .context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: if self.name.is_empty() {
                    None
                } else {
                    Some(&self.name)
                },
                contents: &self.data,
                usage,
            });

        let type_ids = self
            .vertex_streams
            .iter()
            .map(|vs| vs.type_id)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.context.meshes.push(Mesh {
            buffer,
            index_stream: self.index_stream,
            vertex_streams: self.vertex_streams.into_boxed_slice(),
            vertex_count: self.vertex_count as u32,
        });

        Prototype {
            reference: super::MeshRef(index as u32),
            type_ids,
            type_infos: self.type_infos.into_boxed_slice(),
        }
    }
}
