//! SketchUp C API FFI bindings（動態載入）
//!
//! 基於 SketchUp SDK headers：
//!   slapi/model/model.h
//!   slapi/model/entities.h
//!   slapi/model/face.h
//!   slapi/model/component_definition.h
//!   slapi/model/component_instance.h
//!   slapi/model/group.h
//!   slapi/model/material.h

use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use crate::SkpError;

// ─── SU 型別（opaque pointer handles）─────────────────────────────────────

#[repr(C)] #[derive(Copy, Clone)] pub struct SUModelRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUEntitiesRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUFaceRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUEdgeRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUVertexRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUMaterialRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUComponentDefinitionRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUComponentInstanceRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUGroupRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUMeshHelperRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SUStringRef { pub ptr: *mut std::ffi::c_void }
#[repr(C)] #[derive(Copy, Clone)] pub struct SULayerRef { pub ptr: *mut std::ffi::c_void }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUPoint3D { pub x: f64, pub y: f64, pub z: f64 }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUVector3D { pub x: f64, pub y: f64, pub z: f64 }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUTransformation { pub values: [f64; 16] }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUColor { pub red: u8, pub green: u8, pub blue: u8, pub alpha: u8 }

/// SU_ERROR codes
pub const SU_ERROR_NONE: i32 = 0;

// ─── SDK 動態載入包裝 ──────────────────────────────────────────────────

