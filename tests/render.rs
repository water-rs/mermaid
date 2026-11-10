//! Rendered diagrams, rasterized through the offscreen Hydrolysis renderer and
//! written out to be looked at.
//!
//! What the geometry has to *be* is asserted in the library's own unit tests;
//! these frames are how the drawing is judged by eye: every node shape, every
//! edge style and marker, subgraphs, and a sequence diagram's lifelines and
//! fragments.

use std::path::PathBuf;

use hydrolysis_m3::Material3;
use waterui_mermaid::mermaid;
use waterui_testing::ui;

/// Every node shape, edge stroke and edge marker a flowchart has, with
/// routed corners.
const SHAPES: &str = "\
flowchart LR
    rect[Rectangle] --> round(Rounded)
    round --> stadium([Stadium])
    stadium --> sub[[Subroutine]]
    sub --> db[(Database)]
    db --> circ((Circle))
    circ --> dbl(((Double)))
    dbl --> asym>Asymmetric]
    asym --> dia{Diamond}
    dia --> hex{{Hexagon}}
    hex --> par[/Parallelogram/]
    par --> trap[/Trapezoid\\]
    rect -.-> dia
    rect ==> hex
    rect --o circ
    rect --x db
    rect <--> stadium
";

const SUBGRAPHS: &str = "\
flowchart TD
    subgraph Ingest
        A[Read] --> B[Parse]
    end
    subgraph Serve
        C[Render] --> D{Cached?}
        D -->|no| E[(Store)]
    end
    B --> C
    E --> A
";

const SEQUENCE: &str = "\
sequenceDiagram
    actor User
    participant App
    participant Server
    User->>App: Open diagram
    App->>Server: Fetch source
    alt cached
        Server-->>App: Source
    else miss
        Server-->>App: Not found
        Note over App: Show an error
    end
    loop every minute
        App->>Server: Poll
    end
";

fn output(file: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mermaid");
    std::fs::create_dir_all(&directory).expect("output directory must be creatable");
    directory.join(file)
}

fn render(name: &str, source: &'static str, width: u32, height: u32) {
    let mut app = ui()
        .viewport(width, height)
        .theme(Material3::defaults())
        .mount_offscreen(move || mermaid(source));
    app.snapshot()
        .save_png(output(&format!("{name}.png")))
        .expect("the png must be writable");
}

#[core::prelude::v1::test]
fn every_node_shape_and_edge_style_renders() {
    render("shapes", SHAPES, 1400, 700);
}

#[core::prelude::v1::test]
fn subgraphs_render() {
    render("subgraphs", SUBGRAPHS, 700, 700);
}

#[core::prelude::v1::test]
fn a_sequence_diagram_renders() {
    render("sequence", SEQUENCE, 800, 700);
}
