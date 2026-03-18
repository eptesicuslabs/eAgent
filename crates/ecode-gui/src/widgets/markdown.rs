//! Simple Markdown renderer for egui.
//!
//! This provides basic markdown rendering using pulldown-cmark to parse
//! and egui to render. Full featured rendering (code highlighting, etc.)
//! will be added incrementally.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Render markdown text into the egui UI.
#[allow(dead_code)]
pub fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(text, options);

    let mut in_code_block = false;
    let mut code_text = String::new();
    let mut is_bold = false;
    let mut is_italic = false;
    let is_code = false;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    ui.add_space(8.0);
                    // Heading level determines font size
                    let _ = level; // Used below in End
                }
                Tag::Paragraph => {}
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    code_text.clear();
                }
                Tag::Strong => {
                    is_bold = true;
                }
                Tag::Emphasis => {
                    is_italic = true;
                }
                _ => {}
            },

            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_level) => {
                    ui.add_space(4.0);
                }
                TagEnd::Paragraph => {
                    ui.add_space(4.0);
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    // Render code block
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(24, 24, 27))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&code_text)
                                    .monospace()
                                    .color(egui::Color32::from_rgb(212, 212, 216)),
                            );
                        });
                    code_text.clear();
                }
                TagEnd::Strong => {
                    is_bold = false;
                }
                TagEnd::Emphasis => {
                    is_italic = false;
                }
                _ => {}
            },

            Event::Text(text) => {
                if in_code_block {
                    code_text.push_str(&text);
                } else {
                    let mut rt = egui::RichText::new(text.as_ref());
                    if is_bold {
                        rt = rt.strong();
                    }
                    if is_italic {
                        rt = rt.italics();
                    }
                    if is_code {
                        rt = rt.monospace();
                    }
                    ui.label(rt);
                }
            }

            Event::Code(text) => {
                ui.label(
                    egui::RichText::new(text.as_ref())
                        .monospace()
                        .background_color(egui::Color32::from_rgb(39, 39, 42)),
                );
            }

            Event::SoftBreak => {
                // Treat as space in inline context
            }

            Event::HardBreak => {
                ui.add_space(4.0);
            }

            Event::Rule => {
                ui.separator();
            }

            _ => {}
        }
    }
}
