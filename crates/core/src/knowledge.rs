//! RAG 知識庫基礎模組
//! 提供結構化的建築/鋼構知識索引，供 AI 語意推斷引擎使用
//! Phase 1: 靜態知識 + 簡單搜尋，後續可接 embedding 向量庫

use serde::{Deserialize, Serialize};

/// 知識條目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub category: KnowledgeCategory,
    pub title: String,
    /// 內容文字（用於搜尋比對）
    pub content: String,
    /// 關鍵字標籤
    pub tags: Vec<String>,
    /// 參考來源（法規/手冊/標準）
    pub source: Option<String>,
}

/// 知識類別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeCategory {
    /// 鋼構規範（AISC 360/341/358、CNS）
    SteelCode,
    /// 接頭設計
    ConnectionDesign,
    /// 耐震設計
    SeismicDesign,
    /// 焊接規範
    WeldingCode,
    /// 螺栓規範
    BoltCode,
    /// 建築法規
    BuildingCode,
    /// 管線規範
    PipingCode,
    /// 材料性質
    MaterialProperty,
    /// BIM/IFC 標準
    BimStandard,
    /// 施工實務
    ConstructionPractice,
}

impl KnowledgeCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SteelCode => "鋼構規範",
            Self::ConnectionDesign => "接頭設計",
            Self::SeismicDesign => "耐震設計",
            Self::WeldingCode => "焊接規範",
            Self::BoltCode => "螺栓規範",
            Self::BuildingCode => "建築法規",
            Self::PipingCode => "管線規範",
            Self::MaterialProperty => "材料性質",
            Self::BimStandard => "BIM 標準",
            Self::ConstructionPractice => "施工實務",
        }
    }
}

