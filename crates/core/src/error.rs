//! 統一錯誤型別（thiserror），取代散落的 Result<_, String>

/// 檔案 IO 錯誤
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("開啟檔案失敗: {path} — {source}")]
    Open { path: String, source: std::io::Error },

    #[error("讀取失敗: {path} — {source}")]
    Read { path: String, source: std::io::Error },

    #[error("寫入失敗: {path} — {source}")]
    Write { path: String, source: std::io::Error },

    #[error("序列化失敗: {0}")]
    Serialize(String),

    #[error("反序列化失敗: {0}")]
    Deserialize(String),

    #[error("格式錯誤: {0}")]
    Format(String),

    #[error("不支援的格式: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<serde_json::Error> for FileError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e.to_string())
    }
}

/// 方便從 String 轉換（向下相容）
impl From<String> for FileError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

/// 方便轉回 String（向下相容期間）
impl From<FileError> for String {
    fn from(e: FileError) -> Self {
        e.to_string()
    }
}
