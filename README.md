# waterui-mermaid

Mermaid diagrams for WaterUI, drawn through `Scene2D`.

```rust
use waterui_mermaid::mermaid;

mermaid("flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Go]\n  B -->|no| A")
```

## How a diagram gets on screen

Mermaid source is parsed and laid out by
[merman](https://github.com/water-rs/merman), which tracks upstream Mermaid's
own grammar and geometry. Everything after that is this crate's:

- **Layout is measured with WaterUI's text engine.** merman sizes every node
  and every label through a host-supplied measurer, and this crate supplies one
  backed by the same `parley` engine and the very same `FontCollection` the
  host installed for every other component. Without it, boxes would be sized
  from a browser compatibility profile and the glyphs inside them drawn from
  ours, and the text would not fit.
- **Geometry is drawn through `Scene2D`.** Node outlines, subgraph frames and
  routed connectors are vector paths on the shared scene contract, so one
  drawing path serves every backend.
- **Text is not drawn into the scene.** Labels are real `text()` views placed
  into the boxes layout reserved for them, which is what gives a diagram a
  meaningful accessibility tree and the platform's own text rendering.
- **Colours come from theme tokens.** A diagram follows a light/dark switch
  and a custom accent because it reads the same tokens every other component
  reads, never Mermaid's CSS themes.

A diagram is drawn at its natural size. Scaling it to fit would break the
agreement between a reserved box and the glyphs in it, so a diagram larger
than its container is the container's to scroll.

## Markdown

A ```` ```mermaid ```` fence inside a Markdown document stays a plain code
block until the environment carries this crate's hook — `waterui` has no
dependency on `waterui-mermaid`, which is what lets a build that does not want
diagrams render the fence as code. Install it once at the root of the app:

```rust
use waterui::env::use_env;
use waterui::metadata::Metadata;
use waterui::prelude::*;

use_env(|mut env: Environment| {
    waterui_mermaid::install(&mut env);
    Metadata::new(scroll(include_markdown!("example.md").padding()), env)
})
```

`examples/markdown` mounts a full document this way and captures it to a PNG:

```text
cargo run --example markdown
```

The PNG lands at `markdown/document/rendered.png` under
`WATERUI_TEST_ARTIFACTS_DIR`, or the system temp dir when that is unset.

## Consuming the crate

`waterui-mermaid` is not on crates.io: its `merman-core`/`merman-render`
dependencies come from the water-rs/merman fork while the typed layout
projection is unreleased upstream, and crates.io refuses git-only
dependencies. Until it can publish, depend on this repository directly:

```toml
waterui-mermaid = { git = "https://github.com/water-rs/mermaid" }
```

The changelog, tags and GitHub releases are still produced by release-plz.

## Testing

```bash
cargo test --workspace --all-targets --all-features   # unit and end-to-end tests
cargo test --workspace --doc --all-features           # doctests
```

The end-to-end tests mount an offscreen host and assert on the accessibility
tree — a diagram's labels are real views, so that is where they are found.
The export tests write PNGs under `WATERUI_TEST_ARTIFACTS_DIR` for visual
review.

## License

Apache-2.0 OR MIT
