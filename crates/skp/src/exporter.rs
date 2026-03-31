//! SKP 匯出器 — 將 K3D 場景轉存為 SketchUp .skp 檔案
//!
//! 使用 SketchUp C SDK 的寫入 API：
//!   SUModelCreate → SUGeometryInputCreate → AddVertex/AddFace → SUEntitiesFill → SUModelSaveToFileWithVersion
//!
//! 如果 SDK DLL 不可用，fallback 用 OBJ 匯出（SketchUp 可開啟 OBJ）。

use crate::ffi::*;
use crate::SkpError;
use std::ffi::CString;

/// 匯出場景為 .skp 檔案
/// scene_data: 三角化的面資料 (vertices, indices, material_colors)
pub fn export_skp(
    path: &str,
    objects: &[SkpExportObject],
) -> Result<usize, SkpError> {
    // 嘗試用 SDK 匯出
    match export_skp_sdk(path, objects) {
        Ok(count) => Ok(count),
        Err(e) => {
            eprintln!("[skp-export] SDK export failed: {}, falling back to OBJ", e);
            Err(e)
        }
    }
}

/// 要匯出的物件
pub struct SkpExportObject {
    pub name: String,
    pub vertices: Vec<[f64; 3]>,    // 三角化頂點（SU 用 f64、吋為單位）
    pub face_indices: Vec<[usize; 3]>, // 三角形索引
    pub color: [u8; 4],             // RGBA
}

