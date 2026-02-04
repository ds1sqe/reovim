; Markdown highlight queries (block-level only)
; NOTE: Inline elements (emphasis, links, code_span) are handled by markdown_inline grammar

; Headings
(atx_heading (atx_h1_marker) @markup.heading)
(atx_heading (atx_h2_marker) @markup.heading)
(atx_heading (atx_h3_marker) @markup.heading)
(atx_heading (atx_h4_marker) @markup.heading)
(atx_heading (atx_h5_marker) @markup.heading)
(atx_heading (atx_h6_marker) @markup.heading)
(atx_heading (inline) @markup.heading)
(setext_heading) @markup.heading

; Code blocks
(fenced_code_block) @markup.raw
(indented_code_block) @markup.raw
(info_string) @label

; Lists
(list_marker_minus) @markup.list
(list_marker_plus) @markup.list
(list_marker_star) @markup.list
(list_marker_dot) @markup.list
(list_marker_parenthesis) @markup.list
(task_list_marker_unchecked) @markup.list
(task_list_marker_checked) @markup.list

; Block quotes
(block_quote_marker) @punctuation.special

; Thematic breaks
(thematic_break) @punctuation.special

; HTML
(html_block) @markup.raw
