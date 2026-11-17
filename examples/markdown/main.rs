//! The `example.md` document — a Markdown page whose ```` ```mermaid ```` fence
//! is claimed by `waterui-mermaid` — mounted offscreen and captured to a PNG for
//! visual review.
//!
//! ```text
//! cargo run --example markdown
//! ```
//!
//! The PNG lands at `markdown/document/rendered.png` under
//! `WATERUI_TEST_ARTIFACTS_DIR`, or the system temp dir when that is unset.

use waterui::prelude::*;
use waterui_mermaid::MermaidFences;
use waterui_testing::{Role, ui as test_ui};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut app = test_ui()
        .viewport(800, 1000)
        .theme(hydrolysis_m3::Material3::defaults())
        .mount_offscreen(|| {
            // Scoping the Mermaid hook is the application's job — `waterui`
            // has no dependency on `waterui-mermaid`, which is what lets a
            // build that does not want diagrams render the fence as plain
            // code.
            scroll(include_markdown!("example.md").padding()).hooks(MermaidFences)
        });

    // The fence became a diagram: its node and edge labels reached the
    // accessibility tree, which a plain code block does not produce.
    for label in ["Markdown", "Diagram", "Code block"] {
        app.query().role(Role::LABEL).label(label).assert_exists();
    }

    let captured = app.capture_snapshot("markdown", "document", "rendered");
    tracing::info!(path = %captured.path().display(), "rendered the markdown example");
}
