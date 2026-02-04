; Markdown injection queries
;
; These queries identify regions where different languages should be highlighted.
; Only fenced code blocks use injection - inline content is handled by the
; markdown decorator for better performance.

; Fenced code blocks with language annotation
; Captures the language name from info_string and the content for injection
(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)
