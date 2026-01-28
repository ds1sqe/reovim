; Rust language injections
; Inject markdown into doc comments (///, //!, /** */, /*! */)

((line_comment (doc_comment)) @injection.content
 (#set! injection.language "markdown"))

((block_comment (doc_comment)) @injection.content
 (#set! injection.language "markdown"))
