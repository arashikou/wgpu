use crate::{
    AdapterHandle,
    AdapterId,
    BindGroupHandle,
    BindGroupId,
    BindGroupLayoutHandle,
    BindGroupLayoutId,
    BufferHandle,
    BufferId,
    CommandBufferHandle,
    CommandBufferId,
    ComputePassHandle,
    ComputePassId,
    ComputePipelineHandle,
    ComputePipelineId,
    DeviceHandle,
    DeviceId,
    Epoch,
    Index,
    PipelineLayoutHandle,
    PipelineLayoutId,
    RenderPassHandle,
    RenderPassId,
    RenderPipelineHandle,
    RenderPipelineId,
    SamplerHandle,
    SamplerId,
    ShaderModuleHandle,
    ShaderModuleId,
    SurfaceHandle,
    SurfaceId,
    TextureHandle,
    TextureId,
    TextureViewHandle,
    TextureViewId,
    TypedId,
};
#[cfg(not(feature = "gfx-backend-gl"))]
use crate::{InstanceHandle, InstanceId};
use lazy_static::lazy_static;
#[cfg(feature = "local")]
use parking_lot::Mutex;
use parking_lot::RwLock;
use vec_map::VecMap;

use std::{fmt, ops, sync::Arc};

/// A simple structure to manage identities of objects.
#[derive(Debug)]
pub struct IdentityManager<I: TypedId> {
    free: Vec<Index>,
    epochs: Vec<Epoch>,
    phantom: std::marker::PhantomData<I>,
}

impl<I: TypedId> Default for IdentityManager<I> {
    fn default() -> IdentityManager<I> {
        IdentityManager {
            free: Default::default(),
            epochs: Default::default(),
            phantom: std::marker::PhantomData,
        }
    }
}

impl<I: TypedId> IdentityManager<I> {
    pub fn alloc(&mut self) -> I {
        match self.free.pop() {
            Some(index) => I::new(index, self.epochs[index as usize]),
            None => {
                let id = I::new(self.epochs.len() as Index, 1);
                self.epochs.push(id.epoch());
                id
            }
        }
    }

    pub fn free(&mut self, id: I) {
        let (index, epoch) = (id.index(), id.epoch());
        // avoid doing this check in release
        if cfg!(debug_assertions) {
            assert!(!self.free.contains(&index));
        }
        let pe = &mut self.epochs[index as usize];
        assert_eq!(*pe, epoch);
        *pe += 1;
        self.free.push(index);
    }
}

#[derive(Debug)]
pub struct Storage<T, I: TypedId> {
    //TODO: consider concurrent hashmap?
    map: VecMap<(T, Epoch)>,
    _phantom: std::marker::PhantomData<I>,
}

impl<T, I: TypedId> ops::Index<I> for Storage<T, I> {
    type Output = T;
    fn index(&self, id: I) -> &T {
        let (ref value, epoch) = self.map[id.index() as usize];
        assert_eq!(epoch, id.epoch());
        value
    }
}

impl<T, I: TypedId> ops::IndexMut<I> for Storage<T, I> {
    fn index_mut(&mut self, id: I) -> &mut T {
        let (ref mut value, epoch) = self.map[id.index() as usize];
        assert_eq!(epoch, id.epoch());
        value
    }
}

impl<T, I: TypedId> Storage<T, I> {
    pub fn contains(&self, id: I) -> bool {
        match self.map.get(id.index() as usize) {
            Some(&(_, epoch)) if epoch == id.epoch() => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Registry<T, I: TypedId> {
    #[cfg(feature = "local")]
    identity: Mutex<IdentityManager<I>>,
    data: RwLock<Storage<T, I>>,
}

impl<T, I: TypedId> Default for Registry<T, I> {
    fn default() -> Self {
        Registry {
            #[cfg(feature = "local")]
            identity: Mutex::new(IdentityManager::default()),
            data: RwLock::new(Storage {
                map: VecMap::new(),
                _phantom: std::marker::PhantomData,
            }),
        }
    }
}

impl<T, I: TypedId> ops::Deref for Registry<T, I> {
    type Target = RwLock<Storage<T, I>>;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T, I: TypedId> ops::DerefMut for Registry<T, I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T, I: TypedId + Copy> Registry<T, I> {
    pub fn register(&self, id: I, value: T) {
        let old = self
            .data
            .write()
            .map
            .insert(id.index() as usize, (value, id.epoch()));
        assert!(old.is_none());
    }

    #[cfg(feature = "local")]
    pub fn register_local(&self, value: T) -> I {
        let id = self.identity.lock().alloc();
        self.register(id, value);
        id
    }

    pub fn unregister(&self, id: I) -> T {
        let (value, epoch) = self.data.write().map.remove(id.index() as usize).unwrap();
        assert_eq!(epoch, id.epoch());
        //Note: careful about the order here!
        #[cfg(feature = "local")]
        self.identity.lock().free(id);
        value
    }
}

#[derive(Default)]
pub struct Hub {
    #[cfg(not(feature = "gfx-backend-gl"))]
    pub instances: Arc<Registry<InstanceHandle, InstanceId>>,

    pub surfaces: Arc<Registry<SurfaceHandle, SurfaceId>>,
    pub adapters: Arc<Registry<AdapterHandle, AdapterId>>,
    pub devices: Arc<Registry<DeviceHandle, DeviceId>>,
    pub pipeline_layouts: Arc<Registry<PipelineLayoutHandle, PipelineLayoutId>>,
    pub bind_group_layouts: Arc<Registry<BindGroupLayoutHandle, BindGroupLayoutId>>,
    pub bind_groups: Arc<Registry<BindGroupHandle, BindGroupId>>,
    pub shader_modules: Arc<Registry<ShaderModuleHandle, ShaderModuleId>>,
    pub command_buffers: Arc<Registry<CommandBufferHandle, CommandBufferId>>,
    pub render_pipelines: Arc<Registry<RenderPipelineHandle, RenderPipelineId>>,
    pub compute_pipelines: Arc<Registry<ComputePipelineHandle, ComputePipelineId>>,
    pub render_passes: Arc<Registry<RenderPassHandle, RenderPassId>>,
    pub compute_passes: Arc<Registry<ComputePassHandle, ComputePassId>>,
    pub buffers: Arc<Registry<BufferHandle, BufferId>>,
    pub textures: Arc<Registry<TextureHandle, TextureId>>,
    pub texture_views: Arc<Registry<TextureViewHandle, TextureViewId>>,
    pub samplers: Arc<Registry<SamplerHandle, SamplerId>>,
}

lazy_static! {
    pub static ref HUB: Hub = Hub::default();
}

impl fmt::Debug for Hub {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Hub")
            .field("surfaces", &self.surfaces)
            .field("devices", &self.devices)
            .field("pipeline_layouts", &self.pipeline_layouts)
            .field("bind_group_layouts", &self.bind_group_layouts)
            .field("bind_groups", &self.bind_groups)
            .field("shader_modules", &self.shader_modules)
            .field("command_buffers", &self.command_buffers)
            .field("render_pipelines", &self.render_pipelines)
            .field("compute_pipelines", &self.compute_pipelines)
            .field("render_passes", &self.render_passes)
            .field("compute_passes", &self.compute_passes)
            .field("buffers", &self.buffers)
            .field("textures", &self.textures)
            .field("texture_views", &self.texture_views)
            .field("samplers", &self.samplers)
            .finish()
    }
}
