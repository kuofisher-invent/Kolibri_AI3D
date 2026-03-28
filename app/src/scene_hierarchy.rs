use std::collections::BTreeSet;

use eframe::egui;
use glam::Vec3;

use crate::app::{KolibriApp, RightTab, Tool};
use crate::scene::{ComponentDef, Shape};

impl KolibriApp {
    pub(crate) fn current_component_edit_summary(&self) -> Option<String> {
        let def_id = self.editor.editing_component_def_id.as_deref()?;
        let def = self.scene.component_defs.get(def_id)?;
        let total = self.scene.component_instance_count(def_id);
        let visible = self.scene.component_visible_instance_count(def_id);
        Some(format!("實例 {}/{} 可見 | 定義物件 {}", visible, total, def.objects.len()))
    }

    pub(crate) fn finish_component_editing(&mut self, sync_changes: bool) {
        let Some(def_id) = self.editor.editing_component_def_id.clone() else {
            return;
        };

        let def_name = self
            .scene
            .component_defs
            .get(&def_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| def_id.clone());

        let instance_ids = self.scene.component_instance_ids(&def_id);
        let representative_id = self
            .editor
            .selected_ids
            .iter()
            .find(|id| {
                self.scene
                    .objects
                    .get(*id)
                    .and_then(|obj| obj.component_def_id.as_deref())
                    == Some(def_id.as_str())
            })
            .cloned()
            .or_else(|| instance_ids.first().cloned());

        if sync_changes {
            if let Some(source_id) = representative_id.as_deref() {
                self.scene.auto_sync_component(source_id);
            }
        }

        self.editor.editing_component_def_id = None;
        self.editor.selected_ids = representative_id.into_iter().collect();
        self.right_tab = RightTab::Properties;
        self.file_message = Some((
            if sync_changes {
                format!("已完成元件編輯同步: {}", def_name)
            } else {
                format!("已退出元件編輯: {}", def_name)
            },
            std::time::Instant::now(),
        ));
    }

    pub(crate) fn render_scene_hierarchy(&mut self, ui: &mut egui::Ui) {
        let mut to_delete = None;

        self.render_component_library(ui, &mut to_delete);

        if !self.scene.component_defs.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
        }

        self.render_scene_tree(ui, &mut to_delete);

