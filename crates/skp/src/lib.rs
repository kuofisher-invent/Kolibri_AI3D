//! kolibri-skp — SketchUp SDK FFI (動態載入，不需安裝 SketchUp)
//!
//! 使用 SketchUp C API SDK 直接讀取 .skp 檔案。
//! SDK DLL 在 runtime 動態載入：
//!   - 有 DLL → 完整匯入（幾何 + 元件 + 群組 + 材質）
//!   - 沒有 DLL → 回傳 Err，由上層退回 heuristic/bridge
//!
//! SDK 下載：https://extensions.sketchup.com/developer_center/sketchup_sdk

pub mod ffi;
pub mod converter;

/// 檢查 SDK DLL 是否可用
pub fn sdk_available() -> bool {
    ffi::try_load_sdk().is_ok()
}

/// 讀取 SKP 檔案版本（從檔案 header）
/// 回傳 (major, minor, build) 例如 (25, 0, 571) 代表 SU 2025
pub fn detect_skp_version(path: &str) -> Option<(u32, u32, u32)> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 100 { return None; }
    // SKP header 是 UTF-16LE，找 "{XX.X.XXX}" 格式的版本號
    let text: String = data.iter().take(200)
        .zip(data.iter().skip(1))
        .step_by(2)
        .filter_map(|(&lo, &hi)| {
            if hi == 0 && lo.is_ascii() { Some(lo as char) } else { None }
        })
        .collect();
    let start = text.find('{')?;
    let end = text.find('}')?;
    let ver_str = &text[start + 1..end]; // "25.0.571"
    let parts: Vec<&str> = ver_str.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let build = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        Some((major, minor, build))
    } else {
        None
    }
}

/// SDK 能處理的最高 SKP 主版本（SU 2025 SDK → major 25）
/// 根據安裝的 DLL 路徑推斷
fn sdk_max_version() -> u32 {
    // 檢查哪個 SDK DLL 路徑存在
    let paths = [
        ("C:/Program Files/SketchUp/SketchUp 2025/SketchUp/SketchUpAPI.dll", 25),
        ("C:/Program Files/SketchUp/SketchUp 2024/SketchUpAPI.dll", 24),
        ("C:/Program Files/SketchUp/SketchUp 2023/SketchUpAPI.dll", 23),
    ];
    for (p, ver) in &paths {
        if std::path::Path::new(p).exists() {
            return *ver;
        }
    }
    // 找不到已知路徑，假設可處理到 25
    25
}

/// 從 .skp 檔案匯入場景資料
/// 回傳序列化的場景結構，與 UnifiedIR 相容
/// 加入版本檢查 + catch_unwind 保護
pub fn import_skp(path: &str) -> Result<SkpScene, SkpError> {
    // 版本檢查：避免載入比 SDK 新的 SKP 格式（會 crash）
    if let Some((major, _minor, _build)) = detect_skp_version(path) {
        let sdk_ver = sdk_max_version();
        eprintln!("[skp] File version: SU {} (major={}), SDK max version: SU {} (major={})",
            2000 + major, major, 2000 + sdk_ver - 2, sdk_ver);
        if major > sdk_ver {
            return Err(SkpError::OpenFailed(format!(
                "SKP 檔案版本 (SU {}) 比安裝的 SDK (SU {}) 新。請用 SketchUp 另存為 {} 或更早版本格式。",
                2000 + major, 2000 + sdk_ver - 2, 2000 + sdk_ver - 2,
            )));
        }
    }

    let path_owned = path.to_string();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let sdk = ffi::try_load_sdk()?;
        let model = sdk.open_model(&path_owned)?;
        let result = converter::convert_model(&sdk, &model);
        // 注意：不要手動 drop model/sdk — SUModelRelease 在某些情況下會 crash
        // （快取的 entities ref 跟 release 衝突）。
        // 讓 Rust 自動 Drop 順序處理（model 先 drop，sdk 後 drop）。
        // 如果 worker 子進程，process exit 會清理一切。
        std::mem::forget(model);
        std::mem::forget(sdk);
        result
    })) {
        Ok(result) => result,
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic in SDK DLL".to_string()
            };
            Err(SkpError::SdkError(format!(
                "SDK 崩潰（可能是 DLL 版本與 SKP 檔案不相容）: {}。請用 SketchUp 另存為較舊版本格式。",
                msg
            )))
        }
    }
}

