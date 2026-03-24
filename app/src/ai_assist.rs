//! AI Assistant — contextual suggestions based on user actions

use crate::scene::{Scene, Shape};
use crate::app::Tool;

#[derive(Debug, Clone)]
pub struct AiSuggestionItem {
    pub icon: &'static str,
    pub text: String,
    pub detail: String,
    pub action: Option<SuggestedAction>,
}

#[derive(Debug, Clone)]
pub enum SuggestedAction {
    SwitchTool(Tool),
    SetDimension { obj_id: String, axis: u8, value: f32 },
    ApplyMaterial { obj_id: String, material: String },
}

/// Analyze current scene + user state and generate suggestions
pub fn generate_suggestions(
    scene: &Scene,
    tool: Tool,
    selected_ids: &[String],
    last_action: &str,
) -> Vec<AiSuggestionItem> {
    let mut suggestions = Vec::new();

    // === Context: Empty scene ===
    if scene.objects.is_empty() && scene.free_mesh.vertices.is_empty() {
        suggestions.push(AiSuggestionItem {
            icon: "\u{1f4a1}",
            text: "\u{958b}\u{59cb}\u{5efa}\u{6a21}".into(),
            detail: "\u{6309} B \u{5efa}\u{7acb}\u{65b9}\u{584a}\u{ff0c}\u{6216}\u{6309} L \u{756b}\u{7dda}\u{6bb5}".into(),
            action: Some(SuggestedAction::SwitchTool(Tool::CreateBox)),
        });
        return suggestions;
    }

    // === Context: Object just created ===
    if last_action.starts_with("create") {
        if let Some(id) = selected_ids.first() {
            if let Some(obj) = scene.objects.get(id) {
                // Suggest push/pull if it's a box
                if matches!(obj.shape, Shape::Box { .. }) {
                    suggestions.push(AiSuggestionItem {
                        icon: "\u{2b06}",
                        text: "\u{63a8}\u{62c9}\u{9762}".into(),
                        detail: "\u{6309} P \u{5207}\u{63db}\u{63a8}\u{62c9}\u{5de5}\u{5177}\u{ff0c}\u{9ede}\u{64ca}\u{9762}\u{62d6}\u{66f3}".into(),
                        action: Some(SuggestedAction::SwitchTool(Tool::PushPull)),
                    });
                }

                // Suggest setting material
                suggestions.push(AiSuggestionItem {
                    icon: "\u{1f3a8}",
                    text: "\u{8a2d}\u{5b9a}\u{6750}\u{8cea}".into(),
                    detail: format!("\u{76ee}\u{524d}\u{6750}\u{8cea}: {}\u{ff0c}\u{53ef}\u{5728}\u{53f3}\u{5074}\u{9762}\u{677f}\u{66f4}\u{63db}", obj.material.label()),
                    action: None,
                });
            }
        }
    }

    // === Context: Multiple objects, suggest grouping ===
    if scene.objects.len() >= 3 && scene.groups.is_empty() {
        suggestions.push(AiSuggestionItem {
            icon: "\u{1f4c1}",
            text: "\u{5efa}\u{7acb}\u{7fa4}\u{7d44}".into(),
            detail: format!("\u{5834}\u{666f}\u{6709} {} \u{500b}\u{7269}\u{4ef6}\u{ff0c}\u{5efa}\u{8b70}\u{5206}\u{7d44}\u{7ba1}\u{7406}", scene.objects.len()),
            action: None,
        });
    }

    // === Context: Object selected, suggest common next actions ===
    if !selected_ids.is_empty() {
        if let Some(id) = selected_ids.first() {
            if let Some(obj) = scene.objects.get(id) {
                match tool {
                    Tool::Select => {
                        // Suggest move or push/pull
                        suggestions.push(AiSuggestionItem {
                            icon: "\u{2725}",
                            text: "\u{79fb}\u{52d5}\u{7269}\u{4ef6}".into(),
                            detail: "\u{6309} M \u{5207}\u{63db}\u{79fb}\u{52d5}\u{5de5}\u{5177}\u{ff0c}Ctrl \u{5207}\u{63db}\u{8ef8}\u{5411}".into(),
                            action: Some(SuggestedAction::SwitchTool(Tool::Move)),
                        });
                    }
                    Tool::PushPull => {
                        // Suggest standard dimensions
                        if let Shape::Box { height, .. } = &obj.shape {
                            if (*height - 2800.0).abs() > 100.0 {
                                suggestions.push(AiSuggestionItem {
                                    icon: "\u{1f4d0}",
                                    text: "\u{6a19}\u{6e96}\u{5c64}\u{9ad8}".into(),
                                    detail: "\u{5efa}\u{8b70}\u{9ad8}\u{5ea6} 2800 mm\u{ff08}\u{6a19}\u{6e96}\u{6a13}\u{5c64}\u{6de8}\u{9ad8}\u{ff09}".into(),
                                    action: Some(SuggestedAction::SetDimension {
                                        obj_id: id.clone(), axis: 1, value: 2800.0
                                    }),
                                });
                            }
                            if (*height - 200.0).abs() < 10.0 || (*height - 150.0).abs() < 10.0 {
                                suggestions.push(AiSuggestionItem {
                                    icon: "\u{1f9f1}",
                                    text: "\u{5075}\u{6e2c}\u{5230}\u{7246}\u{9ad4}".into(),
                                    detail: "\u{5efa}\u{8b70}\u{8a2d}\u{70ba}\u{78da}\u{6750}\u{6216}\u{6df7}\u{51dd}\u{571f}\u{6750}\u{8cea}".into(),
                                    action: Some(SuggestedAction::ApplyMaterial {
                                        obj_id: id.clone(), material: "brick".into()
                                    }),
                                });
                            }
                        }
                    }
                    _ => {}
                }

                // Alignment suggestions
                check_alignment_suggestions(scene, obj, &mut suggestions);
            }
        }
    }

    // === Context: Drawing lines ===
    if matches!(tool, Tool::Line) && scene.free_mesh.edges.len() > 2 {
        let face_count = scene.free_mesh.faces.len();
        if face_count == 0 {
            suggestions.push(AiSuggestionItem {
                icon: "\u{1f537}",
                text: "\u{5c01}\u{9589}\u{7dda}\u{6bb5}".into(),
                detail: "\u{56de}\u{5230}\u{8d77}\u{9ede}\u{5c01}\u{9589}\u{5f62}\u{6210}\u{9762}\u{ff0c}\u{5373}\u{53ef}\u{63a8}\u{62c9}\u{6210} 3D".into(),
                action: None,
            });
        } else {
            suggestions.push(AiSuggestionItem {
                icon: "\u{2b06}",
                text: format!("\u{5df2}\u{5f62}\u{6210} {} \u{500b}\u{9762}", face_count),
                detail: "\u{6309} P \u{5207}\u{63db}\u{63a8}\u{62c9}\u{5de5}\u{5177}\u{62c9}\u{4f38}\u{6210} 3D \u{5be6}\u{9ad4}".into(),
                action: Some(SuggestedAction::SwitchTool(Tool::PushPull)),
            });
        }
    }

    suggestions.truncate(3); // max 3 suggestions
    suggestions
}