/// 知識庫
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub entries: Vec<KnowledgeEntry>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        let mut kb = Self::default();
        kb.load_builtin();
        kb
    }

    /// 載入內建知識條目
    fn load_builtin(&mut self) {
        // ── AISC 360-22 鋼構設計 ──
        self.add("aisc360_j1", KnowledgeCategory::SteelCode,
            "AISC 360-22 J1 一般接頭規定",
            "構件連接應依 AISC 360-22 Chapter J 設計。接頭應傳遞所需之力量與力矩，\
             並應符合極限強度設計法(LRFD)或容許應力設計法(ASD)之要求。",
            &["AISC", "360", "接頭", "J1"], Some("AISC 360-22"));

        self.add("aisc360_j2", KnowledgeCategory::WeldingCode,
            "AISC 360-22 J2 焊接",
            "焊接設計應符合 AWS D1.1。角焊最小尺寸依 Table J2.4，最大尺寸不超過板厚-2mm（板厚>6mm時）。\
             角焊有效長度≥4倍焊腳尺寸。全滲透焊(CJP)有效喉厚等於較薄件厚度。",
            &["焊接", "角焊", "CJP", "AWS", "J2"], Some("AISC 360-22"));

        self.add("aisc360_j3", KnowledgeCategory::BoltCode,
            "AISC 360-22 J3 螺栓",
            "高強度螺栓（F10T/A325/A490）之設計拉力與剪力���度依 Table J3.2。\
             螺栓孔距≥3d（d=螺栓直徑），邊距≥1.5d。滑動臨界接合(slip-critical)用於振動或反覆載重。",
            &["螺栓", "F10T", "A325", "J3", "滑動臨界"], Some("AISC 360-22"));

        // ── AISC 341-22 耐震設計 ──
        self.add("aisc341_e3", KnowledgeCategory::SeismicDesign,
            "AISC 341-22 E3 特殊抗彎矩框架 SMF",
            "SMF 梁柱接頭需達 0.04 rad 層間位移角。柱梁彎矩比 ΣMpc*/ΣMpb* > 1.0。\
             梁翼板至柱接合需全���透焊(CJP)。柱翼板需 panel zone 檢核。",
            &["SMF", "耐震", "341", "層間位移", "E3"], Some("AISC 341-22"));

        self.add("aisc341_rbs", KnowledgeCategory::SeismicDesign,
            "AISC 358 RBS 狗骨頭接頭",
            "RBS(Reduced Beam Section)在梁翼板切削以形成塑性鉸。\
             切削參數: a=0.5~0.75bf, b=0.65~0.85d, c≤0.25bf。\
             適用於 SMF 和 IMF。梁深≤W920，翼板厚≤44mm。",
            &["RBS", "狗骨頭", "358", "SMF", "塑性鉸"], Some("AISC 358-22"));

        // ── CNS 鋼材 ──
        self.add("cns_ss400", KnowledgeCategory::MaterialProperty,
            "SS400 一般結構用鋼",
            "SS400: Fy=245 MPa (t≤16), Fu=400~510 MPa。CNS 2947 / JIS G3101。\
             適用一般結構，不建議用於耐震抗彎矩框架之梁翼板。",
            &["SS400", "CNS", "材��", "降伏"], Some("CNS 2947"));

        self.add("cns_sn490b", KnowledgeCategory::MaterialProperty,
            "SN490B 建築結構用鋼",
            "SN490B: Fy=325 MPa, Fu=490~610 MPa。CNS 13812。\
             具良好焊接性與降伏比控制(YR≤0.80)，適用耐震結構。\
             Ry=1.1（AISC 341 超強係數）。",
            &["SN490B", "CNS", "耐震", "YR", "超強"], Some("CNS 13812"));

        // ── 管線 ──
        self.add("pipe_cns_sch", KnowledgeCategory::PipingCode,
            "CNS 管材肉厚等級",
            "CNS 管材依 Schedule 分級: Sch10/20/30/40/80/160/XXS。\
             消防鐵管需 Sch40 以上。不鏽鋼管常用 Sch10S/40S。\
             PVC 管依���力等級: 0.5/1.0/1.5 MPa。",
            &["管線", "CNS", "Schedule", "消防", "PVC"], Some("CNS 6445"));

        // ── IFC 標準 ──
        self.add("ifc4_overview", KnowledgeCategory::BimStandard,
            "IFC4 主要變更",
            "IFC4 相較 IFC2x3 新增: IfcPipeSegment、IfcMaterial 改進、\
             IfcRelAssociatesMaterial、Property Template、新幾何表示法。\
             View Definition: Reference View (RV) 和 Design Transfer View (DTV)。",
            &["IFC4", "BIM", "buildingSMART"], Some("ISO 16739-1:2018"));
    }

    fn add(&mut self, id: &str, category: KnowledgeCategory, title: &str,
           content: &str, tags: &[&str], source: Option<&str>) {
        self.entries.push(KnowledgeEntry {
            id: id.into(),
            category,
            title: title.into(),
            content: content.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            source: source.map(|s| s.into()),
        });
    }

    /// 簡單關鍵字搜尋（TF-IDF 的簡化版）
    pub fn search(&self, query: &str, max_results: usize) -> Vec<&KnowledgeEntry> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
        if query_terms.is_empty() { return Vec::new(); }

        let mut scored: Vec<(&KnowledgeEntry, f32)> = self.entries.iter()
            .map(|entry| {
                let mut score = 0.0_f32;
                let title_lower = entry.title.to_lowercase();
                let content_lower = entry.content.to_lowercase();
                for term in &query_terms {
                    // 標題命中加重
                    if title_lower.contains(term) { score += 3.0; }
                    // 內容��中
                    if content_lower.contains(term) { score += 1.0; }
                    // 標籤精確命中加重
                    for tag in &entry.tags {
                        if tag.to_lowercase() == *term { score += 5.0; }
                        else if tag.to_lowercase().contains(term) { score += 2.0; }
                    }
                }
                (entry, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(max_results).map(|(e, _)| e).collect()
    }

    /// 依類別篩選
    pub fn by_category(&self, cat: KnowledgeCategory) -> Vec<&KnowledgeEntry> {
        self.entries.iter().filter(|e| e.category == cat).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_basic() {
        let kb = KnowledgeBase::new();
        let results = kb.search("焊接 角焊", 5);
        assert!(!results.is_empty());
        assert!(results[0].content.contains("角焊"));
    }

    #[test]
    fn test_search_rbs() {
        let kb = KnowledgeBase::new();
        let results = kb.search("RBS 狗骨頭 SMF", 3);
        assert!(!results.is_empty());
        assert!(results[0].title.contains("RBS"));
    }

    #[test]
    fn test_category_filter() {
        let kb = KnowledgeBase::new();
        let seismic = kb.by_category(KnowledgeCategory::SeismicDesign);
        assert!(seismic.len() >= 2);
    }

    #[test]
    fn test_builtin_count() {
        let kb = KnowledgeBase::new();
        assert!(kb.entries.len() >= 8);
    }
}