/// 已載入的 SketchUp SDK
pub struct SkpSdk {
    _lib: Library,
    // Model
    pub(crate) fn_initialize: unsafe extern "C" fn(),
    pub(crate) fn_terminate: unsafe extern "C" fn(),
    pub(crate) fn_model_create_from_file: unsafe extern "C" fn(*mut SUModelRef, *const c_char) -> i32,
    pub(crate) fn_model_release: unsafe extern "C" fn(*mut SUModelRef) -> i32,
    pub(crate) fn_model_get_entities: unsafe extern "C" fn(SUModelRef, *mut SUEntitiesRef) -> i32,
    // Entities
    pub(crate) fn_entities_get_num_faces: unsafe extern "C" fn(SUEntitiesRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_faces: unsafe extern "C" fn(SUEntitiesRef, usize, *mut SUFaceRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_num_groups: unsafe extern "C" fn(SUEntitiesRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_groups: unsafe extern "C" fn(SUEntitiesRef, usize, *mut SUGroupRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_num_instances: unsafe extern "C" fn(SUEntitiesRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_instances: unsafe extern "C" fn(SUEntitiesRef, usize, *mut SUComponentInstanceRef, *mut usize) -> i32,
    // Face
    pub(crate) fn_face_get_num_vertices: unsafe extern "C" fn(SUFaceRef, *mut usize) -> i32,
    pub(crate) fn_face_get_vertices: unsafe extern "C" fn(SUFaceRef, usize, *mut SUVertexRef, *mut usize) -> i32,
    pub(crate) fn_face_get_normal: unsafe extern "C" fn(SUFaceRef, *mut SUVector3D) -> i32,
    pub(crate) fn_face_get_material: unsafe extern "C" fn(SUFaceRef, *mut SUMaterialRef) -> i32,
    // Vertex
    pub(crate) fn_vertex_get_position: unsafe extern "C" fn(SUVertexRef, *mut SUPoint3D) -> i32,
    // Material
    pub(crate) fn_material_get_name: unsafe extern "C" fn(SUMaterialRef, *mut SUStringRef) -> i32,
    pub(crate) fn_material_get_color: unsafe extern "C" fn(SUMaterialRef, *mut SUColor) -> i32,
    // ComponentDefinition
    pub(crate) fn_comp_def_get_name: unsafe extern "C" fn(SUComponentDefinitionRef, *mut SUStringRef) -> i32,
    pub(crate) fn_comp_def_get_entities: unsafe extern "C" fn(SUComponentDefinitionRef, *mut SUEntitiesRef) -> i32,
    // ComponentInstance
    pub(crate) fn_comp_inst_get_definition: unsafe extern "C" fn(SUComponentInstanceRef, *mut SUComponentDefinitionRef) -> i32,
    pub(crate) fn_comp_inst_get_transform: unsafe extern "C" fn(SUComponentInstanceRef, *mut SUTransformation) -> i32,
    pub(crate) fn_comp_inst_get_name: unsafe extern "C" fn(SUComponentInstanceRef, *mut SUStringRef) -> i32,
    // Group
    pub(crate) fn_group_get_entities: unsafe extern "C" fn(SUGroupRef, *mut SUEntitiesRef) -> i32,
    pub(crate) fn_group_get_transform: unsafe extern "C" fn(SUGroupRef, *mut SUTransformation) -> i32,
    pub(crate) fn_group_get_name: unsafe extern "C" fn(SUGroupRef, *mut SUStringRef) -> i32,
    // String
    pub(crate) fn_string_create: unsafe extern "C" fn(*mut SUStringRef) -> i32,
    pub(crate) fn_string_release: unsafe extern "C" fn(*mut SUStringRef) -> i32,
    pub(crate) fn_string_get_utf8: unsafe extern "C" fn(SUStringRef, usize, *mut c_char, *mut usize) -> i32,
    pub(crate) fn_string_get_utf8_length: unsafe extern "C" fn(SUStringRef, *mut usize) -> i32,
}

/// SDK DLL 搜尋路徑
const SDK_PATHS: &[&str] = &[
    "SketchUpAPI.dll",
    "sketchup_sdk/SketchUpAPI.dll",
    "./lib/SketchUpAPI.dll",
    "C:/Program Files/SketchUp/SketchUp 2024/SketchUpAPI.dll",
    "C:/Program Files/SketchUp/SketchUp 2023/SketchUpAPI.dll",
];

/// 嘗試載入 SDK
pub fn try_load_sdk() -> Result<SkpSdk, SkpError> {
    let lib = SDK_PATHS.iter()
        .find_map(|p| unsafe { Library::new(p).ok() })
        .ok_or_else(|| SkpError::SdkNotFound(
            format!("Searched: {:?}", SDK_PATHS)
        ))?;

    unsafe {
        macro_rules! load {
            ($name:ident, $sym:expr) => {
                let $name: Symbol<_> = lib.get($sym)
                    .map_err(|e| SkpError::SdkNotFound(format!("Symbol {}: {}", stringify!($name), e)))?;
                let $name = *$name;
            };
        }

        load!(fn_initialize, b"SUInitialize");
        load!(fn_terminate, b"SUTerminate");
        load!(fn_model_create_from_file, b"SUModelCreateFromFile");
        load!(fn_model_release, b"SUModelRelease");
        load!(fn_model_get_entities, b"SUModelGetEntities");
        load!(fn_entities_get_num_faces, b"SUEntitiesGetNumFaces");
        load!(fn_entities_get_faces, b"SUEntitiesGetFaces");
        load!(fn_entities_get_num_groups, b"SUEntitiesGetNumGroups");
        load!(fn_entities_get_groups, b"SUEntitiesGetGroups");
        load!(fn_entities_get_num_instances, b"SUEntitiesGetNumInstances");
        load!(fn_entities_get_instances, b"SUEntitiesGetInstances");
        load!(fn_face_get_num_vertices, b"SUFaceGetNumVertices");
        load!(fn_face_get_vertices, b"SUFaceGetVertices");
        load!(fn_face_get_normal, b"SUFaceGetNormal");
        load!(fn_face_get_material, b"SUFaceGetMaterial");
        load!(fn_vertex_get_position, b"SUVertexGetPosition");
        load!(fn_material_get_name, b"SUMaterialGetName");
        load!(fn_material_get_color, b"SUMaterialGetColor");
        load!(fn_comp_def_get_name, b"SUComponentDefinitionGetName");
        load!(fn_comp_def_get_entities, b"SUComponentDefinitionGetEntities");
        load!(fn_comp_inst_get_definition, b"SUComponentInstanceGetDefinition");
        load!(fn_comp_inst_get_transform, b"SUComponentInstanceGetTransform");
        load!(fn_comp_inst_get_name, b"SUComponentInstanceGetName");
        load!(fn_group_get_entities, b"SUGroupGetEntities");
        load!(fn_group_get_transform, b"SUGroupGetTransform");
        load!(fn_group_get_name, b"SUGroupGetName");
        load!(fn_string_create, b"SUStringCreate");
        load!(fn_string_release, b"SUStringRelease");
        load!(fn_string_get_utf8, b"SUStringGetUTF8");
        load!(fn_string_get_utf8_length, b"SUStringGetUTF8Length");

        let sdk = SkpSdk {
            _lib: lib,
            fn_initialize, fn_terminate,
            fn_model_create_from_file, fn_model_release, fn_model_get_entities,
            fn_entities_get_num_faces, fn_entities_get_faces,
            fn_entities_get_num_groups, fn_entities_get_groups,
            fn_entities_get_num_instances, fn_entities_get_instances,
            fn_face_get_num_vertices, fn_face_get_vertices,
            fn_face_get_normal, fn_face_get_material,
            fn_vertex_get_position,
            fn_material_get_name, fn_material_get_color,
            fn_comp_def_get_name, fn_comp_def_get_entities,
            fn_comp_inst_get_definition, fn_comp_inst_get_transform, fn_comp_inst_get_name,
            fn_group_get_entities, fn_group_get_transform, fn_group_get_name,
            fn_string_create, fn_string_release, fn_string_get_utf8, fn_string_get_utf8_length,
        };

        // 初始化 SDK
        (sdk.fn_initialize)();

        Ok(sdk)
    }
}

impl SkpSdk {
    /// 開啟 .skp 檔案
    pub fn open_model(&self, path: &str) -> Result<SkpModel, SkpError> {
        let c_path = CString::new(path).map_err(|e| SkpError::OpenFailed(e.to_string()))?;
        let mut model = SUModelRef { ptr: std::ptr::null_mut() };
        let result = unsafe { (self.fn_model_create_from_file)(&mut model, c_path.as_ptr()) };
        if result != SU_ERROR_NONE {
            return Err(SkpError::OpenFailed(format!("SUModelCreateFromFile returned {}", result)));
        }
        Ok(SkpModel { model, sdk: self })
    }

    /// 從 SUStringRef 取得 Rust String
    pub(crate) fn string_to_rust(&self, su_str: SUStringRef) -> String {
        unsafe {
            let mut len = 0usize;
            (self.fn_string_get_utf8_length)(su_str, &mut len);
            if len == 0 { return String::new(); }
            let mut buf = vec![0u8; len + 1];
            let mut actual = 0usize;
            (self.fn_string_get_utf8)(su_str, len + 1, buf.as_mut_ptr() as *mut c_char, &mut actual);
            String::from_utf8_lossy(&buf[..actual.saturating_sub(1)]).to_string()
        }
    }

    /// 建立 + 讀取 + 釋放 SUStringRef
    pub(crate) fn read_name<F>(&self, getter: F) -> String
    where F: FnOnce(*mut SUStringRef) -> i32
    {
        unsafe {
            let mut s = SUStringRef { ptr: std::ptr::null_mut() };
            (self.fn_string_create)(&mut s);
            getter(&mut s);
            let result = self.string_to_rust(s);
            (self.fn_string_release)(&mut s);
            result
        }
    }
}

/// 已開啟的 SKP 模型
pub struct SkpModel<'a> {
    pub(crate) model: SUModelRef,
    pub(crate) sdk: &'a SkpSdk,
}

impl<'a> Drop for SkpModel<'a> {
    fn drop(&mut self) {
        unsafe { (self.sdk.fn_model_release)(&mut self.model); }
    }
}

impl Drop for SkpSdk {
    fn drop(&mut self) {
        unsafe { (self.fn_terminate)(); }
    }
}
