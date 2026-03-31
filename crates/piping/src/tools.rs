//! 管線繪製工具狀態與操作

use serde::{Deserialize, Serialize};
use crate::pipe_data::*;
use crate::catalog::PipeCatalog;

/// 管線工具類型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipingTool {
    /// 畫管線（click-click 連續）
    DrawPipe,
    /// 放置管件
    PlaceFitting,
    /// 選取/編輯管線
    EditPipe,
}

impl PipingTool {
    pub fn id(&self) -> &'static str {
        match self {
            Self::DrawPipe => "pipe_draw",
            Self::PlaceFitting => "pipe_fitting",
            Self::EditPipe => "pipe_edit",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::DrawPipe => "畫管線",
            Self::PlaceFitting => "管件",
            Self::EditPipe => "編輯管線",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "pipe_draw" => Some(Self::DrawPipe),
            "pipe_fitting" => Some(Self::PlaceFitting),
            "pipe_edit" => Some(Self::EditPipe),
            _ => None,
        }
    }
}

/// 管線繪製狀態
#[derive(Debug, Clone)]
pub enum PipeDrawState {
    /// 等待第一次點擊
    Idle,
    /// 已設定起點，等待終點（連續畫管）
    Drawing { start: [f32; 3], segments_drawn: usize },
}

/// 管線模組的編輯器狀態
#[derive(Debug, Clone)]
pub struct PipingState {
    /// 目前管線工具
    pub tool: PipingTool,
    /// 繪製狀態
    pub draw_state: PipeDrawState,
    /// 目前選擇的管線系統
    pub current_system: PipeSystem,
    /// 目前選擇的管徑規格索引
    pub current_spec_idx: usize,
    /// 目前選擇的管件種類
    pub current_fitting: FittingKind,
    /// 管線資料儲存
    pub store: PipingStore,
    /// 繪製高度 (mm)
    pub draw_height: f32,
}

impl Default for PipingState {
    fn default() -> Self {
        Self {
            tool: PipingTool::DrawPipe,
            draw_state: PipeDrawState::Idle,
            current_system: PipeSystem::PvcWater,
            current_spec_idx: 2, // DN25 預設
            current_fitting: FittingKind::Elbow90,
            store: PipingStore::default(),
            draw_height: 2700.0, // 預設管線高度 2.7m（天花板下）
        }
    }
}

impl PipingState {
    /// 取得目前選中的管材規格
    pub fn current_spec(&self) -> PipeSpec {
        let specs = PipeCatalog::specs_for(self.current_system);
        specs.into_iter()
            .nth(self.current_spec_idx)
            .unwrap_or_else(|| PipeCatalog::default_spec(self.current_system))
    }

    /// 處理點擊事件（畫管線）
    pub fn on_click(
        &mut self,
        scene: &mut kolibri_core::scene::Scene,
        ground_pos: [f32; 3],
    ) {
        let pos = [ground_pos[0], self.draw_height, ground_pos[2]];

        match self.tool {
            PipingTool::DrawPipe => {
                match &self.draw_state {
                    PipeDrawState::Idle => {
                        self.draw_state = PipeDrawState::Drawing {
                            start: pos,
                            segments_drawn: 0,
                        };
                    }
                    PipeDrawState::Drawing { start, segments_drawn } => {
                        let start = *start;
                        let count = *segments_drawn;
                        let spec = self.current_spec();
                        let name = format!("{}_{}", spec.system.label(), count + 1);

                        // 建立管段
                        let obj_id = crate::geometry::create_pipe_segment(
                            scene, &spec, start, pos, name,
                        );

                        if !obj_id.is_empty() {
                            // 記錄管段
                            let seg_id = self.store.next_id();
                            let seg_data_id = seg_id.clone();
                            self.store.segments.insert(seg_id, PipeSegment {
                                id: seg_data_id,
                                spec: spec.clone(),
                                start,
                                end: pos,
                                scene_object_id: obj_id,
                            });

                            // 連續畫：終點變起點
                            self.draw_state = PipeDrawState::Drawing {
                                start: pos,
                                segments_drawn: count + 1,
                            };
                        }
                    }
                }
            }
            PipingTool::PlaceFitting => {
                let spec = self.current_spec();
                let name = format!("{} {}", self.current_fitting.label(), spec.spec_name);
                let obj_id = crate::geometry::create_fitting(
                    scene, self.current_fitting, &spec, pos, name,
                );
                if !obj_id.is_empty() {
                    let fit_id = self.store.next_id();
                    let fit_data_id = fit_id.clone();
                    self.store.fittings.insert(fit_id, PipeFitting {
                        id: fit_data_id,
                        kind: self.current_fitting,
                        spec,
                        position: pos,
                        rotation_y: 0.0,
                        scene_object_id: obj_id,
                    });
                }
            }
            PipingTool::EditPipe => {
                // 選取管線（由 app 的 pick 處理）
            }
        }
    }

    /// 取消目前繪製
    pub fn cancel(&mut self) {
        self.draw_state = PipeDrawState::Idle;
    }

    /// 狀態文字
    pub fn status_text(&self) -> String {
        let spec = self.current_spec();
        match &self.draw_state {
            PipeDrawState::Idle => {
                match self.tool {
                    PipingTool::DrawPipe => format!("管線 — 點擊設定起點（{} {}）", spec.system.label(), spec.spec_name),
                    PipingTool::PlaceFitting => format!("管件 — 點擊放置 {} （{}）", self.current_fitting.label(), spec.spec_name),
                    PipingTool::EditPipe => "選取管線物件".to_string(),
                }
            }
            PipeDrawState::Drawing { segments_drawn, .. } => {
                format!("管線 — 點擊設定終點（已畫 {} 段，ESC 結束）", segments_drawn)
            }
        }
    }
}
