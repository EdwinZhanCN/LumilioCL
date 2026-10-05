use crate::resource_pack::{ResourcePack, TextureData};

/// Font glyph data loaded from `font/ascii.png`.
struct FontData {
    /// RGBA pixel data of the 128x128 (16x16 grid of 8x8 glyphs) font texture.
    pixels: Vec<u8>,
    /// Font texture width.
    width: u32,
    /// Font texture height.
    height: u32,
    /// Per-character glyph widths (rightmost non-transparent pixel + 1).
    glyph_widths: [u8; 256],
    /// Cell width (pixels per glyph cell).
    cell_w: u32,
    /// Cell height (pixels per glyph cell).
    cell_h: u32,
}

/// A text segment with its own color.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextSegment {
    pub text: String,
    pub color: String,
}

/// Minecraft text color names → RGB values.
fn text_color_rgb(color: &str) -> [u8; 3] {
    match color {
        "black" => [0, 0, 0],
        "dark_blue" => [0, 0, 170],
        "dark_green" => [0, 170, 0],
        "dark_aqua" => [0, 170, 170],
        "dark_red" => [170, 0, 0],
        "dark_purple" => [170, 0, 170],
        "gold" => [255, 170, 0],
        "gray" => [170, 170, 170],
        "dark_gray" => [85, 85, 85],
        "blue" => [85, 85, 255],
        "green" => [85, 255, 85],
        "aqua" => [85, 255, 255],
        "red" => [255, 85, 85],
        "light_purple" => [255, 85, 255],
        "yellow" => [255, 255, 85],
        "white" => [255, 255, 255],
        _ => [0, 0, 0],
    }
}

/// Compute outline color: text color × 0.25 (darkened).
fn outline_color(rgb: &[u8; 3]) -> [u8; 3] {
    [
        (rgb[0] as f32 * 0.25) as u8,
        (rgb[1] as f32 * 0.25) as u8,
        (rgb[2] as f32 * 0.25) as u8,
    ]
}

/// Parse a Minecraft JSON text component into a list of segments.
///
/// Supports:
/// - Plain string → single segment with default color
/// - JSON string `"hello"` → single segment with default color
/// - JSON object `{"text":"hi","color":"red"}` → segment, recurse into `"extra"` array
/// - JSON array `[{"text":"a","color":"red"},{"text":"b"}]` → parse each element
///
/// Falls back to treating input as plain string on parse failure.
pub(crate) fn parse_text_component(input: &str, default_color: &str) -> Vec<TextSegment> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    // Try parsing as JSON
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let mut segments = Vec::new();
        parse_json_value(&value, default_color, &mut segments);
        return segments;
    }

    // Fallback: treat as plain text
    vec![TextSegment {
        text: input.to_string(),
        color: default_color.to_string(),
    }]
}

fn parse_json_value(value: &serde_json::Value, parent_color: &str, segments: &mut Vec<TextSegment>) {
    match value {
        serde_json::Value::String(s) => {
            if !s.is_empty() {
                segments.push(TextSegment {
                    text: s.clone(),
                    color: parent_color.to_string(),
                });
            }
        }
        serde_json::Value::Object(obj) => {
            let color = obj.get("color")
                .and_then(|v| v.as_str())
                .unwrap_or(parent_color);
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    segments.push(TextSegment {
                        text: text.to_string(),
                        color: color.to_string(),
                    });
                }
            }
            if let Some(extra) = obj.get("extra").and_then(|v| v.as_array()) {
                for item in extra {
                    parse_json_value(item, color, segments);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                parse_json_value(item, parent_color, segments);
            }
        }
        _ => {}
    }
}

/// Calculate the total pixel width of segments using font glyph widths.
fn segments_width(segments: &[TextSegment], font: &FontData, glyph_scale: u32) -> u32 {
    segments.iter().map(|seg| {
        seg.text.bytes()
            .map(|ch| font.glyph_widths[ch as usize] as u32 * glyph_scale)
            .sum::<u32>()
    }).sum()
}

