use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_line_dxf() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
LINE
8
GRID
10
0.0
20
0.0
11
1000.0
21
0.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Line(line) => {
                assert_eq!(line.layer, "GRID");
                assert_eq!(line.start, [0.0, 0.0, 0.0]);
                assert_eq!(line.end, [1000.0, 0.0, 0.0]);
            }
            _ => panic!("Expected line"),
        }
    }

    #[test]
    fn parse_old_style_polyline_with_vertices() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
POLYLINE
8
WALLS
70
1
0
VERTEX
10
0.0
20
0.0
30
0.0
0
VERTEX
10
100.0
20
0.0
30
0.0
0
VERTEX
10
100.0
20
50.0
30
0.0
0
SEQEND
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "WALLS");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 3);
                assert_eq!(p.points[0], [0.0, 0.0, 0.0]);
                assert_eq!(p.points[1], [100.0, 0.0, 0.0]);
                assert_eq!(p.points[2], [100.0, 50.0, 0.0]);
            }
            _ => panic!("Expected polyline"),
        }
    }

    #[test]
    fn parse_spline_as_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
SPLINE
8
CURVES
70
0
10
0.0
20
0.0
10
50.0
20
100.0
10
100.0
20
0.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "CURVES");
                assert_eq!(p.points.len(), 3);
            }
            _ => panic!("Expected polyline from spline"),
        }
    }

    #[test]
    fn parse_ellipse_as_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
ELLIPSE
8
SHAPES
10
500.0
20
500.0
30
0.0
11
200.0
21
0.0
31
0.0
40
0.5
41
0.0
42
6.283185
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "SHAPES");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 33); // 32 segments + 1
            }
            _ => panic!("Expected polyline from ellipse"),
        }
    }

    #[test]
    fn parse_solid_as_closed_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
SOLID
8
FILL
10
0.0
20
0.0
11
100.0
21
0.0
12
0.0
22
100.0
13
100.0
23
100.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "FILL");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 4);
            }
            _ => panic!("Expected closed polyline from SOLID"),
        }
    }

    #[test]
    fn parse_dimension_with_measured_value() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
DIMENSION
8
DIM
13
0.0
23
0.0
14
5000.0
24
0.0
42
5000.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.dimensions.len(), 1);
        assert_eq!(ir.dimensions[0].value_text, Some("5000".to_string()));
        assert!(ir.dimensions[0].definition_points.len() >= 2);
    }

    #[test]
    fn parse_mtext_continuation() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
MTEXT
8
NOTES
10
100.0
20
200.0
40
10.0
3
Hello
3
World
1
Final
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.texts.len(), 1);
        assert_eq!(ir.texts[0].value, "HelloWorldFinal");
        assert_eq!(ir.texts[0].layer, "NOTES");
    }

    #[test]
    fn parse_multi_entity_dxf() {
        let dxf = "\
0
SECTION
2
TABLES
0
TABLE
2
LAYER
0
LAYER
2
WALLS
62
1
0
LAYER
2
GRID
62
3
0
ENDTAB
0
ENDSEC
0
SECTION
2
ENTITIES
0
LINE
8
WALLS
10
0.0
20
0.0
30
0.0
11
5000.0
21
0.0
31
0.0
0
LINE
8
WALLS
10
5000.0
20
0.0
11
5000.0
21
3000.0
0
ARC
8
GRID
10
2500.0
20
1500.0
40
500.0
50
0.0
51
180.0
0
CIRCLE
8
GRID
10
2500.0
20
1500.0
40
300.0
0
TEXT
8
GRID
10
100.0
20
100.0
40
200.0
1
A
0
DIMENSION
8
DIM
13
0.0
23
0.0
14
5000.0
24
0.0
1
5000
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        // 2 lines + 1 arc + 1 circle = 4 curves
        assert_eq!(ir.curves.len(), 4);
        assert_eq!(ir.texts.len(), 1);
        assert_eq!(ir.dimensions.len(), 1);
        assert_eq!(ir.layers.len(), 2);

        // Entity counts should be in metadata
        assert!(ir.metadata.contains_key("entity_counts"));
    }

    #[test]
    fn entity_counts_in_metadata() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
LINE
8
L
10
0.0
20
0.0
11
1.0
21
1.0
0
LINE
8
L
10
1.0
20
1.0
11
2.0
21
2.0
0
CIRCLE
8
L
10
0.0
20
0.0
40
5.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        let counts = ir.metadata.get("entity_counts").expect("should have counts");
        assert!(counts.contains("LINE:2"));
        assert!(counts.contains("CIRCLE:1"));
    }
}
