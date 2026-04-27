// History heatmap image generation

use crate::images::{escape_svg, render_svg_to_png};

// ============================================================================
// Overview heatmap (all users × all weeks)
// ============================================================================

/// One row in the history heatmap (one user across all weeks).
pub struct HistoryRow {
    pub name: String,
    /// One entry per week column. None = user wasn't in tracker that week.
    /// Tuple: (count, goal_met, loa_exempt, type_counts)
    pub weeks: Vec<Option<(i32, bool, bool, Vec<(String, i32)>)>>,
}

/// Render a user×week heatmap grid as PNG.
/// Shows count + ✓/✗ per cell. No per-cell type breakdown.
pub fn generate_history_image(
    rows: &[HistoryRow],
    week_labels: &[String],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    const NAME_COL_W: u32 = 130;
    const CELL_W: u32 = 60;
    const CELL_H: u32 = 36;
    const HEADER_H: u32 = 55;
    const COL_LABEL_H: u32 = 28;
    const PADDING: u32 = 12;
    const LABEL_FONT: u32 = 11;
    const COUNT_FONT: u32 = 13;
    const NAME_FONT: u32 = 13;

    let n_weeks = week_labels.len() as u32;
    let n_rows = rows.len() as u32;

    let image_w = PADDING * 2 + NAME_COL_W + n_weeks * CELL_W;
    let image_h = PADDING + HEADER_H + n_rows * CELL_H + PADDING;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"##,
        image_w, image_h
    ));
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#2f3136"/>"##,
        image_w, image_h
    ));

    // Title
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="16" font-weight="bold" fill="#ffffff">Gym History</text>"##,
        PADDING, PADDING + 20
    ));

    // Column headers
    let col_label_y = PADDING + HEADER_H - COL_LABEL_H;
    for (wi, label) in week_labels.iter().enumerate() {
        let cx = PADDING + NAME_COL_W + wi as u32 * CELL_W + CELL_W / 2;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
            PADDING + NAME_COL_W + wi as u32 * CELL_W, col_label_y, CELL_W, COL_LABEL_H,
        ));
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="{}" fill="#b9bbbe" text-anchor="middle">{}</text>"##,
            cx, col_label_y + 19, LABEL_FONT, escape_svg(label)
        ));
    }

    // Rows
    for (ri, row) in rows.iter().enumerate() {
        let row_y = PADDING + HEADER_H + ri as u32 * CELL_H;
        let row_bg = if ri % 2 == 0 { "#2f3136" } else { "#36393f" };

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
            PADDING, row_y, image_w - PADDING * 2, CELL_H, row_bg
        ));

        // Name
        let display_name = truncate_name(&row.name, 14);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="{}" fill="#ffffff">{}</text>"##,
            PADDING + 4,
            row_y + CELL_H / 2 + NAME_FONT / 2,
            NAME_FONT,
            escape_svg(&display_name)
        ));

        // Week cells
        for (wi, week_data) in row.weeks.iter().enumerate() {
            let cell_x = PADDING + NAME_COL_W + wi as u32 * CELL_W;
            let cell_y = row_y;

            match week_data {
                None => {
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#1e2124" rx="3"/>"##,
                        cell_x + 2, cell_y + 3, CELL_W - 4, CELL_H - 6
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="{}" fill="#4f545c" text-anchor="middle">—</text>"##,
                        cell_x + CELL_W / 2,
                        cell_y + CELL_H / 2 + COUNT_FONT / 2,
                        COUNT_FONT
                    ));
                }
                Some((count, goal_met, loa_exempt, _)) => {
                    if *loa_exempt {
                        // LOA week — muted blue, "N LOA" label
                        svg.push_str(&format!(
                            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#1a2d3d" rx="3"/>"##,
                            cell_x + 2, cell_y + 3, CELL_W - 4, CELL_H - 6
                        ));
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="{}" fill="#5b8fbd" text-anchor="middle">{} LOA</text>"##,
                            cell_x + CELL_W / 2,
                            cell_y + CELL_H / 2 + COUNT_FONT / 2,
                            COUNT_FONT,
                            count,
                        ));
                    } else {
                        let (cell_fill, count_color) = if *goal_met {
                            ("#1e4a2e", "#43b581")
                        } else {
                            ("#4a1e1e", "#f04747")
                        };
                        svg.push_str(&format!(
                            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="3"/>"##,
                            cell_x + 2, cell_y + 3, CELL_W - 4, CELL_H - 6, cell_fill
                        ));
                        let sym = if *goal_met { "✓" } else { "✗" };
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="{}" font-weight="bold" fill="{}" text-anchor="middle">{}{}</text>"##,
                            cell_x + CELL_W / 2,
                            cell_y + CELL_H / 2 + COUNT_FONT / 2,
                            COUNT_FONT,
                            count_color,
                            count,
                            sym
                        ));
                    }
                }
            }
        }
    }

    svg.push_str("</svg>");
    render_svg_to_png(&svg, image_w, image_h)
}

