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
#[repr(C)] #[derive(Copy, Clone)] pub struct SUDrawingElementRef { pub ptr: *mut std::ffi::c_void }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUPoint3D { pub x: f64, pub y: f64, pub z: f64 }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUVector3D { pub x: f64, pub y: f64, pub z: f64 }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUTransformation { pub values: [f64; 16] }

#[repr(C)] #[derive(Copy, Clone, Debug)]
pub struct SUColor { pub red: u8, pub green: u8, pub blue: u8, pub alpha: u8 }

/// SU_ERROR codes（完整列表，參考 SketchUp C API 文件）
pub const SU_ERROR_NONE: i32 = 0;
pub const SU_ERROR_NULL_POINTER_INPUT: i32 = 1;
pub const SU_ERROR_INVALID_INPUT: i32 = 2;
pub const SU_ERROR_NULL_POINTER_OUTPUT: i32 = 3;
pub const SU_ERROR_INVALID_OUTPUT: i32 = 4;
pub const SU_ERROR_OVERWRITE_VALID: i32 = 5;
pub const SU_ERROR_GENERIC: i32 = 6;
pub const SU_ERROR_SERIALIZATION: i32 = 7;
pub const SU_ERROR_OUT_OF_RANGE: i32 = 8;
pub const SU_ERROR_NO_DATA: i32 = 9;
pub const SU_ERROR_INSUFFICIENT_SIZE: i32 = 10;
pub const SU_ERROR_UNKNOWN_EXCEPTION: i32 = 11;
pub const SU_ERROR_MODEL_INVALID: i32 = 12;
pub const SU_ERROR_MODEL_VERSION: i32 = 13;
pub const SU_ERROR_LAYER_LOCKED: i32 = 14;
pub const SU_ERROR_DUPLICATE: i32 = 15;
pub const SU_ERROR_PARTIAL_SUCCESS: i32 = 16;
pub const SU_ERROR_UNSUPPORTED: i32 = 17;
pub const SU_ERROR_INVALID_ARGUMENT: i32 = 18;

/// 將 SU 錯誤碼轉為可讀訊息
pub fn su_error_name(code: i32) -> &'static str {
    match code {
        0 => "SU_ERROR_NONE",
        1 => "NULL_POINTER_INPUT",
        2 => "INVALID_INPUT",
        3 => "NULL_POINTER_OUTPUT",
        4 => "INVALID_OUTPUT",
        5 => "OVERWRITE_VALID",
        6 => "GENERIC",
        7 => "SERIALIZATION",
        8 => "OUT_OF_RANGE",
        9 => "NO_DATA",
        10 => "INSUFFICIENT_SIZE",
        11 => "UNKNOWN_EXCEPTION",
        12 => "MODEL_INVALID",
        13 => "MODEL_VERSION (SKP 版本比 SDK 新，請用 SketchUp 另存為舊版格式)",
        14 => "LAYER_LOCKED",
        15 => "DUPLICATE",
        16 => "PARTIAL_SUCCESS",
        17 => "UNSUPPORTED",
        18 => "INVALID_ARGUMENT",
        _ => "UNKNOWN",
    }
}

// ─── SDK 動態載入包裝 ──────────────────────────────────────────────────

