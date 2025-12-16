//! Landing page content shown when starting without a file

const VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO: [&str; 5] = [
    r"                      _",
    r" _ __ ___  _____   __(_)_ __ ___",
    r"| '__/ _ \/ _ \ \ / /| | '_ ` _ \",
    r"| | |  __/ (_) \ V / | | | | | | |",
    r"|_|  \___|\___/ \_/  |_|_| |_| |_|",
];

/// Generate landing page content centered for the given screen dimensions
#[must_use]
pub fn generate(width: u16, height: u16) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Help hints
    let version_line = format!("version {VERSION}");
    let hints: [&str; 7] = [
        "",
        &version_line,
        "",
        "Type :q<Enter> to exit",
        "Type i to enter insert mode",
        "Type :help<Enter> for help",
        "",
    ];

    // Calculate total content height
    let content_height = LOGO.len() + hints.len();
    let start_row = if height as usize > content_height {
        (height as usize - content_height) / 2
    } else {
        0
    };

    // Add empty lines for vertical centering
    for _ in 0..start_row {
        lines.push(String::new());
    }

    // Add logo lines (centered as a block)
    let logo_width = LOGO.iter().map(|l| l.len()).max().unwrap_or(0);
    let logo_padding = if (width as usize) > logo_width {
        (width as usize - logo_width) / 2
    } else {
        0
    };
    for logo_line in &LOGO {
        lines.push(format!("{:padding$}{}", "", logo_line, padding = logo_padding));
    }

    // Add hint lines (centered)
    for hint in &hints {
        lines.push(center_text(hint, width));
    }

    // Fill remaining lines
    while lines.len() < height as usize {
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Center text within the given width
fn center_text(text: &str, width: u16) -> String {
    let text_len = text.chars().count();
    if text_len >= width as usize {
        return text.to_string();
    }
    let padding = (width as usize - text_len) / 2;
    format!("{:padding$}{}", "", text, padding = padding)
}
