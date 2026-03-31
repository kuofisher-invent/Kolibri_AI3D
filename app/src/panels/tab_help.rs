//! 說明分頁 — 操作說明 + 法規/規格查詢

use eframe::egui;
use super::material_swatches::{section_frame_full, section_header_text};
use crate::app::KolibriApp;

impl KolibriApp {
    pub(crate) fn tab_help(&mut self, ui: &mut egui::Ui) {
        // ── 分類 tabs ──
        ui.horizontal(|ui| {
            let cats = [("操作", 0), ("快捷鍵", 1), ("管線規格", 2), ("鋼構規格", 3), ("法規", 4)];
            for (label, idx) in &cats {
                let active = self.help_category == *idx;
                let btn = if active {
                    egui::Button::new(egui::RichText::new(*label).size(10.0).strong().color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(76, 139, 245))
                        .rounding(8.0)
                } else {
                    egui::Button::new(egui::RichText::new(*label).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)))
                        .fill(egui::Color32::TRANSPARENT)
                        .rounding(8.0)
                };
                if ui.add(btn).clicked() {
                    self.help_category = *idx;
                }
            }
        });
        ui.add_space(6.0);

        match self.help_category {
            0 => self.help_operations(ui),
            1 => self.help_shortcuts(ui),
            2 => self.help_pipe_specs(ui),
            3 => self.help_steel_specs(ui),
            4 => self.help_regulations(ui),
            _ => {}
        }
    }

    fn help_operations(&self, ui: &mut egui::Ui) {
        section_frame_full(ui, |ui| {
            section_header_text(ui, "基本操作");
            help_row(ui, "中鍵拖曳", "旋轉視角 (Orbit)");
            help_row(ui, "Shift + 中鍵", "平移視角 (Pan)");
            help_row(ui, "滾輪", "縮放（朝游標方向）");
            help_row(ui, "左鍵點擊", "選取物件 / 執行工具動作");
            help_row(ui, "左鍵拖曳", "框選（左→右 窗選，右→左 交叉選）");
            help_row(ui, "右鍵", "右鍵選單");
            help_row(ui, "ESC", "取消目前操作 / 回到選取工具");
            help_row(ui, "Enter", "確認輸入 / 完成路徑");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "繪圖工具");
            help_row(ui, "Line (L)", "連續點擊畫線段，ESC 結束");
            help_row(ui, "Arc (A)", "三點定弧：起點→終點→凸度");
            help_row(ui, "Rectangle (R)", "點兩角，自動切換推拉");
            help_row(ui, "Circle (C)", "圓心→半徑→高度");
            help_row(ui, "PushPull (P)", "點擊面 → 拖曳拉伸");
            help_row(ui, "Offset (F)", "點擊面 → 拖曳內縮/外擴");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "VCB 尺寸輸入");
            help_row(ui, "1000", "1000 mm（預設單位）");
            help_row(ui, "1m", "1 公尺 = 1000 mm");
            help_row(ui, "100cm", "100 公分 = 1000 mm");
            help_row(ui, "3ft", "3 英尺 = 914.4 mm");
            help_row(ui, "3'6\"", "3 呎 6 吋 = 1066.8 mm");
            help_row(ui, "1000,2000", "寬 1000, 深 2000（逗號分隔）");
            help_row(ui, "5x", "陣列複製 5 份（Ctrl+Move 後）");
            help_row(ui, "6r", "極座標陣列 6 份（Ctrl+Move 後）");
        });
    }

    fn help_shortcuts(&self, ui: &mut egui::Ui) {
        section_frame_full(ui, |ui| {
            section_header_text(ui, "工具快捷鍵");
            let shortcuts = [
                ("Space", "選取"), ("M", "移動"), ("Q", "旋轉"), ("S", "縮放"),
                ("L", "線段"), ("A", "弧線"), ("R", "矩形"), ("C", "圓形"),
                ("B", "方塊"), ("P", "推拉"), ("F", "偏移"), ("E", "橡皮擦"),
                ("T", "捲尺"), ("D", "標註"), ("O", "環繞"), ("H", "平移"),
                ("Z", "全部顯示"), ("W", "牆工具"), ("G", "群組"),
            ];
            for (key, desc) in &shortcuts {
                help_row(ui, key, desc);
            }
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "系統快捷鍵");
            help_row(ui, "Ctrl+Z", "復原");
            help_row(ui, "Ctrl+Y", "重做");
            help_row(ui, "Ctrl+S", "儲存");
            help_row(ui, "Ctrl+D", "就地複製");
            help_row(ui, "Ctrl+M", "鏡射 X");
            help_row(ui, "Ctrl+A", "全選");
            help_row(ui, "Alt+H", "隱藏選取");
            help_row(ui, "Delete", "刪除選取");
            help_row(ui, "1/2/3", "前視/俯視/等角視圖");
            help_row(ui, "5", "切換透視/正交");
            help_row(ui, "F12", "Console");
        });
    }

    fn help_pipe_specs(&self, ui: &mut egui::Ui) {
        let systems = [
            ("PVC 給水管", "CNS 4055 / ASTM D2241", "白色硬質 PVC，壓力管路用。台灣自來水工程常用。"),
            ("PVC 排水管", "CNS 1298", "灰色薄壁 PVC，重力排水用。台灣建築排水標準管材。"),
            ("EMT 電管", "CNS 6445 / ANSI C80.3", "薄壁鍍鋅鋼管，室內配線用。台灣電氣法規認可。"),
            ("消防鍍鋅鐵管", "CNS 4626 / ASTM A795", "SCH40 鍍鋅鐵管。依內政部消防署規定，消防灑水管路使用。"),
            ("碳鋼管", "CNS 6331 / ASTM A106", "SCH40 無縫碳鋼管，製程管路、蒸氣管路用。"),
            ("不鏽鋼管", "CNS 6259 / ASTM A312", "SCH10S SUS304/316，衛生管路、食品/化工製程用。"),
            ("銅管", "CNS 2433 / ASTM B88", "Type L 銅管，冷媒、瓦斯、醫用氣體管路。"),
        ];

        for (name, standard, desc) in &systems {
            section_frame_full(ui, |ui| {
                section_header_text(ui, name);
                ui.label(egui::RichText::new(format!("標準：{}", standard)).size(11.0).strong());
                ui.label(egui::RichText::new(*desc).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.add_space(4.0);

                // 規格表
                #[cfg(feature = "piping")]
                {
                    let system = match *name {
                        "PVC 給水管" => Some(kolibri_piping::PipeSystem::PvcWater),
                        "PVC 排水管" => Some(kolibri_piping::PipeSystem::PvcDrain),
                        "EMT 電管" => Some(kolibri_piping::PipeSystem::ElectricalConduit),
                        "消防鍍鋅鐵管" => Some(kolibri_piping::PipeSystem::IronFireSprinkler),
                        "碳鋼管" => Some(kolibri_piping::PipeSystem::SteelProcess),
                        "不鏽鋼管" => Some(kolibri_piping::PipeSystem::StainlessSteel),
                        "銅管" => Some(kolibri_piping::PipeSystem::Copper),
                        _ => None,
                    };
                    if let Some(sys) = system {
                        let specs = kolibri_piping::PipeCatalog::specs_for(sys);
                        // 表頭
                        ui.horizontal(|ui| {
                            let muted = egui::Color32::from_rgb(140, 148, 160);
                            ui.label(egui::RichText::new("規格名稱").size(10.0).color(muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new("壁厚").size(10.0).color(muted));
                                ui.label(egui::RichText::new("外徑").size(10.0).color(muted));
                            });
                        });
                        for spec in &specs {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&spec.spec_name).size(10.0));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(format!("{:.1}mm", spec.wall_thickness)).size(10.0));
                                    ui.label(egui::RichText::new(format!("Ø{:.1}", spec.outer_diameter)).size(10.0));
                                });
                            });
                        }
                    }
                }
            });
            ui.add_space(6.0);
        }
    }

    fn help_steel_specs(&self, ui: &mut egui::Ui) {
        section_frame_full(ui, |ui| {
            section_header_text(ui, "H 型鋼（CNS 386 / JIS G3192）");
            ui.label(egui::RichText::new("台灣常用熱軋 H 型鋼規格，依 CNS 386 / JIS G3192 標準。")
                .size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
            ui.label(egui::RichText::new("材質：SN400B / SN490B / SS400 / A572 Gr.50")
                .size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
            ui.add_space(6.0);

            // 表頭
            ui.horizontal(|ui| {
                let m = egui::Color32::from_rgb(140, 148, 160);
                ui.label(egui::RichText::new("規格").size(10.0).color(m));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("kg/m").size(10.0).color(m));
                    ui.label(egui::RichText::new("tw").size(10.0).color(m));
                    ui.label(egui::RichText::new("tf").size(10.0).color(m));
                });
            });

            let profiles = [
                // (名稱, H, B, tw, tf, 單位重)
                ("H100×100×6×8", 100, 100, 6.0, 8.0, 17.2),
                ("H125×125×6.5×9", 125, 125, 6.5, 9.0, 23.8),
                ("H150×75×5×7", 150, 75, 5.0, 7.0, 14.0),
                ("H150×150×7×10", 150, 150, 7.0, 10.0, 31.5),
                ("H175×175×7.5×11", 175, 175, 7.5, 11.0, 40.4),
                ("H200×100×5.5×8", 200, 100, 5.5, 8.0, 21.3),
                ("H200×200×8×12", 200, 200, 8.0, 12.0, 49.9),
                ("H244×175×7×11", 244, 175, 7.0, 11.0, 44.1),
                ("H248×124×5×8", 248, 124, 5.0, 8.0, 25.8),
                ("H250×250×9×14", 250, 250, 9.0, 14.0, 72.4),
                ("H294×200×8×12", 294, 200, 8.0, 12.0, 56.8),
                ("H298×149×5.5×8", 298, 149, 5.5, 8.0, 32.0),
                ("H300×300×10×15", 300, 300, 10.0, 15.0, 94.0),
                ("H340×250×9×14", 340, 250, 9.0, 14.0, 79.7),
                ("H346×174×6×9", 346, 174, 6.0, 9.0, 41.4),
                ("H350×350×12×19", 350, 350, 12.0, 19.0, 137.0),
                ("H390×300×10×16", 390, 300, 10.0, 16.0, 107.0),
                ("H394×199×7×11", 394, 199, 7.0, 11.0, 56.6),
                ("H396×199×7×11", 396, 199, 7.0, 11.0, 56.6),
                ("H400×400×13×21", 400, 400, 13.0, 21.0, 172.0),
                ("H446×199×8×12", 446, 199, 8.0, 12.0, 66.2),
                ("H450×200×9×14", 450, 200, 9.0, 14.0, 76.0),
                ("H488×300×11×18", 488, 300, 11.0, 18.0, 128.0),
                ("H496×199×9×14", 496, 199, 9.0, 14.0, 79.5),
                ("H500×200×10×16", 500, 200, 10.0, 16.0, 89.6),
                ("H500×500×16×25", 500, 500, 16.0, 25.0, 263.0),
                ("H582×300×12×17", 582, 300, 12.0, 17.0, 137.0),
                ("H588×300×12×20", 588, 300, 12.0, 20.0, 151.0),
                ("H594×302×14×23", 594, 302, 14.0, 23.0, 175.0),
                ("H600×200×11×17", 600, 200, 11.0, 17.0, 106.0),
                ("H700×300×13×24", 700, 300, 13.0, 24.0, 185.0),
                ("H800×300×14×26", 800, 300, 14.0, 26.0, 210.0),
                ("H900×300×16×28", 900, 300, 16.0, 28.0, 243.0),
            ];

            for (name, _h, _b, tw, tf, weight) in &profiles {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(*name).size(10.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0}", weight)).size(10.0));
                        ui.label(egui::RichText::new(format!("{:.0}", tw)).size(10.0));
                        ui.label(egui::RichText::new(format!("{:.0}", tf)).size(10.0));
                    });
                });
            }
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "鋼管（圓管 / 方管 / 矩管）");
            ui.label(egui::RichText::new("依 CNS 4435 / JIS G3466 標準。材質 STKR400/490。")
                .size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
            ui.add_space(4.0);

            let tubes = [
                "□50×50×2.3", "□60×60×2.3", "□75×75×3.2", "□100×100×3.2",
                "□100×100×4.5", "□125×125×4.5", "□125×125×6.0", "□150×150×4.5",
                "□150×150×6.0", "□175×175×6.0", "□200×200×6.0", "□200×200×8.0",
                "□250×250×6.0", "□250×250×9.0", "□300×300×9.0", "□300×300×12.0",
                "□350×350×12.0", "□400×400×12.0",
            ];
            for t in &tubes {
                ui.label(egui::RichText::new(*t).size(10.0));
            }
        });
    }

    fn help_regulations(&self, ui: &mut egui::Ui) {
        section_frame_full(ui, |ui| {
            section_header_text(ui, "建築相關法規");
            reg_item(ui, "建築技術規則", "內政部", "建築設計施工編 — 結構、防火、無障礙、機電");
            reg_item(ui, "建築物耐震設計規範", "內政部營建署", "耐震設計、地震力計算、隔震消能");
            reg_item(ui, "鋼構造建築物鋼結構設計技術規範", "內政部", "鋼構設計、接合、防火被覆");
            reg_item(ui, "混凝土結構設計規範", "內政部", "RC 結構設計、配筋、耐久性");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "消防法規");
            reg_item(ui, "各類場所消防安全設備設置標準", "內政部消防署", "滅火器、灑水、排煙、警報設備配置");
            reg_item(ui, "消防安全設備審核認可作業須知", "內政部消防署", "消防管路材質、口徑、間距規定");
            reg_item(ui, "灑水設備管路", "NFPA 13 / 消防署", "管路口徑：\n• 閉鎖型灑水頭 ≤8 顆：DN25\n• 9~18 顆：DN32\n• 19~36 顆：DN40\n• 幹管：DN50~DN150");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "給排水法規");
            reg_item(ui, "建築物給水排水設備設計技術規範", "內政部營建署", "給水管路設計、排水管路坡度、透氣管");
            reg_item(ui, "自來水法", "經濟部水利署", "自來水管路材質、接合、水壓規定");
            reg_item(ui, "排水坡度規定", "技術規範", "• 排水橫管 DN50 以下：1/50\n• DN65~DN150：1/100\n• DN200 以上：1/200");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "電氣法規");
            reg_item(ui, "屋內線路裝置規則", "經濟部", "配管、配線、接地、過載保護");
            reg_item(ui, "EMT 管路規定", "屋內線路裝置規則", "• 同一管路不超過 4 個 90° 彎\n• 管路總彎曲角度 ≤ 360°\n• 垂直管支撐間距 ≤ 3m");
        });
        ui.add_space(8.0);

        section_frame_full(ui, |ui| {
            section_header_text(ui, "鋼構法規");
            reg_item(ui, "鋼結構容許應力設計法 (ASD)", "內政部", "鋼構容許應力、挫屈、螺栓/焊接接合");
            reg_item(ui, "鋼結構極限設計法 (LRFD)", "內政部", "荷重組合、強度設計、耐震設計");
            reg_item(ui, "鋼材規格", "CNS 標準", "• SN400B：降伏 235 MPa\n• SN490B：降伏 325 MPa\n• SS400：降伏 245 MPa\n• A572 Gr.50：降伏 345 MPa");
        });
    }
}

fn help_row(ui: &mut egui::Ui, key: &str, desc: &str) {
    ui.horizontal(|ui| {
        let key_bg = egui::Color32::from_rgb(235, 237, 243);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(70.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, key_bg);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
            key, egui::FontId::monospace(10.0), egui::Color32::from_rgb(50, 60, 80));
        ui.label(egui::RichText::new(desc).size(11.0));
    });
}

fn reg_item(ui: &mut egui::Ui, title: &str, source: &str, desc: &str) {
    ui.label(egui::RichText::new(title).size(11.0).strong());
    ui.label(egui::RichText::new(format!("來源：{}", source)).size(10.0)
        .color(egui::Color32::from_rgb(76, 139, 245)));
    ui.label(egui::RichText::new(desc).size(10.0)
        .color(egui::Color32::from_rgb(110, 118, 135)));
    ui.add_space(6.0);
}
