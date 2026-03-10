; Markdown inline decoration queries
;
; These captures are mapped to AnnotationKind via DecorationRule.
; Runs on tree_sitter_md::INLINE_LANGUAGE grammar.

; Code span delimiters (backticks) — concealed
(code_span (code_span_delimiter) @code_span.delimiter)

; Code span content — background styling
(code_span) @code_span

; Emphasis (italic) — styled as italic
(emphasis) @emphasis

; Strong emphasis (bold) — styled as bold
(strong_emphasis) @strong

; Strikethrough — styled as strikethrough
(strikethrough) @strikethrough

; Link URL and surrounding brackets — conceal destination
(inline_link (link_destination) @link.destination)
