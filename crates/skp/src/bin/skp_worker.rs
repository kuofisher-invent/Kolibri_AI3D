//! SKP Worker — 子進程匯入 SKP 檔案
//!
//! 在獨立進程中執行 SDK 呼叫，避免 DLL 崩潰影響主 APP。
//! 用法：kolibri-skp-worker <path.skp>
//! 成功時將 SkpScene JSON 輸出到 stdout，失敗時 exit code != 0。

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: kolibri-skp-worker <path.skp>");
        std::process::exit(1);
    }
    let path = &args[1];

    // 診斷模式：--diag 只做 init + open + release + terminate
    if args.iter().any(|a| a == "--diag") {
        eprintln!("[diag] Loading SDK...");
        let sdk = match kolibri_skp::ffi::try_load_sdk() {
            Ok(s) => { eprintln!("[diag] SDK loaded, API version: {:?}", s.api_version()); s }
            Err(e) => { eprintln!("[diag] SDK load FAILED: {}", e); std::process::exit(10); }
        };
        eprintln!("[diag] Opening model: {}", path);
        match sdk.open_model(path) {
            Ok(_m) => { eprintln!("[diag] Model opened OK"); }
            Err(e) => { eprintln!("[diag] open_model FAILED: {}", e); std::process::exit(11); }
        };
        eprintln!("[diag] All OK — model is readable");
        std::process::exit(0);
    }

    match kolibri_skp::import_skp(path) {
        Ok(scene) => {
            // 輸出 JSON 到 stdout
            match serde_json::to_string(&scene) {
                Ok(json) => {
                    println!("{}", json);
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("JSON serialize error: {}", e);
                    std::process::exit(2);
                }
            }
        }
        Err(e) => {
            // 輸出錯誤訊息到 stderr
            eprintln!("SKP import error: {}", e);
            std::process::exit(3);
        }
    }
}