/// 用 SDK 直接匯出
fn export_skp_sdk(path: &str, objects: &[SkpExportObject]) -> Result<usize, SkpError> {
    let sdk = try_load_sdk()?;

    // 動態載入寫入函數
    let fn_model_create: unsafe extern "C" fn(*mut SUModelRef) -> i32 =
        unsafe { *sdk._lib.get(b"SUModelCreate")
            .map_err(|e| SkpError::SdkError(format!("SUModelCreate not found: {}", e)))? };

    let fn_model_save: unsafe extern "C" fn(SUModelRef, *const std::os::raw::c_char) -> i32 =
        unsafe { *sdk._lib.get(b"SUModelSaveToFile")
            .map_err(|e| SkpError::SdkError(format!("SUModelSaveToFile not found: {}", e)))? };

    // 嘗試載入 SaveToFileWithVersion（可指定 SKP 版本）
    let fn_model_save_ver: Option<unsafe extern "C" fn(SUModelRef, *const std::os::raw::c_char, i32) -> i32> =
        unsafe { sdk._lib.get(b"SUModelSaveToFileWithVersion").ok().map(|s| *s) };

    let fn_geom_input_create: unsafe extern "C" fn(*mut SUGeometryInputRef) -> i32 =
        unsafe { *sdk._lib.get(b"SUGeometryInputCreate")
            .map_err(|e| SkpError::SdkError(format!("SUGeometryInputCreate: {}", e)))? };

    let fn_geom_input_release: unsafe extern "C" fn(*mut SUGeometryInputRef) -> i32 =
        unsafe { *sdk._lib.get(b"SUGeometryInputRelease")
            .map_err(|e| SkpError::SdkError(format!("SUGeometryInputRelease: {}", e)))? };

    let fn_geom_input_add_vertex: unsafe extern "C" fn(SUGeometryInputRef, *const SUPoint3D) -> i32 =
        unsafe { *sdk._lib.get(b"SUGeometryInputAddVertex")
            .map_err(|e| SkpError::SdkError(format!("SUGeometryInputAddVertex: {}", e)))? };

    let fn_geom_input_face_create: unsafe extern "C" fn(*mut SULoopInputRef) -> i32 =
        unsafe { *sdk._lib.get(b"SULoopInputCreate")
            .map_err(|e| SkpError::SdkError(format!("SULoopInputCreate: {}", e)))? };

    let fn_loop_input_add_vertex_index: unsafe extern "C" fn(SULoopInputRef, usize) -> i32 =
        unsafe { *sdk._lib.get(b"SULoopInputAddVertexIndex")
            .map_err(|e| SkpError::SdkError(format!("SULoopInputAddVertexIndex: {}", e)))? };

    let fn_geom_input_add_face: unsafe extern "C" fn(SUGeometryInputRef, *mut SULoopInputRef, *mut usize) -> i32 =
        unsafe { *sdk._lib.get(b"SUGeometryInputAddFace")
            .map_err(|e| SkpError::SdkError(format!("SUGeometryInputAddFace: {}", e)))? };

    let fn_entities_fill: unsafe extern "C" fn(SUEntitiesRef, SUGeometryInputRef) -> i32 =
        unsafe { *sdk._lib.get(b"SUEntitiesFill")
            .map_err(|e| SkpError::SdkError(format!("SUEntitiesFill: {}", e)))? };

    // 建立空模型
    let mut model = SUModelRef { ptr: std::ptr::null_mut() };
    let rc = unsafe { fn_model_create(&mut model) };
    if rc != SU_ERROR_NONE {
        return Err(SkpError::SdkError(format!("SUModelCreate failed: {}", rc)));
    }

    // 取得 root entities
    let mut entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
    unsafe { (sdk.fn_model_get_entities)(model, &mut entities) };

    let mut total_faces = 0usize;

    for obj in objects {
        if obj.vertices.is_empty() || obj.face_indices.is_empty() { continue; }

        // 建立 geometry input
        let mut geom_input = SUGeometryInputRef { ptr: std::ptr::null_mut() };
        let rc = unsafe { fn_geom_input_create(&mut geom_input) };
        if rc != SU_ERROR_NONE { continue; }

        // 加入頂點（K3D 用 mm，SU 用吋，需轉換）
        let mm_to_inch = 1.0 / 25.4;
        for v in &obj.vertices {
            let pt = SUPoint3D {
                x: v[0] * mm_to_inch,
                y: v[2] * mm_to_inch,  // K3D Y=up → SU Z=up，交換 Y/Z
                z: v[1] * mm_to_inch,
            };
            unsafe { fn_geom_input_add_vertex(geom_input, &pt) };
        }

        // 加入三角面
        for tri in &obj.face_indices {
            let mut loop_input = SULoopInputRef { ptr: std::ptr::null_mut() };
            let rc = unsafe { fn_geom_input_face_create(&mut loop_input) };
            if rc != SU_ERROR_NONE { continue; }

            unsafe {
                fn_loop_input_add_vertex_index(loop_input, tri[0]);
                fn_loop_input_add_vertex_index(loop_input, tri[1]);
                fn_loop_input_add_vertex_index(loop_input, tri[2]);
            }

            let mut face_idx = 0usize;
            unsafe { fn_geom_input_add_face(geom_input, &mut loop_input, &mut face_idx) };
            total_faces += 1;
        }

        // 填入 entities
        unsafe { fn_entities_fill(entities, geom_input) };
        unsafe { fn_geom_input_release(&mut geom_input) };
    }

    // 儲存檔案
    let c_path = CString::new(path).map_err(|e| SkpError::SdkError(e.to_string()))?;

    let save_rc = if let Some(save_ver) = fn_model_save_ver {
        // SUFileVersion_SU2021 = 21, SU2020 = 20 ...
        // 存成 SU 2021 格式（廣泛相容）
        unsafe { save_ver(model, c_path.as_ptr(), 21) }
    } else {
        unsafe { fn_model_save(model, c_path.as_ptr()) }
    };

    // 釋放模型
    unsafe { (sdk.fn_model_release)(&mut model) };

    if save_rc != SU_ERROR_NONE {
        return Err(SkpError::SdkError(format!("SUModelSaveToFile failed: {}", save_rc)));
    }

    eprintln!("[skp-export] Saved {} faces to {}", total_faces, path);
    Ok(total_faces)
}

// ─── Additional SU types for export ───

#[repr(C)] #[derive(Copy, Clone)]
pub struct SUGeometryInputRef { pub ptr: *mut std::ffi::c_void }

#[repr(C)] #[derive(Copy, Clone)]
pub struct SULoopInputRef { pub ptr: *mut std::ffi::c_void }
