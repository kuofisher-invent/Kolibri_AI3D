use super::types::*;
use super::entity_parsers::parse_single_entity;
use std::collections::HashMap;

pub(super) fn split_sections(content: &str) -> Vec<DxfSection> {
    let pairs = parse_group_code_pairs(content);
    let mut sections = Vec::new();
    let mut i = 0usize;

    while i + 1 < pairs.len() {
        if pairs[i].0 == 0 && pairs[i].1 == "SECTION" {
            if i + 3 < pairs.len() && pairs[i + 1].0 == 2 {
                let name = pairs[i + 1].1.clone();
                i += 2;

                let mut section_pairs = Vec::new();
                while i + 1 < pairs.len() {
                    if pairs[i].0 == 0 && pairs[i].1 == "ENDSEC" {
                        break;
                    }
                    section_pairs.push(pairs[i].clone());
                    i += 1;
                }

                sections.push(DxfSection {
                    name,
                    pairs: section_pairs,
                });
            }
        }
        i += 1;
    }

    sections
}

/// CHANGELOG: v0.1.0 - Parse raw group code pairs from DXF text.
pub(super) fn parse_group_code_pairs(content: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i + 1 < lines.len() {
        let code_str = lines[i].trim();
        let value_str = lines[i + 1].trim().to_string();

        if let Ok(code) = code_str.parse::<i32>() {
            out.push((code, value_str));
        }

        i += 2;
    }

    out
}

/// CHANGELOG: v0.1.0 - Parse LAYER table entries.
pub(super) fn parse_layers_from_tables(section: &DxfSection) -> ImportResult<Vec<LayerIr>> {
    let mut layers = Vec::new();
    let mut i = 0usize;

    while i < section.pairs.len() {
        if section.pairs[i].0 == 0 && section.pairs[i].1 == "LAYER" {
            let mut layer_name = String::new();
            let mut color = None;
            let mut is_visible = true;
            i += 1;

            while i < section.pairs.len() {
                let (code, value) = &section.pairs[i];
                if *code == 0 {
                    break;
                }

                match *code {
                    2 => layer_name = value.clone(),
                    62 => {
                        if let Ok(c) = value.parse::<i16>() {
                            is_visible = c >= 0;
                            color = Some(CadColor(c.abs()));
                        }
                    }
                    _ => {}
                }

                i += 1;
            }

            if !layer_name.is_empty() {
                layers.push(LayerIr {
                    name: layer_name,
                    color,
                    is_visible,
                });
            }
        } else {
            i += 1;
        }
    }

    Ok(layers)
}

/// CHANGELOG: v0.1.0 - Parse block definition section.
/// NOTE: Skeleton only; entity extraction inside blocks is intentionally shallow for v0.1.
pub(super) fn parse_block_definitions(section: &DxfSection) -> ImportResult<Vec<BlockDefinitionIr>> {
    let mut blocks = Vec::new();
    let mut i = 0usize;

    while i < section.pairs.len() {
        if section.pairs[i].0 == 0 && section.pairs[i].1 == "BLOCK" {
            let mut name = String::new();
            let mut base_point = [0.0, 0.0, 0.0];
            let mut raw_entities = Vec::new();
            i += 1;

            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 && section.pairs[i].1 == "ENDBLK" {
                    break;
                }

                match section.pairs[i].0 {
                    2 => name = section.pairs[i].1.clone(),
                    10 => base_point[0] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    20 => base_point[1] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    30 => base_point[2] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    0 => {
                        let start = i;
                        let entity_type = section.pairs[i].1.clone();
                        i += 1;
                        let mut codes = Vec::new();
                        let mut layer = String::new();

                        while i < section.pairs.len() {
                            if section.pairs[i].0 == 0 {
                                break;
                            }
                            if section.pairs[i].0 == 8 {
                                layer = section.pairs[i].1.clone();
                            }
                            codes.push(section.pairs[i].clone());
                            i += 1;
                        }

                        if start < i {
                            raw_entities.push(RawEntityIr {
                                entity_type,
                                layer,
                                group_codes: codes,
                            });
                            continue;
                        }
                    }
                    _ => {}
                }

                i += 1;
            }

            if !name.is_empty() {
                blocks.push(BlockDefinitionIr {
                    name,
                    base_point,
                    entities: raw_entities,
                });
            }
        } else {
            i += 1;
        }
    }

    Ok(blocks)
}

