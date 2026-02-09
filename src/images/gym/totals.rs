// All-time totals leaderboard image generation

use crate::images::{escape_svg, render_svg_to_png};

/// Data for a single user's all-time totals
pub struct UserTotals {
    pub rank: usize,
    pub name: String,
    pub total_count: i32,
    pub achieved_goals: i32,
    pub missed_goals: i32,
    pub type_totals: Vec<(String, i32)>,
}

/// Generate an all-time totals leaderboard image as PNG bytes
pub fn generate_totals_image(
    users: &[UserTotals],
    activity_types: &[String],
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Layout constants
    const PADDING: u32 = 20;
    const HEADER_HEIGHT: u32 = 60;
    const ROW_HEIGHT: u32 = 45;
    const RANK_COL_WIDTH: u32 = 50;
    const NAME_COL_WIDTH: u32 = 150;
    const TYPE_COL_WIDTH: u32 = 70;
    const TOTAL_COL_WIDTH: u32 = 80;
    const GOALS_COL_WIDTH: u32 = 100;

    // Calculate dimensions
    let num_types = activity_types.len().max(1);
    let width = PADDING * 2 + RANK_COL_WIDTH + NAME_COL_WIDTH + (TYPE_COL_WIDTH * num_types as u32) + TOTAL_COL_WIDTH + GOALS_COL_WIDTH;
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
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="24" font-weight="bold" fill="#ffd700">🏆 All-Time Leaderboard</text>"##,
        PADDING, PADDING + 28
    ));

    // Table header row
    let header_y = PADDING + HEADER_HEIGHT;
    let text_y = header_y + 28;

    // Header background
    svg.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#202225"/>"##,
        PADDING, header_y, width - PADDING * 2, ROW_HEIGHT
    ));

    // Column headers
    let mut x = PADDING + 10;

    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="#dcddde">#</text>"##,
        x + RANK_COL_WIDTH / 2 - 5, text_y
    ));
    x += RANK_COL_WIDTH;

    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="#dcddde">User</text>"##,
        x, text_y
    ));
    x += NAME_COL_WIDTH;

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
    x += TOTAL_COL_WIDTH;

    svg.push_str(&format!(
        r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="12" font-weight="bold" fill="#dcddde" text-anchor="middle">Goals</text>"##,
        x + GOALS_COL_WIDTH / 2, text_y
    ));

    // Data rows
    for (i, user) in users.iter().enumerate() {
        let row_y = header_y + ROW_HEIGHT + (i as u32 * ROW_HEIGHT);
        let text_y = row_y + 28;

        // Alternating row background with gold tint for top 3
        let bg_color = match user.rank {
            1 => "#3d3520", // Gold tint
            2 => "#2d3035", // Silver tint
            3 => "#352d25", // Bronze tint
            _ if i % 2 == 0 => "#2f3136",
            _ => "#36393f",
        };
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"##,
            PADDING, row_y, width - PADDING * 2, ROW_HEIGHT, bg_color
        ));

        let mut x = PADDING + 10;

        // Rank with medal
        let rank_display = match user.rank {
            1 => "🥇".to_string(),
            2 => "🥈".to_string(),
            3 => "🥉".to_string(),
            n => format!("{}", n),
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="16" fill="#ffffff" text-anchor="middle">{}</text>"##,
            x + RANK_COL_WIDTH / 2 - 5, text_y, rank_display
        ));
        x += RANK_COL_WIDTH;

        // User name
        let display_name = if user.name.len() > 16 {
            format!("{}...", &user.name[..14])
        } else {
            user.name.clone()
        };
        let name_color = match user.rank {
            1 => "#ffd700",
            2 => "#c0c0c0",
            3 => "#cd7f32",
            _ => "#ffffff",
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="14" font-weight="bold" fill="{}">{}</text>"##,
            x, text_y, name_color, escape_svg(&display_name)
        ));
        x += NAME_COL_WIDTH;

        // Type totals
        for activity_type in activity_types {
            let count = user.type_totals.iter()
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

        // Total count
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="16" font-weight="bold" fill="#ffffff" text-anchor="middle">{}</text>"##,
            x + TOTAL_COL_WIDTH / 2, text_y, user.total_count
        ));
        x += TOTAL_COL_WIDTH;

        // Goals achieved/missed
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" font-family="Arial, sans-serif" font-size="12" text-anchor="middle"><tspan fill="#43b581">{}</tspan><tspan fill="#72767d"> / </tspan><tspan fill="#f04747">{}</tspan></text>"##,
            x + GOALS_COL_WIDTH / 2, text_y, user.achieved_goals, user.missed_goals
        ));
    }

    svg.push_str("</svg>");

    render_svg_to_png(&svg, width, height)
}
