// Weekly summary image generation

use crate::images::{escape_svg, render_svg_to_png};

/// Data for a single user's weekly summary
pub struct UserSummary {
    pub name: String,
    pub total: i32,
    pub goal: i32,
    pub goal_met: bool,
    pub type_counts: Vec<(String, i32)>,
}

/// Generate a weekly summary image as PNG bytes
pub fn generate_summary_image(
    title: &str,
    period_str: &str,
    users: &[UserSummary],
    activity_types: &[String],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Layout constants
    const PADDING: u32 = 20;
    const HEADER_HEIGHT: u32 = 80;
    const ROW_HEIGHT: u32 = 40;
    const NAME_COL_WIDTH: u32 = 150;
    const STATUS_COL_WIDTH: u32 = 50;
    const TYPE_COL_WIDTH: u32 = 70;
    const TOTAL_COL_WIDTH: u32 = 80;

    // Calculate dimensions
    let num_types = activity_types.len().max(1);
    let width = PADDING * 2 + NAME_COL_WIDTH + STATUS_COL_WIDTH + (TYPE_COL_WIDTH * num_types as u32) + TOTAL_COL_WIDTH;
    let height = PADDING * 2 + HEADER_HEIGHT + ROW_HEIGHT + (ROW_HEIGHT * users.len() as u32);

    let mut svg = String::new();

    // SVG header
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"##,
        width, height
    ));

    // Background
    svg.push_str(&format!(
        r##"<rect width="{}" height="{}" fill="#2f3136"/>"##,
        width, height
    ));

    // Title
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="#ffffff">{}</text>"##,
        PADDING, PADDING + 28, escape_svg(title)
    ));

    // Period subtitle
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" fill="#b9bbbe">{}</text>"##,
        PADDING, PADDING + 50, escape_svg(period_str)
    ));

    // Table header row
    let header_y = PADDING + HEADER_HEIGHT;
    let text_y = header_y + 26;

    // Header background
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
        PADDING, header_y, width - PADDING * 2, ROW_HEIGHT
    ));

    // Column headers
    let mut x = PADDING + 10;
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="#dcddde">User</text>"##,
        x, text_y
    ));
    x += NAME_COL_WIDTH;

    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="#dcddde" text-anchor="middle">✓</text>"##,
        x + STATUS_COL_WIDTH / 2, text_y
    ));
    x += STATUS_COL_WIDTH;

    for activity_type in activity_types {
        let display_name = if activity_type.len() > 8 {
            format!("{}...", &activity_type[..6])
        } else {
            activity_type.clone()
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="12" font-weight="bold" fill="#dcddde" text-anchor="middle">{}</text>"##,
            x + TYPE_COL_WIDTH / 2, text_y, escape_svg(&display_name)
        ));
        x += TYPE_COL_WIDTH;
    }

    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="#dcddde" text-anchor="middle">Total</text>"##,
        x + TOTAL_COL_WIDTH / 2, text_y
    ));

    // Data rows
    for (i, user) in users.iter().enumerate() {
        let row_y = header_y + ROW_HEIGHT + (i as u32 * ROW_HEIGHT);
        let text_y = row_y + 26;

        // Alternating row background
        let bg_color = if i % 2 == 0 { "#2f3136" } else { "#36393f" };
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
            PADDING, row_y, width - PADDING * 2, ROW_HEIGHT, bg_color
        ));

        let mut x = PADDING + 10;

        // User name
        let display_name = if user.name.len() > 16 {
            format!("{}...", &user.name[..14])
        } else {
            user.name.clone()
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" fill="#ffffff">{}</text>"##,
            x, text_y, escape_svg(&display_name)
        ));
        x += NAME_COL_WIDTH;

        // Status emoji
        let status = if user.goal_met { "✅" } else { "⏳" };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="16" text-anchor="middle">{}</text>"##,
            x + STATUS_COL_WIDTH / 2, text_y, status
        ));
        x += STATUS_COL_WIDTH;

        // Type counts
        for activity_type in activity_types {
            let count = user.type_counts.iter()
                .find(|(t, _)| t == activity_type)
                .map(|(_, c)| *c)
                .unwrap_or(0);
            let color = if count > 0 { "#43b581" } else { "#72767d" };
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" fill="{}" text-anchor="middle">{}</text>"##,
                x + TYPE_COL_WIDTH / 2, text_y, color, count
            ));
            x += TYPE_COL_WIDTH;
        }

        // Total with goal
        let total_color = if user.goal_met { "#43b581" } else { "#faa61a" };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="{}" text-anchor="middle">{}/{}</text>"##,
            x + TOTAL_COL_WIDTH / 2, text_y, total_color, user.total, user.goal
        ));
    }

    svg.push_str("</svg>");

    render_svg_to_png(&svg, width, height)
}