/// 用子進程匯入 SKP 檔案（防止 DLL 崩潰影響主 APP）
/// 子進程 `kolibri-skp-worker` 在獨立進程中執行 SDK 呼叫
pub fn import_skp_subprocess(path: &str) -> Result<SkpScene, SkpError> {
    // 版本預檢
    if let Some((major, _minor, _build)) = detect_skp_version(path) {
        let sdk_ver = sdk_max_version();
        if major > sdk_ver {
            return Err(SkpError::OpenFailed(format!(
                "SKP 檔案版本 (SU {}) 比安裝的 SDK (SU {}) 新。請用 SketchUp 另存為 {} 或更早版本格式。",
                2000 + major, 2000 + sdk_ver, 2000 + sdk_ver,
            )));
        }
    }

    // 找到 worker 執行檔
    let worker_name = if cfg!(windows) { "kolibri-skp-worker.exe" } else { "kolibri-skp-worker" };
    let worker_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(worker_name)))
        .unwrap_or_else(|| std::path::PathBuf::from(worker_name));

    if !worker_path.exists() {
        eprintln!("[skp] Worker not found at {}, falling back to in-process import", worker_path.display());
        return import_skp(path);
    }

    eprintln!("[skp] Launching subprocess: {} {}", worker_path.display(), path);

    let output = std::process::Command::new(&worker_path)
        .arg(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| SkpError::SdkError(format!("無法啟動 SKP worker: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        return Err(SkpError::SdkError(format!(
            "SKP worker 失敗 (exit={:#x}): {}",
            code, stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout)
        .map_err(|e| SkpError::ConvertError(format!("Worker JSON 解析失敗: {}", e)))
}

/// SKP 匯入結果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpScene {
    pub meshes: Vec<SkpMesh>,
    pub instances: Vec<SkpInstance>,
    pub groups: Vec<SkpGroup>,
    pub component_defs: Vec<SkpComponentDef>,
    pub materials: Vec<SkpMaterial>,
    pub units: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpMesh {
    pub id: String,
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub material_id: Option<String>,
    /// SDK 原始邊線（非三角化產物），每條邊是兩個頂點座標
    pub edges: Vec<([f32; 3], [f32; 3])>,
    pub source_vertex_labels: Vec<String>,
    pub source_triangle_debug: Vec<SkpTriangleDebug>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpTriangleDebug {
    pub triangle_index: usize,
    pub indices: [u32; 3],
    pub source_face_label: String,
    pub source_vertex_labels: [String; 3],
    pub generator: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpInstance {
    pub id: String,
    pub mesh_id: String,
    pub component_def_id: Option<String>,
    pub transform: [f32; 16],
    pub name: String,
    pub layer: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpGroup {
    pub id: String,
    pub name: String,
    pub children: Vec<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpComponentDef {
    pub id: String,
    pub name: String,
    pub mesh_ids: Vec<String>,
    pub instance_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkpMaterial {
    pub id: String,
    pub name: String,
    pub color: [f32; 4],
    pub texture_path: Option<String>,
    pub opacity: f32,
}

/// 除錯：列出 SKP 原始面數量和每面的三角化結果
pub fn debug_raw_faces(path: &str) {
    let sdk = match ffi::try_load_sdk() { Ok(s) => s, Err(e) => { println!("SDK: {}", e); return; } };
    let model = match sdk.open_model(path) { Ok(m) => m, Err(e) => { println!("Open: {}", e); return; } };

    let mut entities = ffi::SUEntitiesRef { ptr: std::ptr::null_mut() };
    unsafe { (sdk.fn_model_get_entities)(model.model, &mut entities) };

    let mut root_faces = 0usize;
    unsafe { (sdk.fn_entities_get_num_faces)(entities, &mut root_faces) };
    println!("Root loose faces: {}", root_faces);

    let mut inst_count = 0usize;
    unsafe { (sdk.fn_entities_get_num_instances)(entities, &mut inst_count) };
    if inst_count > 0 {
        let mut insts = vec![ffi::SUComponentInstanceRef { ptr: std::ptr::null_mut() }; inst_count];
        let mut actual = 0usize;
        unsafe { (sdk.fn_entities_get_instances)(entities, inst_count, insts.as_mut_ptr(), &mut actual) };
        for (i, inst) in insts[..actual].iter().enumerate() {
            let mut def = ffi::SUComponentDefinitionRef { ptr: std::ptr::null_mut() };
            unsafe { (sdk.fn_comp_inst_get_definition)(*inst, &mut def) };
            let name = sdk.read_name(|s| unsafe { (sdk.fn_comp_def_get_name)(def, s) });
            let mut def_ent = ffi::SUEntitiesRef { ptr: std::ptr::null_mut() };
            unsafe { (sdk.fn_comp_def_get_entities)(def, &mut def_ent) };
            let mut nf = 0usize;
            unsafe { (sdk.fn_entities_get_num_faces)(def_ent, &mut nf) };
            let mut faces = vec![ffi::SUFaceRef { ptr: std::ptr::null_mut() }; nf];
            let mut af = 0usize;
            unsafe { (sdk.fn_entities_get_faces)(def_ent, nf, faces.as_mut_ptr(), &mut af) };
            println!("\nComponent[{}] '{}' — {} SU faces:", i, name, af);
            let mut total_tris = 0usize;
            for (fi, face) in faces[..af].iter().enumerate() {
                let mut nv = 0usize;
                unsafe { (sdk.fn_face_get_num_vertices)(*face, &mut nv) };
                let mut helper = ffi::SUMeshHelperRef { ptr: std::ptr::null_mut() };
                let rc = unsafe { (sdk.fn_mesh_helper_create)(&mut helper, *face) };
                let (mut ntri, mut nverts) = (0usize, 0usize);
                if rc == 0 {
                    unsafe {
                        (sdk.fn_mesh_helper_get_num_triangles)(helper, &mut ntri);
                        (sdk.fn_mesh_helper_get_num_vertices)(helper, &mut nverts);
                        (sdk.fn_mesh_helper_release)(&mut helper);
                    };
                }
                let mut normal = ffi::SUVector3D { x: 0.0, y: 0.0, z: 0.0 };
                unsafe { (sdk.fn_face_get_normal)(*face, &mut normal) };
                // 座標軸交換顯示 (SU: X,Y,Z → Kolibri: X,Z,Y)
                println!("  F{:2}: {} verts → {} tris | SU normal=({:.2},{:.2},{:.2}) → Kolibri=({:.2},{:.2},{:.2})",
                    fi, nv, ntri, normal.x, normal.y, normal.z, normal.x, normal.z, normal.y);
                total_tris += ntri;
            }
            println!("  Total: {} SU faces → {} triangles", af, total_tris);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkpError {
    #[error("SketchUp SDK DLL not found: {0}")]
    SdkNotFound(String),
    #[error("Failed to open model: {0}")]
    OpenFailed(String),
    #[error("SDK error: {0}")]
    SdkError(String),
    #[error("Conversion error: {0}")]
    ConvertError(String),
}
