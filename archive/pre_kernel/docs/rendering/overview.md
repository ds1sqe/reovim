# Rendering System Overview

This document describes reovim's rendering architecture, which combines a data pipeline with plugin UI systems.

## Two Aspects of Rendering

1. **Render Pipeline** - Transforms buffer content into terminal output through composable stages
2. **Plugin UI Systems** - Three systems for different UI patterns (overlays, windows, components)

## Quick Decision Matrix

| Use Case | System | When to Use |
|----------|--------|-------------|
| Popup/Modal | **Overlay** | Temporary UI over editor (completion, microscope) |
| Sidebar/Panel | **WindowProvider** | Persistent side panel (explorer, outline) |
| Status/Tab line | **UIComponent** | Fixed position UI elements |
| Syntax highlighting | **RenderStage** | Transform buffer content |

## Pipeline Overview

```
Buffer → Visibility → Highlighting → Decorations → Visual → Indent → FrameBuffer → Terminal
```

Each stage adds information to the `RenderData` structure, which accumulates all the data needed to render a window.

## Z-Order System

Higher values render on top of lower values:

| Layer | Z-Order | Use |
|-------|---------|-----|
| `BASE` | 0 | Base layer for main content |
| `WINDOW` | 10 | Window content |
| `STATUS_LINE` | 50 | Status line elements |
| `OVERLAY` | 100 | Overlay popups |
| `COMPLETION` | 110 | Completion menu |
| `COMMAND_LINE` | 120 | Command line (topmost) |

## Performance Considerations

- **Render stages**: Process entire windows at once, not line-by-line
- **Overlays**: Only rendered when visible
- **UIComponents**: Rendered every frame - keep very efficient
- **Caching**: HighlightCache and DecorationCache avoid re-computation

## Related Documentation

- [Pipeline](./pipeline.md) - Data transformation stages
- [UI Systems](./ui-systems.md) - Plugin UI rendering
- [Custom Stages](./custom-stages.md) - Extending the pipeline