/// CHANGELOG: v0.2.0 - Parse entities section into normalized IR.
/// Handles old-style POLYLINE+VERTEX+SEQEND sequences, entity counting,
/// and block INSERT explosion.
pub(super) fn parse_entities(
    section: &DxfSection,
    config: &DxfImportConfig,
    ir: &mut GeometryIr,
) -> ImportResult<()> {
    let mut i = 0usize;
    let mut entity_counts: HashMap<String, usize> = HashMap::new();

    while i < section.pairs.len() {
        if section.pairs[i].0 != 0 {
            i += 1;
            continue;
        }

        let entity_type = section.pairs[i].1.clone();
        *entity_counts.entry(entity_type.clone()).or_insert(0) += 1;
        i += 1;

        // Special handling for old-style POLYLINE (VERTEX/SEQEND sequence)
        if entity_type == "POLYLINE" {
            let mut payload = Vec::new();
            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 {
                    break;
                }
                payload.push(section.pairs[i].clone());
                i += 1;
            }

            let mut poly_layer = String::from("0");
            let mut is_closed = false;
            let mut color = None;

            for (code, value) in &payload {
                match *code {
                    8 => poly_layer = value.clone(),
                    62 => color = value.parse::<i16>().ok().map(CadColor),
                    70 => is_closed = (value.parse::<i32>().unwrap_or(0) & 1) != 0,
                    _ => {}
                }
            }

            // Collect subsequent VERTEX entities until SEQEND
            let mut points = Vec::new();
            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 {
                    let next_type = &section.pairs[i].1;
                    if next_type == "SEQEND" {
                        // Skip past SEQEND and its payload
                        i += 1;
                        while i < section.pairs.len() && section.pairs[i].0 != 0 {
                            i += 1;
                        }
                        break;
                    }
                    if next_type == "VERTEX" {
                        i += 1;
                        let mut vx = 0.0f32;
                        let mut vy = 0.0f32;
                        let mut vz = 0.0f32;
                        while i < section.pairs.len() && section.pairs[i].0 != 0 {
                            match section.pairs[i].0 {
                                10 => vx = section.pairs[i].1.parse().unwrap_or(0.0),
                                20 => vy = section.pairs[i].1.parse().unwrap_or(0.0),
                                30 => vz = section.pairs[i].1.parse().unwrap_or(0.0),
                                _ => {}
                            }
                            i += 1;
                        }
                        points.push([vx, vy, vz]);
                        continue;
                    }
                    // Unknown entity inside POLYLINE sequence — break out
                    break;
                }
                i += 1;
            }

            ir.curves.push(CurveIr::Polyline(PolylineIr {
                layer: poly_layer,
                points,
                is_closed,
                color,
            }));
            continue;
        }

        let mut payload = Vec::new();
        while i < section.pairs.len() {
            if section.pairs[i].0 == 0 {
                break;
            }
            payload.push(section.pairs[i].clone());
            i += 1;
        }

        let parsed = parse_single_entity(&entity_type, &payload)?;
        match parsed {
            ParsedEntity::Line(v) => ir.curves.push(CurveIr::Line(v)),
            ParsedEntity::Polyline(v) => ir.curves.push(CurveIr::Polyline(v)),
            ParsedEntity::Circle(v) => ir.curves.push(CurveIr::Circle(v)),
            ParsedEntity::Arc(v) => ir.curves.push(CurveIr::Arc(v)),
            ParsedEntity::Text(v) => ir.texts.push(v),
            ParsedEntity::Dimension(v) => ir.dimensions.push(v),
            ParsedEntity::Insert(v) => {
                if config.explode_inserts {
                    // Explode: find block definition and add its entities as raw
                    if let Some(block) = ir.blocks.iter().find(|b| b.name == v.block_name) {
                        let block_entities = block.entities.clone();
                        for raw_entity in block_entities {
                            ir.raw_entities.push(raw_entity);
                        }
                    }
                }
                ir.inserts.push(v);
            }
            ParsedEntity::Unsupported(v) => {
                if config.preserve_raw_entities {
                    ir.raw_entities.push(v);
                }
            }
        }
    }

    // Store entity counts in metadata
    let mut counts_report = Vec::new();
    let mut sorted_counts: Vec<_> = entity_counts.iter().collect();
    sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
    for (etype, count) in &sorted_counts {
        counts_report.push(format!("{}:{}", etype, count));
    }
    ir.metadata.insert("entity_counts".to_string(), counts_report.join(","));

    // Log entity counts
    eprintln!("[DXF] Entity counts:");
    for (etype, count) in &sorted_counts {
        eprintln!("[DXF]   {}: {}", etype, count);
    }
    eprintln!("[DXF] Total parsed: curves={}, texts={}, dims={}, inserts={}, raw={}",
        ir.curves.len(), ir.texts.len(), ir.dimensions.len(),
        ir.inserts.len(), ir.raw_entities.len());

    Ok(())
}

/// NOTE: DXF INSUNITS handling is intentionally basic in v0.1.
pub(super) fn detect_units(content: &str) -> Option<Unit> {
    let pairs = parse_group_code_pairs(content);

    for win in pairs.windows(2) {
        if win[0].0 == 9 && win[0].1 == "$INSUNITS" && win[1].0 == 70 {
            return match win[1].1.parse::<i32>().ok() {
                Some(1) => Some(Unit::Inch),
                Some(2) => Some(Unit::Foot),
                Some(4) => Some(Unit::Millimeter),
                Some(5) => Some(Unit::Centimeter),
                Some(6) => Some(Unit::Meter),
                _ => Some(Unit::Unknown),
            };
        }
    }

    None
}

