//! 分析 SketchUp 原始面數量 vs 三角化後
//! 透過 import_skp 間接分析

fn main() {
    let path = std::env::args().nth(1).unwrap_or("docs/sample/SKP_IMPORT.skp".into());
    
    // 用 kolibri_skp 內部的 debug 函數
    kolibri_skp::debug_raw_faces(&path);
}
