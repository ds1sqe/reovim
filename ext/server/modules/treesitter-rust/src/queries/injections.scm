; Rust injection queries for embedded Markdown in doc comments
;
; Doc comments (/// and //!) contain Markdown-formatted documentation.
; We inject Markdown highlighting for the content.
;
; Line doc comments use injection.combined to merge consecutive lines
; into a single Markdown document for proper cross-line parsing.

; Outer doc comments (/// ...) - document the following item
; Match comments starting with /// but not //// (which is a regular comment)
((line_comment) @injection.content
 (#match? @injection.content "^///[^/]")
 (#set! injection.language "markdown")
 (#set! injection.combined))

; Handle /// with nothing after (edge case)
((line_comment) @injection.content
 (#eq? @injection.content "///")
 (#set! injection.language "markdown")
 (#set! injection.combined))

; Inner doc comments (//! ...) - document the enclosing module
((line_comment) @injection.content
 (#match? @injection.content "^//!")
 (#set! injection.language "markdown")
 (#set! injection.combined))

; Outer block doc comments (/** ... */) -- NOT combined (single block)
((block_comment) @injection.content
 (#match? @injection.content "^/\\*\\*[^*]")
 (#set! injection.language "markdown"))

; Inner block doc comments (/*! ... */) -- NOT combined (single block)
((block_comment) @injection.content
 (#match? @injection.content "^/\\*!")
 (#set! injection.language "markdown"))
