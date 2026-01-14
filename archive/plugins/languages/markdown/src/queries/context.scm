; Context query for Markdown
; Captures heading nodes for sticky context headers

; ATX headings (# Header)
(atx_heading
  (atx_h1_marker)
  heading_content: (_) @name) @context

(atx_heading
  (atx_h2_marker)
  heading_content: (_) @name) @context

(atx_heading
  (atx_h3_marker)
  heading_content: (_) @name) @context

(atx_heading
  (atx_h4_marker)
  heading_content: (_) @name) @context

(atx_heading
  (atx_h5_marker)
  heading_content: (_) @name) @context

(atx_heading
  (atx_h6_marker)
  heading_content: (_) @name) @context