/// Load font glyph data from the resource pack.
fn load_font(rp: &ResourcePack) -> Option<FontData> {
    let tex = rp.get_texture("font/ascii")?;
    let pixels = tex.pixels.clone();
    let width = tex.width;
    let height = tex.height;

    // Grid is 16 columns × 16 rows
    let cell_w = width / 16;
    let cell_h = height / 16;

    // Scan each glyph cell for the rightmost non-transparent pixel
    let mut glyph_widths = [0u8; 256];
    for ch in 0..256u16 {
        let col = (ch % 16) as u32;
        let row = (ch / 16) as u32;
        let cx = col * cell_w;
        let cy = row * cell_h;

        let mut max_x = 0u32;
        let mut has_pixels = false;
        for py in 0..cell_h {
            for px in 0..cell_w {
                let idx = ((cy + py) * width + (cx + px)) as usize * 4;
                if idx + 3 < pixels.len() && pixels[idx + 3] > 0 {
                    max_x = max_x.max(px + 1);
                    has_pixels = true;
                }
            }
        }

        glyph_widths[ch as usize] = if has_pixels {
            (max_x + 1).min(cell_w) as u8 // +1 for spacing
        } else {
            (cell_w / 2) as u8 // space character width
        };
    }

    Some(FontData { pixels, width, height, glyph_widths, cell_w, cell_h })
}

/// Sign kind, determines which UV region in the base texture holds the visible
/// board face (and therefore where text should be written).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignKind {
    /// Standing sign — 24x12x2 board at tex_offs(0,0). Text on North face.
    Standing,
    /// Wall sign — same board, but pushed against the wall so the visible face
    /// is the board's South face in the UV layout.
    Wall,
    /// Hanging sign — 14x10x2 board at tex_offs(0,12). Different UV rows.
    Hanging,
}

