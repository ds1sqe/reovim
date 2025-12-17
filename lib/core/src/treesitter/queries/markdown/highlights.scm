; Markdown highlight queries

; Headings
(atx_heading (atx_h1_marker) @markup.heading)
(atx_heading (atx_h2_marker) @markup.heading)
(atx_heading (atx_h3_marker) @markup.heading)
(atx_heading (atx_h4_marker) @markup.heading)
(atx_heading (atx_h5_marker) @markup.heading)
(atx_heading (atx_h6_marker) @markup.heading)
(atx_heading (inline) @markup.heading)
(setext_heading (heading_content) @markup.heading)

; Emphasis
(emphasis) @markup.italic
(strong_emphasis) @markup.bold

; Code
(code_span) @markup.raw
(fenced_code_block) @markup.raw
(indented_code_block) @markup.raw
(info_string) @label

; Links
(link_text) @markup.link
(link_destination) @markup.link.url
(link_title) @string
(image_description) @markup.link

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