fn check_alignment_suggestions(
    scene: &Scene,
    obj: &crate::scene::SceneObject,
    suggestions: &mut Vec<AiSuggestionItem>,
) {
    let p = obj.position;
    let _dims = match &obj.shape {
        Shape::Box { width, height: _, depth } => (*width, *depth),
        _ => return,
    };

    // Check if this object is almost aligned with another
    for other in scene.objects.values() {
        if other.id == obj.id { continue; }
        let op = other.position;

        // Check X alignment (within 50mm)
        let x_diff = (p[0] - op[0]).abs();
        if x_diff > 0.1 && x_diff < 50.0 {
            suggestions.push(AiSuggestionItem {
                icon: "\u{1f4cf}",
                text: "\u{63a5}\u{8fd1}\u{5c0d}\u{9f4a}".into(),
                detail: format!("\u{8207} {} \u{7684} X \u{8ef8}\u{5dee} {:.0}mm\u{ff0c}\u{5efa}\u{8b70}\u{5c0d}\u{9f4a}", other.name, x_diff),
                action: Some(SuggestedAction::SetDimension {
                    obj_id: obj.id.clone(), axis: 0, value: op[0]
                }),
            });
            break;
        }

        // Check Z alignment
        let z_diff = (p[2] - op[2]).abs();
        if z_diff > 0.1 && z_diff < 50.0 {
            suggestions.push(AiSuggestionItem {
                icon: "\u{1f4cf}",
                text: "\u{63a5}\u{8fd1}\u{5c0d}\u{9f4a}".into(),
                detail: format!("\u{8207} {} \u{7684} Z \u{8ef8}\u{5dee} {:.0}mm\u{ff0c}\u{5efa}\u{8b70}\u{5c0d}\u{9f4a}", other.name, z_diff),
                action: Some(SuggestedAction::SetDimension {
                    obj_id: obj.id.clone(), axis: 2, value: op[2]
                }),
            });
            break;
        }
    }
}
