# Reo Color System

## Design Philosophy

The Reo color palette merges the best aspects of three popular themes:
- **Tokyo Night** - Blue/purple focus, cohesive saturation
- **Catppuccin Mocha** - Warm pastels, excellent variety
- **One Dark** - High contrast, readability

## Color Science Principles

### 1. Saturation Consistency

All accent colors maintain 70-85% saturation for visual weight balance.
Higher saturation = more visual "weight" and prominence.

```
Low saturation (40%)  → Soft, muted, background elements
Medium saturation (70%) → Standard UI, most syntax
High saturation (85%+) → Important, attention-grabbing
```

### 2. Lightness Hierarchy

- Foreground text: 75-85% lightness
- Accent colors: 65-85% lightness
- Background surfaces: 12-30% lightness
- Gap of ~50-60 points ensures WCAG AA contrast

### 3. Hue Organization

Colors are grouped by hue clusters for semantic meaning:

| Cluster | Hue Range | Semantic Role |
|---------|-----------|---------------|
| Warm | 0-60° | Danger, constants, parameters |
| Green | 90-150° | Strings, properties, success |
| Cool | 170-240° | Types, functions, imports |
| Purple | 260-320° | Keywords, control flow, macros |

### 4. Perceptual Compensation

HSL lightness is NOT perceptually uniform - yellow appears brighter than
blue at the same lightness. We compensate by:
- Slightly reducing saturation for warmer colors (yellow, orange)
- Adjusting lightness based on hue

## Complete Palette Reference

### Warm Cluster (0-60° Hue)

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| Red | #f38ba8 | 343°, 81%, 75% | Danger, mut |
| Red Bright | #ff6b6b | 0°, 100%, 71% | Errors |
| Maroon | #eba0ac | 350°, 65%, 77% | Alt warnings |
| Coral | #ff8a80 | 5°, 100%, 75% | HTML tags |
| Orange | #fab387 | 23°, 92%, 75% | Constants |
| Peach | #ffb86c | 27°, 100%, 71% | Floats |
| Gold | #f9e2af | 41°, 86%, 83% | Integers |
| Yellow | #e5c07b | 39°, 67%, 69% | Parameters |
| Yellow Soft | #f0d9a0 | 42°, 71%, 78% | Patterns |

### Green Cluster (90-150° Hue)

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| Green | #98c379 | 95°, 38%, 62% | Strings |
| Green Bright | #a6e3a1 | 115°, 54%, 76% | Diff add |
| Lime | #c3e88d | 82°, 68%, 73% | Interpolation |
| Teal | #56b6c2 | 187°, 47%, 55% | Properties |
| Teal Dark | #1abc9c | 168°, 76%, 42% | Lifetimes |
| Mint | #94e2d5 | 170°, 57%, 73% | Characters |

### Cool Cluster (170-240° Hue)

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| Cyan | #7dcfff | 197°, 100%, 74% | Attributes |
| Cyan Bright | #89dceb | 189°, 71%, 73% | Constructors |
| Sky | #74c7ec | 199°, 76%, 69% | Info |
| Aqua | #2ac3de | 189°, 71%, 52% | Types |
| Turquoise | #89ddff | 191°, 100%, 77% | Operators |
| Blue | #7aa2f7 | 220°, 89%, 72% | Functions |
| Blue Bright | #89b4fa | 217°, 92%, 76% | Definitions |
| Blue Dark | #3d59a1 | 222°, 45%, 44% | Modules |
| Indigo | #6366f1 | 239°, 84%, 67% | Deep blue |

### Purple Cluster (260-320° Hue)

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| Purple | #9d7cd8 | 267°, 53%, 67% | Keywords |
| Purple Bright | #bb9af7 | 267°, 84%, 79% | Control flow |
| Lavender | #c7b0fa | 259°, 88%, 84% | Type keywords |
| Mauve | #cba6f7 | 267°, 84%, 81% | Alt keywords |
| Magenta | #f5c2e7 | 316°, 72%, 86% | Macros |
| Pink | #ff79c6 | 326°, 100%, 74% | Async/loops |
| Flamingo | #f2cdcd | 0°, 59%, 88% | Decorators |
| Rosewater | #f5e0dc | 10°, 56%, 91% | Special |

### Neutral Cluster (Desaturated)

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| FG | #c0caf5 | 227°, 70%, 85% | Default text |
| FG Dim | #a9b1d6 | 227°, 36%, 75% | Secondary text |
| FG Muted | #9399b2 | 227°, 20%, 65% | Tertiary text |
| Comment | #565f89 | 225°, 23%, 44% | Comments |
| Overlay | #6c7086 | 227°, 12%, 47% | Muted elements |
| Gutter | #3b4261 | 224°, 25%, 30% | Line numbers |
| Surface2 | #45475a | 225°, 13%, 31% | Elevated |
| Surface1 | #313244 | 231°, 16%, 23% | UI backgrounds |
| Surface0 | #1e1e2e | 240°, 23%, 15% | Base background |
| Black | #181825 | 240°, 21%, 12% | Deepest |

### Semantic Status Colors

| Name | Hex | HSL | Role |
|------|-----|-----|------|
| Error | #f38ba8 | 343°, 81%, 75% | Errors |
| Warning | #f9e2af | 41°, 86%, 83% | Warnings |
| Info | #89b4fa | 217°, 92%, 76% | Information |
| Hint | #a6e3a1 | 115°, 54%, 76% | Suggestions |
| Add | #a6e3a1 | 115°, 54%, 76% | Diff additions |
| Change | #89b4fa | 217°, 92%, 76% | Diff changes |
| Delete | #f38ba8 | 343°, 81%, 75% | Diff deletions |

## Style Modifiers

Combined with 45 colors, these create 120+ unique appearances:

| Modifier | Semantic Use | Example |
|----------|--------------|---------|
| **Bold** | Definitions, important items | `fn main` |
| *Italic* | Builtins, parameters, types | `self`, `T` |
| Underline | Links, clickable elements | `[link](url)` |

## Color Combination Rules

To ensure every highlight target is visually unique, we follow these rules:

1. **No duplicate (color, style) pairs** - Each capture gets a unique combination
2. **Semantic grouping** - Related syntax elements share hue clusters
3. **Contrast within clusters** - Vary saturation/lightness within a hue family

Example within the Purple Cluster:
- `keyword`: Purple + Italic
- `keyword.function`: Magenta (no modifier)
- `keyword.struct`: Lavender + Bold
- `keyword.control`: Purple Bright (no modifier)
- `keyword.control.repeat`: Pink (no modifier)

## References

- [Catppuccin Palette](https://catppuccin.com/palette/)
- [Tokyo Night](https://github.com/folke/tokyonight.nvim)
- [Nord Color Docs](https://www.nordtheme.com/docs/colors-and-palettes/)
- [HSL Color Theory](https://dev.to/futuristicgeeks/hsl-color-theory)
- [WCAG Contrast Guidelines](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html)