/// Composite sign text onto the sign board texture.
///
/// Returns a 4x upscaled sign texture with text rendered onto the appropriate
/// board face region(s) for the given `kind`. For wall and hanging signs this
/// also writes the text onto the mirrored face so it's visible from either
/// side (MC draws the text as a separate billboard; we fake it on both faces).
pub(crate) fn composite_sign_with_text(
    rp: &ResourcePack,
    base_texture_path: &str,
    lines: &[&str],
    color: &str,
    glowing: bool,
    kind: SignKind,
) -> Option<TextureData> {
    let base_tex = rp.get_texture(base_texture_path)?;
    let font = load_font(rp)?;

    let scale = 4u32;
    let out_w = base_tex.width * scale;
    let out_h = base_tex.height * scale;
    let mut pixels = vec![0u8; (out_w * out_h * 4) as usize];

    // Upscale the base texture using nearest-neighbor
    for y in 0..out_h {
        for x in 0..out_w {
            let src_x = x / scale;
            let src_y = y / scale;
            let src_idx = (src_y * base_tex.width + src_x) as usize * 4;
            let dst_idx = (y * out_w + x) as usize * 4;
            if src_idx + 3 < base_tex.pixels.len() {
                pixels[dst_idx..dst_idx + 4].copy_from_slice(&base_tex.pixels[src_idx..src_idx + 4]);
            }
        }
    }

    // Text regions (x0, y0, width, height) in base-texture pixels. One region
    // per face we want to cover; wall/hanging signs get two so either side is
    // readable. Values map from the cube-face UV layout in `cube_face_uvs`.
    //
    // Standing sign board: tex_offs(0,0), dim(24, 12, 2) → N=(2,2,24,12), S=(28,2,24,12)
    // Hanging sign board:  tex_offs(0,12), dim(14,10, 2) → N=(2,14,14,10), S=(18,14,14,10)
    let regions: &[(u32, u32, u32, u32)] = match kind {
        SignKind::Standing => &[(2, 2, 24, 12)],
        SignKind::Wall     => &[(2, 2, 24, 12), (28, 2, 24, 12)],
        SignKind::Hanging  => &[(2, 14, 14, 10), (18, 14, 14, 10)],
    };

    // Count non-empty lines so we can vertically center the text block — MC
    // centers text on the sign regardless of how many of the 4 slots are used.
    let non_empty = lines.iter().take(4).filter(|l| !l.is_empty()).count().max(1) as u32;

    for &(rx, ry, rw, rh) in regions {
        let text_x0 = rx * scale;
        let text_y0 = ry * scale;
        let text_w = rw * scale;
        let text_h = rh * scale;

        let line_height = text_h / 4;
        let glyph_scale = line_height.min(font.cell_h * scale / 2) / font.cell_h;

        // Vertical offset so the `non_empty` lines are centered in the region.
        let v_offset = (4u32.saturating_sub(non_empty)) * line_height / 2;

        // Enumerate only non-empty lines so they stack at the top of the block;
        // then v_offset centers the block in the region.
        let mut visible_idx = 0u32;
        for &line in lines.iter().take(4) {
            if line.is_empty() {
                continue;
            }

            let y_start = text_y0 + v_offset + visible_idx * line_height;
            visible_idx += 1;

            let segments = parse_text_component(line, color);
            if segments.is_empty() {
                continue;
            }

            let total_width = segments_width(&segments, &font, glyph_scale);

            let x_offset = if total_width < text_w {
                text_x0 + (text_w - total_width) / 2
            } else {
                text_x0
            };

            render_segments(
                &mut pixels, out_w,
                &font, glyph_scale,
                &segments,
                x_offset, y_start,
                text_x0 + text_w,
                glowing,
            );
        }
    }

    Some(TextureData {
        width: out_w,
        height: out_h,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Render a line of text segments into the pixel buffer.
fn render_segments(
    pixels: &mut [u8],
    buf_w: u32,
    font: &FontData,
    glyph_scale: u32,
    segments: &[TextSegment],
    x_start: u32,
    y_start: u32,
    x_max: u32,
    glowing: bool,
) {
    let mut cursor_x = x_start;

    for seg in segments {
        let rgb = text_color_rgb(&seg.color);

        if glowing {
            // Glow outline: render 8 directional offsets in darkened color first
            let outline_rgb = outline_color(&rgb);
            let offsets: [(i32, i32); 8] = [
                (-1, 0), (1, 0), (0, -1), (0, 1),
                (-1, -1), (-1, 1), (1, -1), (1, 1),
            ];
            // We need to compute cursor positions for outline pass without advancing cursor_x
            let mut outline_cursor = cursor_x;
            for ch in seg.text.bytes() {
                let glyph_w = font.glyph_widths[ch as usize] as u32 * glyph_scale;
                if outline_cursor + glyph_w > x_max {
                    break;
                }
                for &(dx, dy) in &offsets {
                    let ox = outline_cursor as i32 + dx;
                    let oy = y_start as i32 + dy;
                    if ox >= 0 && oy >= 0 {
                        blit_glyph(
                            pixels, buf_w, font, glyph_scale,
                            ch, &outline_rgb,
                            ox as u32, oy as u32,
                            x_max,
                        );
                    }
                }
                outline_cursor += glyph_w;
            }
        }

        // Render main glyphs on top
        for ch in seg.text.bytes() {
            let glyph_w = font.glyph_widths[ch as usize] as u32 * glyph_scale;
            if cursor_x + glyph_w > x_max {
                break;
            }
            blit_glyph(
                pixels, buf_w, font, glyph_scale,
                ch, &rgb,
                cursor_x, y_start,
                x_max,
            );
            cursor_x += glyph_w;
        }
    }
}

/// Blit a single glyph at the given position with the given color.
fn blit_glyph(
    pixels: &mut [u8],
    buf_w: u32,
    font: &FontData,
    glyph_scale: u32,
    ch: u8,
    rgb: &[u8; 3],
    dst_x: u32,
    dst_y: u32,
    x_max: u32,
) {
    let scaled_h = font.cell_h * glyph_scale;
    let glyph_w = font.glyph_widths[ch as usize] as u32 * glyph_scale;

    let col = (ch as u32) % 16;
    let row = (ch as u32) / 16;
    let gx = col * font.cell_w;
    let gy = row * font.cell_h;

    for py in 0..scaled_h {
        let src_y = py / glyph_scale;
        for px in 0..glyph_w {
            let src_x = px / glyph_scale;
            let font_idx = ((gy + src_y) * font.width + (gx + src_x)) as usize * 4;
            if font_idx + 3 >= font.pixels.len() {
                continue;
            }
            let alpha = font.pixels[font_idx + 3];
            if alpha == 0 {
                continue;
            }

            let px_x = dst_x + px;
            let px_y = dst_y + py;
            if px_x >= x_max {
                break;
            }
            let dst_idx = (px_y * buf_w + px_x) as usize * 4;
            if dst_idx + 3 >= pixels.len() {
                continue;
            }

            // Alpha-blend the glyph color onto the background
            let a = alpha as f32 / 255.0;
            let inv_a = 1.0 - a;
            pixels[dst_idx] = (rgb[0] as f32 * a + pixels[dst_idx] as f32 * inv_a) as u8;
            pixels[dst_idx + 1] = (rgb[1] as f32 * a + pixels[dst_idx + 1] as f32 * inv_a) as u8;
            pixels[dst_idx + 2] = (rgb[2] as f32 * a + pixels[dst_idx + 2] as f32 * inv_a) as u8;
            pixels[dst_idx + 3] = alpha.max(pixels[dst_idx + 3]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_string() {
        let segments = parse_text_component("Hello", "black");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].color, "black");
    }

    #[test]
    fn test_parse_json_string() {
        let segments = parse_text_component(r#""Hello""#, "black");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].color, "black");
    }

    #[test]
    fn test_parse_json_object() {
        let segments = parse_text_component(r#"{"text":"Hello","color":"red"}"#, "black");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].color, "red");
    }

    #[test]
    fn test_parse_json_array() {
        let segments = parse_text_component(
            r#"[{"text":"Hello","color":"red"},{"text":" World","color":"blue"}]"#,
            "black",
        );
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].color, "red");
        assert_eq!(segments[1].text, " World");
        assert_eq!(segments[1].color, "blue");
    }

    #[test]
    fn test_parse_nested_extra() {
        let segments = parse_text_component(
            r#"{"text":"A","color":"red","extra":[{"text":"B","color":"green"},{"text":"C"}]}"#,
            "black",
        );
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], TextSegment { text: "A".into(), color: "red".into() });
        assert_eq!(segments[1], TextSegment { text: "B".into(), color: "green".into() });
        // C inherits parent color "red"
        assert_eq!(segments[2], TextSegment { text: "C".into(), color: "red".into() });
    }

    #[test]
    fn test_parse_color_inheritance() {
        let segments = parse_text_component(
            r#"[{"text":"A","color":"gold"},{"text":"B"}]"#,
            "white",
        );
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].color, "gold");
        // B inherits parent_color from array context = "white"
        assert_eq!(segments[1].color, "white");
    }

    #[test]
    fn test_parse_invalid_json_fallback() {
        let segments = parse_text_component("Not {valid json", "dark_blue");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Not {valid json");
        assert_eq!(segments[0].color, "dark_blue");
    }

    #[test]
    fn test_outline_color_computation() {
        let rgb = [255, 170, 0]; // gold
        let outline = outline_color(&rgb);
        assert_eq!(outline[0], 63); // 255 * 0.25
        assert_eq!(outline[1], 42); // 170 * 0.25
        assert_eq!(outline[2], 0);  // 0 * 0.25
    }

    #[test]
    fn test_parse_empty_text() {
        let segments = parse_text_component("", "black");
        assert!(segments.is_empty());

        let segments = parse_text_component(r#"{"text":"","color":"red"}"#, "black");
        assert!(segments.is_empty());
    }
}
