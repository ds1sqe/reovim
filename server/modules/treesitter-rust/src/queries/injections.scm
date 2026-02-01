; Rust injection queries for embedded Markdown in doc comments
;
; Doc comments (/// and //!) contain Markdown-formatted documentation.
; We inject Markdown highlighting for the content.
;
; Note: The full comment text (including ///) is captured. The Markdown
; parser will see the prefix but should handle it gracefully.

; Outer doc comments (/// ...) - document the following item
; Match comments starting with /// but not //// (which is a regular comment)
((line_comment) @injection.content
 (#match? @injection.content "^///[^/]")
 (#set! injection.language "markdown"))

; Handle /// with nothing after (edge case)
((line_comment) @injection.content
 (#eq? @injection.content "///")
 (#set! injection.language "markdown"))

; Inner doc comments (//! ...) - document the enclosing module
((line_comment) @injection.content
 (#match? @injection.content "^//!")
 (#set! injection.language "markdown"))

; Outer block doc comments (/** ... */)
((block_comment) @injection.content
 (#match? @injection.content "^/\\*\\*[^*]")
 (#set! injection.language "markdown"))

; Inner block doc comments (/*! ... */)
((block_comment) @injection.content
 (#match? @injection.content "^/\\*!")
 (#set! injection.language "markdown"))
