//! `MermaidFences` claiming a fence inside a real Markdown document.
//!
//! The unit of work here is the whole path: `pulldown-cmark` keeps the info
//! token, `Code` offers it to the hook, and the hook turns it into a diagram —
//! while every other fence in the same document is declined and comes back as
//! an ordinary code block.

use hydrolysis_m3::Material3;
use waterui::prelude::*;
use waterui::view::hook::Hook;
use waterui::widget::RichText;
use waterui::widget::code::CodeConfig;
use waterui_mermaid::MermaidFences;
use waterui_testing::{OffscreenApp, Role, ui as test_ui};

const DOC: &str = "\
# Pipeline

```mermaid
flowchart LR
    Parse --> Layout
    Layout --> Draw
```

```rust
fn main() {}
```
";

fn app() -> OffscreenApp {
    test_ui()
        .viewport(900, 900)
        .theme(Material3::defaults())
        .mount_offscreen(|| RichText::from_markdown(DOC).hooks(MermaidFences))
}

#[core::prelude::v1::test]
fn a_mermaid_fence_becomes_a_diagram() {
    let mut app = app();
    for label in ["Parse", "Layout", "Draw"] {
        app.query().role(Role::LABEL).label(label).assert_exists();
    }
}

/// The hook declines every other token, so the `rust` fence keeps the code
/// block's header. Without this the test above would still pass with a hook
/// that swallowed the whole document.
#[core::prelude::v1::test]
fn every_other_fence_is_still_a_code_block() {
    let mut app = app();
    app.query().role(Role::LABEL).label("Rust").assert_exists();
}

/// Claims every fence.
#[derive(Clone)]
struct Bridged;

impl Hook for Bridged {
    type Target = CodeConfig;

    fn apply(&self, _config: CodeConfig, _env: &Environment) -> Result<impl View, CodeConfig> {
        Ok(text("bridged"))
    }
}

/// A hook scoped closer to the document than `MermaidFences` is offered the
/// fence first, and keeps it.
#[core::prelude::v1::test]
fn a_nearer_hook_keeps_the_fence() {
    let mut app = test_ui()
        .viewport(600, 400)
        .theme(Material3::defaults())
        .mount_offscreen(|| {
            RichText::from_markdown(DOC)
                .hooks(Bridged)
                .hooks(MermaidFences)
        });

    app.query()
        .role(Role::LABEL)
        .label("bridged")
        .assert_exists();
    assert!(
        app.query()
            .role(Role::LABEL)
            .label("Parse")
            .all()
            .is_empty(),
        "the nearer hook must keep the fence, not hand it to Mermaid"
    );
}