        if let Some(id) = to_delete {
            self.editor.selected_ids.retain(|selected| selected != &id);
            self.scene.delete(&id);
        }
    }

    fn render_component_library(&mut self, ui: &mut egui::Ui, to_delete: &mut Option<String>) {
        if self.scene.component_defs.is_empty() {
            return;
        }

        ui.label(egui::RichText::new("元件定義").strong());

        if let Some(active_def_id) = self.editor.editing_component_def_id.clone() {
            if let Some(active_def) = self.scene.component_defs.get(&active_def_id).cloned() {
                let active_instance_ids = self.scene.component_instance_ids(&active_def_id);
                let visible_count = self.scene.component_visible_instance_count(&active_def_id);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "元件編輯中: {} ({}/{})",
                                active_def.name,
                                visible_count,
                                active_instance_ids.len()
                            ))
                            .strong(),
                        );
                        ui.small(format!("ID: {}", active_def.id));
                    });

                    if let Some(summary) = self.current_component_edit_summary() {
                        ui.small(summary);
                    }

                    ui.horizontal(|ui| {
                        if ui.small_button("聚焦").clicked() {
                            self.editor.selected_ids = active_instance_ids.clone();
                            self.focus_on_objects(&active_instance_ids);
                            self.right_tab = RightTab::Properties;
                        }
                        if ui.small_button("選取全部").clicked() {
                            self.editor.selected_ids = active_instance_ids.clone();
                            self.right_tab = RightTab::Properties;
                        }
                        if ui.small_button("完成同步").clicked() {
                            self.finish_component_editing(true);
                        }
                        if ui.small_button("退出").clicked() {
                            self.finish_component_editing(false);
                        }
                    });
                });

                ui.add_space(6.0);
            }
        }

        let mut def_ids: Vec<String> = self.scene.component_defs.keys().cloned().collect();
        def_ids.sort_by(|a, b| self.component_def_name(a).cmp(self.component_def_name(b)));

        for def_id in def_ids {
            self.render_component_def_node(ui, &def_id, to_delete);
        }
    }

    fn render_component_def_node(
        &mut self,
        ui: &mut egui::Ui,
        def_id: &str,
        to_delete: &mut Option<String>,
    ) {
        let Some(def) = self.scene.component_defs.get(def_id).cloned() else {
            return;
        };

        let mut instance_ids = self.scene.component_instance_ids(def_id);
        instance_ids.sort_by(|a, b| self.object_name(a).cmp(self.object_name(b)));
        let visible_count = self.scene.component_visible_instance_count(def_id);
        let is_editing = self.editor.editing_component_def_id.as_deref() == Some(def.id.as_str());

        let title = format!(
            "[元件] {} ({}/{}){}",
            def.name,
            visible_count,
            instance_ids.len(),
            if is_editing { " [編輯中]" } else { "" }
        );

        egui::CollapsingHeader::new(title)
            .id_source(("component_def", def_id))
            .default_open(is_editing)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("編輯").clicked() {
                        if let Some(first_id) = instance_ids.first() {
                            self.editor.selected_ids = vec![first_id.clone()];
                            self.editor.editing_component_def_id = Some(def.id.clone());
                            self.right_tab = RightTab::Properties;
                            self.file_message = Some((
                                format!("開始編輯元件定義: {}", def.name),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    if ui.small_button("主選").clicked() {
                        if let Some(first_id) = instance_ids.first() {
                            self.editor.selected_ids = vec![first_id.clone()];
                            if self.editor.editing_component_def_id.as_deref() != Some(def.id.as_str()) {
                                self.editor.editing_component_def_id = None;
                            }
                            self.right_tab = RightTab::Properties;
                        }
                    }
                    if ui.small_button("選取全部").clicked() {
                        self.editor.selected_ids = instance_ids.clone();
                        if self.editor.editing_component_def_id.as_deref() != Some(def.id.as_str()) {
                            self.editor.editing_component_def_id = None;
                        }
                        self.right_tab = RightTab::Properties;
                    }
                    if ui.small_button("聚焦").clicked() {
                        self.editor.selected_ids = instance_ids.clone();
                        if self.editor.editing_component_def_id.as_deref() != Some(def.id.as_str()) {
                            self.editor.editing_component_def_id = None;
                        }
                        self.focus_on_objects(&instance_ids);
                        self.right_tab = RightTab::Properties;
                    }
                    if ui.small_button("顯示").clicked() {
                        self.scene.set_component_instances_visible(def_id, true);
                    }
                    if ui.small_button("隱藏").clicked() {
                        self.scene.set_component_instances_visible(def_id, false);
                    }
                    ui.label(format!("ID: {}", def.id));
                });

                if !def.objects.is_empty() {
                    ui.label(format!("定義物件數: {}", def.objects.len()));
                    ui.small(self.component_def_summary(&def));
                }

                for object_id in &instance_ids {
                    self.render_object_node(ui, object_id, 16.0, to_delete);
                }
            });
    }

    fn render_scene_tree(&mut self, ui: &mut egui::Ui, to_delete: &mut Option<String>) {
        ui.label(egui::RichText::new("場景階層").strong());

        let mut root_group_ids: Vec<String> = self
            .scene
            .groups
            .values()
            .filter(|group| group.parent_id.is_none())
            .map(|group| group.id.clone())
            .collect();
        root_group_ids.sort_by(|a, b| self.group_name(a).cmp(self.group_name(b)));

        for group_id in root_group_ids {
            self.render_group_node(ui, &group_id, 0.0, to_delete);
        }

        let mut root_object_ids: Vec<String> = self
            .scene
            .objects
            .values()
            .filter(|obj| obj.parent_id.is_none())
            .map(|obj| obj.id.clone())
            .collect();
        root_object_ids.sort_by(|a, b| self.object_name(a).cmp(self.object_name(b)));

        for object_id in root_object_ids {
            self.render_object_node(ui, &object_id, 0.0, to_delete);
        }
    }

    fn render_group_node(
        &mut self,
        ui: &mut egui::Ui,
        group_id: &str,
        indent: f32,
        to_delete: &mut Option<String>,
    ) {
        let Some(group) = self.scene.groups.get(group_id).cloned() else {
            return;
        };

        let selection_ids = self.collect_group_selection_ids(group_id);

        ui.horizontal(|ui| {
            ui.add_space(indent);
            let label = format!("[群組] {} ({})", group.name, selection_ids.len());
            if ui.selectable_label(false, label).clicked() {
                if let Some(active_def_id) = self.editor.editing_component_def_id.as_deref() {
                    let touches_active_component = selection_ids.iter().any(|id| {
                        self.scene
                            .objects
                            .get(id)
                            .and_then(|obj| obj.component_def_id.as_deref())
                            == Some(active_def_id)
                    });
                    if !touches_active_component {
                        self.file_message = Some((
                            "目前正在編輯元件，請先完成同步或退出編輯模式".into(),
                            std::time::Instant::now(),
                        ));
                        return;
                    }
                }
                self.editor.selected_ids = selection_ids.clone();
                self.right_tab = RightTab::Properties;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("解散").clicked() {
                    self.scene.dissolve_group(&group.id);
                }
            });
        });

        let mut child_object_ids = group.children.clone();
        child_object_ids.sort_by(|a, b| self.object_name(a).cmp(self.object_name(b)));
        for child_id in child_object_ids {
            self.render_object_node(ui, &child_id, indent + 16.0, to_delete);
        }

        let mut nested_group_ids: Vec<String> = self
            .scene
            .groups
            .values()
            .filter(|nested| nested.parent_id.as_deref() == Some(group_id))
            .map(|nested| nested.id.clone())
            .collect();
        nested_group_ids.sort_by(|a, b| self.group_name(a).cmp(self.group_name(b)));
        for nested_group_id in nested_group_ids {
            self.render_group_node(ui, &nested_group_id, indent + 16.0, to_delete);
        }
    }

    fn render_object_node(
        &mut self,
        ui: &mut egui::Ui,
        object_id: &str,
        indent: f32,
        to_delete: &mut Option<String>,
    ) {
        let Some(obj) = self.scene.objects.get(object_id).cloned() else {
            return;
        };

        let icon = match obj.shape {
            Shape::Box { .. } => "[Box]",
            Shape::Cylinder { .. } => "[Cyl]",
            Shape::Sphere { .. } => "[Sph]",
            Shape::Line { .. } => "[Line]",
            Shape::Mesh(_) => "[Mesh]",
        };
        let label = if let Some(def_id) = &obj.component_def_id {
            format!("{} {} <{}>", icon, obj.name, def_id)
        } else {
            format!("{} {}", icon, obj.name)
        };
        let is_renaming = self.editor.renaming_id.as_ref() == Some(&obj.id);

        ui.horizontal(|ui| {
            ui.add_space(indent);

            if is_renaming {
                let resp = ui.text_edit_singleline(&mut self.editor.rename_buf);
                if !resp.has_focus() {
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    if !self.editor.rename_buf.is_empty() {
                        if let Some(target) = self.scene.objects.get_mut(object_id) {
                            target.name = self.editor.rename_buf.clone();
                            self.scene.version += 1;
                        }
                    }
                    self.editor.renaming_id = None;
                    self.editor.rename_buf.clear();
                }
            } else {
                let selected = self
                    .editor
                    .selected_ids
                    .iter()
                    .any(|selected| selected == object_id);
                let resp = ui.selectable_label(selected, label);
                if resp.clicked() {
                    if let Some(active_def_id) = self.editor.editing_component_def_id.as_deref() {
                        if obj.component_def_id.as_deref() != Some(active_def_id) {
                            self.file_message = Some((
                                "目前正在編輯元件，請先完成同步或退出編輯模式".into(),
                                std::time::Instant::now(),
                            ));
                            return;
                        }
                    }
                    self.editor.selected_ids = vec![obj.id.clone()];
                    self.right_tab = RightTab::Properties;
                }
                if resp.double_clicked() {
                    self.editor.renaming_id = Some(obj.id.clone());
                    self.editor.rename_buf = obj.name.clone();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let eye = if obj.visible { "顯示" } else { "隱藏" };
                if ui.small_button("刪除").clicked() {
                    *to_delete = Some(obj.id.clone());
                }
                if ui.small_button(eye).clicked() {
                    if let Some(target) = self.scene.objects.get_mut(object_id) {
                        target.visible = !target.visible;
                        self.scene.version += 1;
                    }
                }
            });
        });

        let mut child_object_ids: Vec<String> = self
            .scene
            .objects
            .values()
            .filter(|child| child.parent_id.as_deref() == Some(object_id))
            .map(|child| child.id.clone())
            .collect();
        child_object_ids.sort_by(|a, b| self.object_name(a).cmp(self.object_name(b)));
        for child_id in child_object_ids {
            self.render_object_node(ui, &child_id, indent + 16.0, to_delete);
        }
    }

    fn collect_group_selection_ids(&self, group_id: &str) -> Vec<String> {
        let mut ids = Vec::new();

        if let Some(group) = self.scene.groups.get(group_id) {
            for child_id in &group.children {
                ids.push(child_id.clone());
                ids.extend(self.scene.descendants_of(child_id));
            }
        }

        let nested_group_ids: Vec<String> = self
            .scene
            .groups
            .values()
            .filter(|nested| nested.parent_id.as_deref() == Some(group_id))
            .map(|nested| nested.id.clone())
            .collect();
        for nested_group_id in nested_group_ids {
            ids.extend(self.collect_group_selection_ids(&nested_group_id));
        }

        ids.sort();
        ids.dedup();
        ids
    }

    fn component_def_name<'a>(&'a self, component_def_id: &'a str) -> &'a str {
        self.scene
            .component_defs
            .get(component_def_id)
            .map(|component_def| component_def.name.as_str())
            .unwrap_or("")
    }

    fn component_def_summary(&self, def: &ComponentDef) -> String {
        let mut mesh_count = 0usize;
        let mut box_count = 0usize;
        let mut cylinder_count = 0usize;
        let mut sphere_count = 0usize;
        let mut line_count = 0usize;
        let mut materials = BTreeSet::new();

        for obj in &def.objects {
            match obj.shape {
                Shape::Box { .. } => box_count += 1,
                Shape::Cylinder { .. } => cylinder_count += 1,
                Shape::Sphere { .. } => sphere_count += 1,
                Shape::Line { .. } => line_count += 1,
                Shape::Mesh(_) => mesh_count += 1,
            }
            materials.insert(obj.material.label().to_string());
        }

        let mut shape_parts = Vec::new();
        if mesh_count > 0 {
            shape_parts.push(format!("mesh {}", mesh_count));
        }
        if box_count > 0 {
            shape_parts.push(format!("box {}", box_count));
        }
        if cylinder_count > 0 {
            shape_parts.push(format!("cyl {}", cylinder_count));
        }
        if sphere_count > 0 {
            shape_parts.push(format!("sphere {}", sphere_count));
        }
        if line_count > 0 {
            shape_parts.push(format!("line {}", line_count));
        }

        let mut material_list: Vec<String> = materials.into_iter().collect();
        if material_list.len() > 3 {
            material_list.truncate(3);
            material_list.push("...".to_string());
        }

        format!(
            "幾何: {} | 材質: {}",
            if shape_parts.is_empty() {
                "-".to_string()
            } else {
                shape_parts.join(", ")
            },
            if material_list.is_empty() {
                "-".to_string()
            } else {
                material_list.join(", ")
            }
        )
    }

    fn group_name<'a>(&'a self, group_id: &'a str) -> &'a str {
        self.scene
            .groups
            .get(group_id)
            .map(|group| group.name.as_str())
            .unwrap_or("")
    }

    fn object_name<'a>(&'a self, object_id: &'a str) -> &'a str {
        self.scene
            .objects
            .get(object_id)
            .map(|obj| obj.name.as_str())
            .unwrap_or("")
    }

    pub(crate) fn focus_on_objects(&mut self, object_ids: &[String]) {
        if object_ids.is_empty() {
            self.zoom_extents();
            return;
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut found = false;

        for object_id in object_ids {
            let Some(obj) = self.scene.objects.get(object_id) else {
                continue;
            };
            found = true;
            let p = Vec3::from(obj.position);
            let s = match &obj.shape {
                Shape::Box {
                    width,
                    height,
                    depth,
                } => Vec3::new(*width, *height, *depth),
                Shape::Cylinder { radius, height, .. } => {
                    Vec3::new(*radius * 2.0, *height, *radius * 2.0)
                }
                Shape::Sphere { radius, .. } => Vec3::splat(*radius * 2.0),
                Shape::Line { points, .. } => {
                    let mut mx = Vec3::ZERO;
                    for pt in points {
                        mx = mx.max(Vec3::from(*pt) - p);
                    }
                    mx
                }
                Shape::Mesh(mesh) => {
                    let (mmin, mmax) = mesh.aabb();
                    Vec3::from(mmax) - Vec3::from(mmin)
                }
            };
            min = min.min(p);
            max = max.max(p + s);
        }

        if !found {
            self.zoom_extents();
            return;
        }

        let center = (min + max) * 0.5;
        let extent = (max - min).length().max(100.0);
        self.viewer.camera.target = center;
        self.viewer.camera.distance = extent * 1.5;
        self.editor.tool = Tool::Select;
    }
}
