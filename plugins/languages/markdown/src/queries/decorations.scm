; Markdown decoration queries for visual rendering
; Used by MarkdownRenderer for concealment and visual enhancements

; Heading markers for concealment (# ## ### etc.)
(atx_heading (atx_h1_marker) @decoration.heading.1.marker)
(atx_heading (atx_h2_marker) @decoration.heading.2.marker)
(atx_heading (atx_h3_marker) @decoration.heading.3.marker)
(atx_heading (atx_h4_marker) @decoration.heading.4.marker)
(atx_heading (atx_h5_marker) @decoration.heading.5.marker)
(atx_heading (atx_h6_marker) @decoration.heading.6.marker)

; Full heading lines for background styling
(atx_heading) @decoration.heading.line

; List bullet markers for replacement (- + *)
(list_marker_minus) @decoration.list.bullet
(list_marker_plus) @decoration.list.bullet
(list_marker_star) @decoration.list.bullet

; Checkboxes for replacement
(task_list_marker_unchecked) @decoration.checkbox.unchecked
(task_list_marker_checked) @decoration.checkbox.checked

; Code blocks for background styling
(fenced_code_block) @decoration.code_block
(indented_code_block) @decoration.code_block

; Fenced code block language icon - capture opening fence and language
(fenced_code_block
  (fenced_code_block_delimiter) @decoration.code_fence_open
  (info_string
    (language) @decoration.code_lang))

; Tables (pipe tables)
(pipe_table) @decoration.table
(pipe_table_header) @decoration.table.header
(pipe_table_delimiter_row) @decoration.table.delimiter

; Horizontal rules (thematic breaks) for visual enhancement
(thematic_break) @decoration.horizontal_rule

; Blockquotes for styling
(block_quote) @decoration.blockquote
(block_quote_marker) @decoration.blockquote.marker
; Continuation markers on subsequent lines of blockquotes (only inside block_quote)
(block_quote (paragraph (inline (block_continuation) @decoration.blockquote.marker)))

; NOTE: Inline elements (emphasis, strong_emphasis, inline_link) are in
; INLINE_LANGUAGE, not LANGUAGE. See treesitter/grammar.rs for dual-grammar support.
