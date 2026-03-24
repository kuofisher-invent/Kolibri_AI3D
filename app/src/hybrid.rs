//! Hybrid workflow: parse natural-language building commands
//! "建一面 5m x 3m 的牆" -> create_box(5000, 3000, 200)

use crate::scene::{Scene, MaterialKind};

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub description: String,
    pub action: HybridAction,
}

#[derive(Debug, Clone)]
pub enum HybridAction {
    CreateWall { width: f32, height: f32, thickness: f32, material: MaterialKind },
    CreateFloor { width: f32, depth: f32, thickness: f32, material: MaterialKind },
    CreateColumn { radius: f32, height: f32, material: MaterialKind },
    CreateRoom { width: f32, depth: f32, height: f32 },
    SetHeight { target: String, height: f32 },
    Unknown(String),
}

/// Try to parse a Chinese/English building command
pub fn parse_command(input: &str) -> Option<ParsedCommand> {
    let input = input.trim().to_lowercase();

    // Parse dimensions (supports: 5m, 5000mm, 5000, 5.5m)
    let numbers = extract_numbers(&input);

    // Detect "牆" / "wall"
    if input.contains('\u{7246}') || input.contains("wall") {
        let (w, h, t) = match numbers.len() {
            0 => (5000.0, 2800.0, 200.0),
            1 => (numbers[0], 2800.0, 200.0),
            2 => (numbers[0], numbers[1], 200.0),
            _ => (numbers[0], numbers[1], numbers[2]),
        };
        return Some(ParsedCommand {
            description: format!("\u{5efa}\u{7acb}\u{7246}\u{9ad4} {}x{}x{} mm", w, h, t),
            action: HybridAction::CreateWall { width: w, height: h, thickness: t, material: MaterialKind::Concrete },
        });
    }

    // Detect "地板" / "floor" / "樓板"
    if input.contains("\u{5730}\u{677f}") || input.contains("floor") || input.contains("\u{6a13}\u{677f}") {
        let (w, d, t) = match numbers.len() {
            0 => (5000.0, 4000.0, 150.0),
            1 => (numbers[0], numbers[0], 150.0),
            2 => (numbers[0], numbers[1], 150.0),
            _ => (numbers[0], numbers[1], numbers[2]),
        };
        return Some(ParsedCommand {
            description: format!("\u{5efa}\u{7acb}\u{5730}\u{677f} {}x{}x{} mm", w, d, t),
            action: HybridAction::CreateFloor { width: w, depth: d, thickness: t, material: MaterialKind::Concrete },
        });
    }

    // Detect "柱" / "column" / "pillar"
    if input.contains('\u{67f1}') || input.contains("column") || input.contains("pillar") {
        let (r, h) = match numbers.len() {
            0 => (300.0, 3000.0),
            1 => (numbers[0], 3000.0),
            _ => (numbers[0], numbers[1]),
        };
        return Some(ParsedCommand {
            description: format!("\u{5efa}\u{7acb}\u{5713}\u{67f1} r={}mm h={}mm", r, h),
            action: HybridAction::CreateColumn { radius: r, height: h, material: MaterialKind::Concrete },
        });
    }

    // Detect "房間" / "room"
    if input.contains("\u{623f}\u{9593}") || input.contains("room") || input.contains('\u{623f}') {
        let (w, d, h) = match numbers.len() {
            0 => (4000.0, 5000.0, 2800.0),
            1 => (numbers[0], numbers[0], 2800.0),
            2 => (numbers[0], numbers[1], 2800.0),
            _ => (numbers[0], numbers[1], numbers[2]),
        };
        return Some(ParsedCommand {
            description: format!("\u{5efa}\u{7acb}\u{623f}\u{9593} {}x{}x{} mm", w, d, h),
            action: HybridAction::CreateRoom { width: w, depth: d, height: h },
        });
    }

    // Detect "方塊" / "box"
    if input.contains("\u{65b9}\u{584a}") || input.contains("box") || input.contains('\u{5efa}') {
        let (w, h, d) = match numbers.len() {
            0 => return None,
            1 => (numbers[0], numbers[0], numbers[0]),
            2 => (numbers[0], numbers[1], numbers[0]),
            _ => (numbers[0], numbers[1], numbers[2]),
        };
        return Some(ParsedCommand {
            description: format!("\u{5efa}\u{7acb}\u{65b9}\u{584a} {}x{}x{} mm", w, h, d),
            action: HybridAction::CreateWall { width: w, height: h, thickness: d, material: MaterialKind::White },
        });
    }

    None
}

/// Execute a parsed hybrid command on the scene
pub fn execute_command(scene: &mut Scene, cmd: &HybridAction) -> Vec<String> {
    let mut created_ids = Vec::new();

    match cmd {
        HybridAction::CreateWall { width, height, thickness, material } => {
            let id = scene.add_box("\u{7246}".into(), [0.0, 0.0, 0.0], *width, *height, *thickness, *material);
            created_ids.push(id);
        }
        HybridAction::CreateFloor { width, depth, thickness, material } => {
            let id = scene.add_box("\u{5730}\u{677f}".into(), [0.0, 0.0, 0.0], *width, *thickness, *depth, *material);
            created_ids.push(id);
        }
        HybridAction::CreateColumn { radius, height, material } => {
            let id = scene.add_cylinder("\u{67f1}".into(), [0.0, 0.0, 0.0], *radius, *height, 48, *material);
            created_ids.push(id);
        }
        HybridAction::CreateRoom { width, depth, height } => {
            let t = 200.0;
            let mat = MaterialKind::Concrete;
            created_ids.push(scene.add_box("\u{5730}\u{677f}".into(), [0.0, -100.0, 0.0], *width, 100.0, *depth, MaterialKind::Wood));
            created_ids.push(scene.add_box("\u{5317}\u{7246}".into(), [0.0, 0.0, 0.0], *width, *height, t, mat));
            created_ids.push(scene.add_box("\u{5357}\u{7246}".into(), [0.0, 0.0, *depth - t], *width, *height, t, mat));
            created_ids.push(scene.add_box("\u{897f}\u{7246}".into(), [0.0, 0.0, t], t, *height, *depth - 2.0 * t, mat));
            created_ids.push(scene.add_box("\u{6771}\u{7246}".into(), [*width - t, 0.0, t], t, *height, *depth - 2.0 * t, mat));
        }
        HybridAction::SetHeight { target: _, height: _ } => {
            // TODO: find object by name and set height
        }
        HybridAction::Unknown(_) => {}
    }

    created_ids
}

fn extract_numbers(input: &str) -> Vec<f32> {
    let mut nums = Vec::new();
    let mut current = String::new();
    let mut has_dot = false;
    let chars: Vec<char> = input.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if ch == '.' && !has_dot && !current.is_empty() {
            current.push(ch);
            has_dot = true;
        } else {
            if !current.is_empty() {
                if let Ok(n) = current.parse::<f32>() {
                    // Check if followed by 'm' (meters) — convert to mm
                    let next_ch = chars.get(i).copied();
                    let val = if next_ch == Some('m') && n < 100.0 {
                        n * 1000.0
                    } else {
                        n
                    };
                    nums.push(val);
                }
                current.clear();
                has_dot = false;
            }
        }
    }
    if !current.is_empty() {
        if let Ok(n) = current.parse::<f32>() {
            nums.push(n);
        }
    }

    nums
}
