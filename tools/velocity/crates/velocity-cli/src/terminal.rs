use std::io::IsTerminal;

use comfy_table::{CellAlignment, ContentArrangement, Table, presets::UTF8_FULL};
use termimad::MadSkin;

pub fn print_markdown(markdown: &str) {
    let skin = if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        MadSkin::default()
    } else {
        MadSkin::no_style()
    };
    skin.print_text(markdown);
}

pub fn print_table(
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
    right_aligned_columns: &[usize],
) {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL.with_rounded_corners())
        .set_header(headers.to_vec());

    if std::io::stdout().is_terminal() {
        table.set_content_arrangement(ContentArrangement::Dynamic);
    } else {
        table.force_no_tty();
    }

    table.add_rows(rows);
    for &column_index in right_aligned_columns {
        if let Some(column) = table.column_mut(column_index) {
            column.set_cell_alignment(CellAlignment::Right);
        }
    }

    println!("{table}");
}
