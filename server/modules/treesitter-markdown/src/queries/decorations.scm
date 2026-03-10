; Markdown decoration queries (block-level)
;
; These captures are mapped to AnnotationKind via DecorationRule.
; Capture names are deliberately short — the rule provides the category.

; Heading markers (# ## ### etc.) — concealed with level-specific icons
(atx_heading (atx_h1_marker) @heading.1.marker)
(atx_heading (atx_h2_marker) @heading.2.marker)
(atx_heading (atx_h3_marker) @heading.3.marker)
(atx_heading (atx_h4_marker) @heading.4.marker)
(atx_heading (atx_h5_marker) @heading.5.marker)
(atx_heading (atx_h6_marker) @heading.6.marker)

; List bullets — concealed with unicode bullet char
(list_marker_minus) @list.bullet
(list_marker_plus) @list.bullet
(list_marker_star) @list.bullet

; Checkboxes — concealed with check/uncheck glyphs
(task_list_marker_unchecked) @checkbox.unchecked
(task_list_marker_checked) @checkbox.checked

; Code blocks — background highlighting
(fenced_code_block) @code_block
(indented_code_block) @code_block

; Blockquote markers — concealed with bar
(block_quote_marker) @blockquote.marker

; Horizontal rules — concealed with line
(thematic_break) @horizontal_rule