// ============================================================================
// Single-user history table image
// ============================================================================

/// One entry in the single-user history: either a completed week or a goal change event.
pub enum UserHistoryEntry {
    Week {
        week_label: String,  // e.g. "Apr 19 – Apr 26"
        /// (count, goal_met, loa_exempt, type_counts). None = not tracked that week.
        result: Option<(i32, bool, bool, Vec<(String, i32)>)>,
    },
    GoalChange {
        description: String,  // e.g. "total goal → 5/week"
    },
}

/// Render a per-user history table as PNG.
/// Rows alternate between week entries and inline goal-change events.
pub fn generate_user_history_image(
    display_name: &str,
    season_name: &str,
    goal_summary: &str,
    entries: &[UserHistoryEntry],
    total_count: i32,
    goals_met: i32,
    goals_missed: i32,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    const IMG_W: u32 = 620;
    const PADDING: u32 = 14;
    const HEADER_H: u32 = 64; // title (24) + goal line (20) + spacing (20)
    const COL_HDR_H: u32 = 26;
    const ROW_H: u32 = 34;
    const GOAL_ROW_H: u32 = 26;
    const FOOTER_H: u32 = 36;

    // Column layout: week | result | types
    const WEEK_COL_W: u32 = 170;
    const RESULT_COL_W: u32 = 76;
    // types column fills the rest
    let types_col_x = PADDING + WEEK_COL_W + RESULT_COL_W;
    let _types_col_w = IMG_W - PADDING - types_col_x;

    // Pre-compute image height
    let content_h: u32 = entries.iter().map(|e| match e {
        UserHistoryEntry::Week { .. } => ROW_H,
        UserHistoryEntry::GoalChange { .. } => GOAL_ROW_H,
    }).sum();
    let image_h = PADDING + HEADER_H + COL_HDR_H + content_h + FOOTER_H + PADDING;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"##,
        IMG_W, image_h
    ));
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#2f3136"/>"##,
        IMG_W, image_h
    ));

    // Title
    let title = format!("{} — {} History", display_name, season_name);
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="17" font-weight="bold" fill="#ffffff">{}</text>"##,
        PADDING, PADDING + 26, escape_svg(&title)
    ));

    // Current goal line
    if !goal_summary.is_empty() {
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="#8e9297">{}</text>"##,
            PADDING, PADDING + 46, escape_svg(goal_summary)
        ));
    }

    // Column headers background
    let col_hdr_y = PADDING + HEADER_H;
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
        PADDING, col_hdr_y, IMG_W - PADDING * 2, COL_HDR_H
    ));
    let hdr_text_y = col_hdr_y + 18;
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" fill="#8e9297">WEEK</text>"##,
        PADDING + 6, hdr_text_y
    ));
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" fill="#8e9297" text-anchor="middle">RESULT</text>"##,
        PADDING + WEEK_COL_W + RESULT_COL_W / 2, hdr_text_y
    ));
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" fill="#8e9297">TYPES</text>"##,
        types_col_x + 6, hdr_text_y
    ));

    // Rows
    let mut y = col_hdr_y + COL_HDR_H;
    let mut row_idx = 0u32;
    for entry in entries {
        match entry {
            UserHistoryEntry::Week { week_label, result } => {
                let row_bg = if row_idx % 2 == 0 { "#2f3136" } else { "#36393f" };
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
                    PADDING, y, IMG_W - PADDING * 2, ROW_H, row_bg
                ));

                let text_y = y + ROW_H / 2 + 5;

                // Week label
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" fill="#dcddde">{}</text>"##,
                    PADDING + 6, text_y, escape_svg(week_label)
                ));

                // Result cell
                let result_x = PADDING + WEEK_COL_W;
                match result {
                    None => {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="#4f545c" text-anchor="middle">—</text>"##,
                            result_x + RESULT_COL_W / 2, text_y
                        ));
                    }
                    Some((count, _, true, _)) => {
                        // LOA-exempt week
                        svg.push_str(&format!(
                            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#1a2d3d" rx="3"/>"##,
                            result_x + 4, y + 5, RESULT_COL_W - 8, ROW_H - 10
                        ));
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" font-weight="bold" fill="#5b8fbd" text-anchor="middle">{} LOA</text>"##,
                            result_x + RESULT_COL_W / 2, text_y, count
                        ));
                    }
                    Some((count, goal_met, false, _)) => {
                        let (bg, fg) = if *goal_met { ("#1e4a2e", "#43b581") } else { ("#4a1e1e", "#f04747") };
                        svg.push_str(&format!(
                            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="3"/>"##,
                            result_x + 4, y + 5, RESULT_COL_W - 8, ROW_H - 10, bg
                        ));
                        let sym = if *goal_met { "✓" } else { "✗" };
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" font-weight="bold" fill="{}" text-anchor="middle">{} {}</text>"##,
                            result_x + RESULT_COL_W / 2, text_y, fg, count, sym
                        ));
                    }
                }

                // Types
                let types_str = match result {
                    None => "not tracked".to_string(),
                    Some((_, _, _, type_counts)) if type_counts.is_empty() => "no types logged".to_string(),
                    Some((_, _, _, type_counts)) => type_counts.iter()
                        .map(|(t, c)| format!("{}({})", t, c))
                        .collect::<Vec<_>>()
                        .join("  "),
                };
                let types_fill = match result {
                    None => "#4f545c",
                    Some((_, _, true, _)) => "#5b8fbd",  // LOA: blue tint
                    Some((_, true, _, _)) => "#72767d",
                    Some((_, false, _, types)) if types.is_empty() => "#4f545c",
                    _ => "#72767d",
                };
                let truncated_types = truncate_str(&types_str, 52);
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="12" fill="{}">{}</text>"##,
                    types_col_x + 6, text_y, types_fill, escape_svg(&truncated_types)
                ));

                row_idx += 1;
                y += ROW_H;
            }
            UserHistoryEntry::GoalChange { description } => {
                // Full-width goal change banner
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#2a2d31"/>"##,
                    PADDING, y, IMG_W - PADDING * 2, GOAL_ROW_H
                ));
                // Left accent bar
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="3" height="{}" fill="#5865f2"/>"##,
                    PADDING, y, GOAL_ROW_H
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="11" font-style="italic" fill="#8e9297">⚙ goal change: {}</text>"##,
                    PADDING + 10, y + 17, escape_svg(description)
                ));
                y += GOAL_ROW_H;
            }
        }
    }

    // Footer
    let rate = if goals_met + goals_missed > 0 {
        goals_met * 100 / (goals_met + goals_missed)
    } else { 0 };
    let footer = format!(
        "Total: {} workouts   ✓ {} met   ✗ {} missed   Rate: {}%",
        total_count, goals_met, goals_missed, rate
    );
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
        PADDING, y, IMG_W - PADDING * 2, FOOTER_H
    ));
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="DejaVu Sans" font-size="13" font-weight="bold" fill="#dcddde">{}</text>"##,
        PADDING + 6, y + 23, escape_svg(&footer)
    ));

    svg.push_str("</svg>");
    render_svg_to_png(&svg, IMG_W, image_h)
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}…", chars[..max_chars - 1].iter().collect::<String>())
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", chars[..max_chars - 1].iter().collect::<String>())
    }
}