/// 已載入的 SketchUp SDK
pub struct SkpSdk {
    _lib: Library,
    // Model
    pub(crate) fn_initialize: unsafe extern "C" fn(),
    pub(crate) fn_terminate: unsafe extern "C" fn(),
    pub(crate) fn_get_api_version: Option<unsafe extern "C" fn(*mut usize, *mut usize)>,
    pub(crate) fn_model_create_from_file: unsafe extern "C" fn(*mut SUModelRef, *const c_char) -> i32,
    pub(crate) fn_model_create_from_file_with_status: Option<unsafe extern "C" fn(*mut SUModelRef, *const c_char, *mut i32) -> i32>,
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
    pub(crate) fn_face_get_front_material: unsafe extern "C" fn(SUFaceRef, *mut SUMaterialRef) -> i32,
    pub(crate) fn_face_get_num_edges: unsafe extern "C" fn(SUFaceRef, *mut usize) -> i32,
    pub(crate) fn_face_get_edges: unsafe extern "C" fn(SUFaceRef, usize, *mut SUEdgeRef, *mut usize) -> i32,
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
    // Edge（原始邊線，無三角化產物）
    pub(crate) fn_entities_get_num_edges: unsafe extern "C" fn(SUEntitiesRef, *mut usize) -> i32,
    pub(crate) fn_entities_get_edges: unsafe extern "C" fn(SUEntitiesRef, usize, *mut SUEdgeRef, *mut usize) -> i32,
    pub(crate) fn_edge_get_start_vertex: unsafe extern "C" fn(SUEdgeRef, *mut SUVertexRef) -> i32,
    pub(crate) fn_edge_get_end_vertex: unsafe extern "C" fn(SUEdgeRef, *mut SUVertexRef) -> i32,
    pub(crate) fn_edge_get_soft: unsafe extern "C" fn(SUEdgeRef, *mut bool) -> i32,
    pub(crate) fn_edge_get_smooth: unsafe extern "C" fn(SUEdgeRef, *mut bool) -> i32,
    pub(crate) fn_edge_to_drawing_element: unsafe extern "C" fn(SUEdgeRef) -> SUDrawingElementRef,
    pub(crate) fn_drawing_element_get_hidden: unsafe extern "C" fn(SUDrawingElementRef, *mut bool) -> i32,
    pub(crate) fn_drawing_element_get_material: unsafe extern "C" fn(SUDrawingElementRef, *mut SUMaterialRef) -> i32,
    pub(crate) fn_comp_inst_to_drawing_element: unsafe extern "C" fn(SUComponentInstanceRef) -> SUDrawingElementRef,
    pub(crate) fn_group_to_drawing_element: unsafe extern "C" fn(SUGroupRef) -> SUDrawingElementRef,
    // MeshHelper（正確三角化，支援凹多邊形）
    pub(crate) fn_mesh_helper_create: unsafe extern "C" fn(*mut SUMeshHelperRef, SUFaceRef) -> i32,
    pub(crate) fn_mesh_helper_release: unsafe extern "C" fn(*mut SUMeshHelperRef) -> i32,
    pub(crate) fn_mesh_helper_get_num_triangles: unsafe extern "C" fn(SUMeshHelperRef, *mut usize) -> i32,
    pub(crate) fn_mesh_helper_get_num_vertices: unsafe extern "C" fn(SUMeshHelperRef, *mut usize) -> i32,
    pub(crate) fn_mesh_helper_get_vertex_indices: unsafe extern "C" fn(SUMeshHelperRef, usize, *mut usize, *mut usize) -> i32,
    pub(crate) fn_mesh_helper_get_vertices: unsafe extern "C" fn(SUMeshHelperRef, usize, *mut SUPoint3D, *mut usize) -> i32,
    pub(crate) fn_mesh_helper_get_normals: unsafe extern "C" fn(SUMeshHelperRef, usize, *mut SUVector3D, *mut usize) -> i32,
    // String
    pub(crate) fn_string_create: unsafe extern "C" fn(*mut SUStringRef) -> i32,
    pub(crate) fn_string_release: unsafe extern "C" fn(*mut SUStringRef) -> i32,
    pub(crate) fn_string_get_utf8: unsafe extern "C" fn(SUStringRef, usize, *mut c_char, *mut usize) -> i32,
    pub(crate) fn_string_get_utf8_length: unsafe extern "C" fn(SUStringRef, *mut usize) -> i32,
}

/// SDK DLL 搜尋路徑（優先獨立 SDK，次選安裝目錄）
const SDK_PATHS: &[&str] = &[
    // 最優先：專案內獨立 SDK（無外部依賴，最穩定）
    "docs/SU_SDK/binaries/sketchup/x64/SketchUpAPI.dll",
    "sketchup_sdk/SketchUpAPI.dll",
    "./lib/SketchUpAPI.dll",
    // 次選：SketchUp 安裝路徑（可能有 Qt 依賴問題）
    "C:/Program Files/SketchUp/SketchUp 2025/SketchUp/SketchUpAPI.dll",
    "C:/Program Files/SketchUp/SketchUp 2024/SketchUpAPI.dll",
    "C:/Program Files/SketchUp/SketchUp 2023/SketchUpAPI.dll",
];

