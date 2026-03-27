use super::unified_ir::{
    IrComponentDef, IrGroup, IrInstance, IrMaterial, IrMesh, UnifiedIR,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ImportCache {
    pub source_format: String,
    pub source_file: String,
    pub units: String,
    pub meshes: HashMap<String, CachedMesh>,
    pub instances: HashMap<String, CachedInstance>,
    pub groups: HashMap<String, CachedGroup>,
    pub component_defs: HashMap<String, CachedComponentDef>,
    pub materials: HashMap<String, CachedMaterial>,
    pub mesh_order: Vec<String>,
    pub instance_order: Vec<String>,
    pub group_order: Vec<String>,
    pub component_def_order: Vec<String>,
    pub material_order: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CachedMesh {
    pub label: String,
    pub source_index: usize,
    pub ir: IrMesh,
}

#[derive(Debug, Clone)]
pub struct CachedInstance {
    pub label: String,
    pub source_index: usize,
    pub ir: IrInstance,
}

#[derive(Debug, Clone)]
pub struct CachedGroup {
    pub label: String,
    pub source_index: usize,
    pub ir: IrGroup,
}

#[derive(Debug, Clone)]
pub struct CachedComponentDef {
    pub label: String,
    pub source_index: usize,
    pub ir: IrComponentDef,
}

#[derive(Debug, Clone)]
pub struct CachedMaterial {
    pub label: String,
    pub source_index: usize,
    pub ir: IrMaterial,
}

impl ImportCache {
    pub fn from_ir(ir: &UnifiedIR) -> Self {
        let mut cache = Self {
            source_format: ir.source_format.clone(),
            source_file: ir.source_file.clone(),
            units: ir.units.clone(),
            ..Default::default()
        };

        for (index, mesh) in ir.meshes.iter().cloned().enumerate() {
            cache.mesh_order.push(mesh.id.clone());
            cache.meshes.insert(
                mesh.id.clone(),
                CachedMesh {
                    label: format!("ir_mesh:{}", mesh.id),
                    source_index: index,
                    ir: mesh,
                },
            );
        }

        for (index, instance) in ir.instances.iter().cloned().enumerate() {
            cache.instance_order.push(instance.id.clone());
            cache.instances.insert(
                instance.id.clone(),
                CachedInstance {
                    label: format!("ir_instance:{}", instance.id),
                    source_index: index,
                    ir: instance,
                },
            );
        }

        for (index, group) in ir.groups.iter().cloned().enumerate() {
            cache.group_order.push(group.id.clone());
            cache.groups.insert(
                group.id.clone(),
                CachedGroup {
                    label: format!("ir_group:{}", group.id),
                    source_index: index,
                    ir: group,
                },
            );
        }

        for (index, component_def) in ir.component_defs.iter().cloned().enumerate() {
            cache.component_def_order.push(component_def.id.clone());
            cache.component_defs.insert(
                component_def.id.clone(),
                CachedComponentDef {
                    label: format!("ir_component_def:{}", component_def.id),
                    source_index: index,
                    ir: component_def,
                },
            );
        }

        for (index, material) in ir.materials.iter().cloned().enumerate() {
            cache.material_order.push(material.id.clone());
            cache.materials.insert(
                material.id.clone(),
                CachedMaterial {
                    label: format!("ir_material:{}", material.id),
                    source_index: index,
                    ir: material,
                },
            );
        }

        cache
    }

    pub fn mesh(&self, mesh_id: &str) -> Option<&CachedMesh> {
        self.meshes.get(mesh_id)
    }

    pub fn material(&self, material_id: &str) -> Option<&CachedMaterial> {
        self.materials.get(material_id)
    }

    pub fn meshes_in_order(&self) -> impl Iterator<Item = &CachedMesh> {
        self.mesh_order
            .iter()
            .filter_map(|mesh_id| self.meshes.get(mesh_id))
    }

    pub fn instances_in_order(&self) -> impl Iterator<Item = &CachedInstance> {
        self.instance_order
            .iter()
            .filter_map(|instance_id| self.instances.get(instance_id))
    }

    pub fn groups_in_order(&self) -> impl Iterator<Item = &CachedGroup> {
        self.group_order
            .iter()
            .filter_map(|group_id| self.groups.get(group_id))
    }

    pub fn component_defs_in_order(&self) -> impl Iterator<Item = &CachedComponentDef> {
        self.component_def_order
            .iter()
            .filter_map(|component_def_id| self.component_defs.get(component_def_id))
    }
}