/// 嘗試載入 SDK
pub fn try_load_sdk() -> Result<SkpSdk, SkpError> {
    let mut search: Vec<std::path::PathBuf> = Vec::new();
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));

    // 收集所有候選基底目錄（exe 目錄、CWD、exe 的祖先目錄）
    let mut base_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(ref dir) = exe_dir {
        base_dirs.push(dir.clone());
        // 往上走祖先目錄（exe 在 target/debug/ 或 target/release/ 下，往上找專案根目錄）
        let mut ancestor = dir.clone();
        for _ in 0..4 {
            if let Some(parent) = ancestor.parent() {
                ancestor = parent.to_path_buf();
                base_dirs.push(ancestor.clone());
            } else {
                break;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if !base_dirs.contains(&cwd) {
            base_dirs.push(cwd);
        }
    }

    // 將所有搜尋路徑解析為絕對路徑
    for s in SDK_PATHS {
        let p = std::path::PathBuf::from(s);
        if p.is_absolute() {
            search.push(p);
        } else {
            for base in &base_dirs {
                let candidate = base.join(s);
                if candidate.exists() {
                    search.push(candidate);
                }
            }
        }
    }
    // 也嘗試各基底目錄下的裸 SketchUpAPI.dll
    for base in &base_dirs {
        let candidate = base.join("SketchUpAPI.dll");
        if candidate.exists() {
            search.push(candidate);
        }
    }

    if search.is_empty() {
        return Err(SkpError::SdkNotFound(
            format!("No SDK DLL found. Searched base dirs: {:?}", base_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>())
        ));
    }

    let mut last_err = String::new();
    let lib = search.iter()
        .find_map(|p| {
            // 設定 DLL 搜尋目錄（絕對路徑），讓依賴 DLL 能被找到
            if let Some(dir) = p.parent() {
                let abs_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
                if abs_dir.exists() {
                    #[cfg(target_os = "windows")]
                    unsafe {
                        use std::os::windows::ffi::OsStrExt;
                        let wide: Vec<u16> = std::ffi::OsStr::new(&*abs_dir.to_string_lossy())
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        #[link(name = "kernel32")]
                        extern "system" {
                            fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
                        }
                        SetDllDirectoryW(wide.as_ptr());
                    }
                }
            }
            match unsafe { Library::new(p) } {
                Ok(lib) => { eprintln!("[skp-sdk] loaded: {}", p.display()); Some(lib) }
                Err(e) => { last_err = format!("{}: {}", p.display(), e); None }
            }
        })
        .ok_or_else(|| SkpError::SdkNotFound(
            format!("Last error: {}. Searched {} paths", last_err, search.len())
        ))?;

    unsafe {
        macro_rules! load {
            ($name:ident, $sym:expr) => {
                let $name: Symbol<_> = lib.get($sym)
                    .map_err(|e| { eprintln!("[skp-sdk] Symbol load FAIL: {} — {}", stringify!($name), e); SkpError::SdkNotFound(format!("Symbol {}: {}", stringify!($name), e)) })?;
                let $name = *$name;
            };
        }

        load!(fn_initialize, b"SUInitialize");
        load!(fn_terminate, b"SUTerminate");
        // SUGetAPIVersion（optional，用於診斷）
        let fn_get_api_version: Option<unsafe extern "C" fn(*mut usize, *mut usize)> =
            lib.get::<unsafe extern "C" fn(*mut usize, *mut usize)>(b"SUGetAPIVersion")
                .ok().map(|s| *s);
        load!(fn_model_create_from_file, b"SUModelCreateFromFile");
        // SUModelCreateFromFileWithStatus（API 9.0+，optional）
        let fn_model_create_from_file_with_status: Option<unsafe extern "C" fn(*mut SUModelRef, *const c_char, *mut i32) -> i32> =
            lib.get::<unsafe extern "C" fn(*mut SUModelRef, *const c_char, *mut i32) -> i32>(b"SUModelCreateFromFileWithStatus")
                .ok().map(|s| *s);
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
        load!(fn_face_get_num_edges, b"SUFaceGetNumEdges");
        load!(fn_face_get_edges, b"SUFaceGetEdges");
        // 正確 API 名稱是 SUFaceGetFrontMaterial（不是 SUFaceGetMaterial）
        let fn_face_get_front_material = match lib.get::<unsafe extern "C" fn(SUFaceRef, *mut SUMaterialRef) -> i32>(b"SUFaceGetFrontMaterial") {
            Ok(sym) => *sym,
            Err(_) => {
                eprintln!("[skp-sdk] WARNING: SUFaceGetFrontMaterial not found, material extraction disabled");
                unsafe extern "C" fn dummy(_: SUFaceRef, _: *mut SUMaterialRef) -> i32 { -1 }
                dummy as unsafe extern "C" fn(SUFaceRef, *mut SUMaterialRef) -> i32
            }
        };
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
        load!(fn_entities_get_num_edges, b"SUEntitiesGetNumEdges");
        load!(fn_entities_get_edges, b"SUEntitiesGetEdges");
        load!(fn_edge_get_start_vertex, b"SUEdgeGetStartVertex");
        load!(fn_edge_get_end_vertex, b"SUEdgeGetEndVertex");
        load!(fn_edge_get_soft, b"SUEdgeGetSoft");
        load!(fn_edge_get_smooth, b"SUEdgeGetSmooth");
        load!(fn_edge_to_drawing_element, b"SUEdgeToDrawingElement");
        load!(fn_drawing_element_get_hidden, b"SUDrawingElementGetHidden");
        load!(fn_drawing_element_get_material, b"SUDrawingElementGetMaterial");
        load!(fn_comp_inst_to_drawing_element, b"SUComponentInstanceToDrawingElement");
        load!(fn_group_to_drawing_element, b"SUGroupToDrawingElement");
        load!(fn_mesh_helper_create, b"SUMeshHelperCreate");
        load!(fn_mesh_helper_release, b"SUMeshHelperRelease");
        load!(fn_mesh_helper_get_num_triangles, b"SUMeshHelperGetNumTriangles");
        load!(fn_mesh_helper_get_num_vertices, b"SUMeshHelperGetNumVertices");
        load!(fn_mesh_helper_get_vertex_indices, b"SUMeshHelperGetVertexIndices");
        load!(fn_mesh_helper_get_vertices, b"SUMeshHelperGetVertices");
        load!(fn_mesh_helper_get_normals, b"SUMeshHelperGetNormals");
        load!(fn_string_create, b"SUStringCreate");
        load!(fn_string_release, b"SUStringRelease");
        load!(fn_string_get_utf8, b"SUStringGetUTF8");
        load!(fn_string_get_utf8_length, b"SUStringGetUTF8Length");

        let sdk = SkpSdk {
            _lib: lib,
            fn_initialize, fn_terminate, fn_get_api_version,
            fn_model_create_from_file, fn_model_create_from_file_with_status,
            fn_model_release, fn_model_get_entities,
            fn_entities_get_num_faces, fn_entities_get_faces,
            fn_entities_get_num_groups, fn_entities_get_groups,
            fn_entities_get_num_instances, fn_entities_get_instances,
            fn_face_get_num_vertices, fn_face_get_vertices,
            fn_face_get_normal, fn_face_get_front_material,
            fn_face_get_num_edges, fn_face_get_edges,
            fn_vertex_get_position,
            fn_material_get_name, fn_material_get_color,
            fn_comp_def_get_name, fn_comp_def_get_entities,
            fn_comp_inst_get_definition, fn_comp_inst_get_transform, fn_comp_inst_get_name,
            fn_group_get_entities, fn_group_get_transform, fn_group_get_name,
            fn_entities_get_num_edges, fn_entities_get_edges,
            fn_edge_get_start_vertex, fn_edge_get_end_vertex,
            fn_edge_get_soft, fn_edge_get_smooth,
            fn_edge_to_drawing_element, fn_drawing_element_get_hidden,
            fn_drawing_element_get_material, fn_comp_inst_to_drawing_element, fn_group_to_drawing_element,
            fn_mesh_helper_create, fn_mesh_helper_release,
            fn_mesh_helper_get_num_triangles, fn_mesh_helper_get_num_vertices,
            fn_mesh_helper_get_vertex_indices, fn_mesh_helper_get_vertices, fn_mesh_helper_get_normals,
            fn_string_create, fn_string_release, fn_string_get_utf8, fn_string_get_utf8_length,
        };

        // 初始化 SDK
        (sdk.fn_initialize)();

        Ok(sdk)
    }
}

impl SkpSdk {
    /// 取得 SDK API 版本
    pub fn api_version(&self) -> (usize, usize) {
        if let Some(f) = self.fn_get_api_version {
            let (mut major, mut minor) = (0usize, 0usize);
            unsafe { f(&mut major, &mut minor); }
            (major, minor)
        } else {
            (0, 0) // 無法取得
        }
    }

    /// 開啟 .skp 檔案（優先使用帶 status 的新版 API）
    pub fn open_model(&self, path: &str) -> Result<SkpModel, SkpError> {
        let c_path = CString::new(path).map_err(|e| SkpError::OpenFailed(e.to_string()))?;
        let mut model = SUModelRef { ptr: std::ptr::null_mut() };

        // 優先使用 SUModelCreateFromFileWithStatus（API 9.0+）
        if let Some(f) = self.fn_model_create_from_file_with_status {
            let mut status: i32 = 0;
            let result = unsafe { f(&mut model, c_path.as_ptr(), &mut status) };
            if result != SU_ERROR_NONE {
                let msg = if result == SU_ERROR_MODEL_VERSION {
                    format!("SKP 檔案版本比 SDK 新（error={}）。請用 SketchUp 另存為舊版格式。", su_error_name(result))
                } else if result == SU_ERROR_SERIALIZATION {
                    format!("SKP 檔案損毀或無法讀取（error={}）", su_error_name(result))
                } else {
                    format!("SUModelCreateFromFileWithStatus failed: {} (code={})", su_error_name(result), result)
                };
                return Err(SkpError::OpenFailed(msg));
            }
            // status = 1 表示 SKP 版本比 SDK 新（可能遺漏資料）
            if status == 1 {
                eprintln!("[skp-sdk] WARNING: SKP file was created in a newer SketchUp version — some data may be missing");
            }
        } else {
            // Fallback: 舊版 API
            let result = unsafe { (self.fn_model_create_from_file)(&mut model, c_path.as_ptr()) };
            if result != SU_ERROR_NONE {
                let msg = format!("SUModelCreateFromFile failed: {} (code={})", su_error_name(result), result);
                return Err(SkpError::OpenFailed(msg));
            }
        }
        Ok(SkpModel { model, sdk: self })
    }

    /// 從 SUStringRef 取得 Rust String（正確處理 UTF-8 中文）
    pub(crate) fn string_to_rust(&self, su_str: SUStringRef) -> String {
        unsafe {
            let mut len = 0usize;
            (self.fn_string_get_utf8_length)(su_str, &mut len);
            if len == 0 { return String::new(); }
            // 給足夠的 buffer（len 是不含 null 的 byte 數）
            let buf_size = len + 4; // 多給幾個 byte 以防萬一
            let mut buf = vec![0u8; buf_size];
            let mut actual = 0usize;
            (self.fn_string_get_utf8)(su_str, buf_size, buf.as_mut_ptr() as *mut c_char, &mut actual);
            // actual 包含 null terminator，找到第一個 0 byte 截斷
            let end = buf.iter().position(|&b| b == 0).unwrap_or(actual);
            String::from_utf8_lossy(&buf[..end]).to_string()
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
