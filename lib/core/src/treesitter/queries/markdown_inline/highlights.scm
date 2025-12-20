; Markdown inline syntax highlighting

; Emphasis (italic)
(emphasis) @markup.italic

; Strong emphasis (bold)
(strong_emphasis) @markup.bold

; Inline code
(code_span) @markup.raw.inline

; Link text
(inline_link
  (link_text) @markup.link.text)

; Link destination
(inline_link
  (link_destination) @markup.link.url)

; Image alt text
(image
  (image_description) @markup.link.text)

; Strikethrough
(strikethrough) @markup.strikethrough

; Autolinks
(uri_autolink) @markup.link.url
(email_autolink) @markup.link.url

; HTML entities
(entity_reference) @constant.character.escape
(numeric_character_reference) @constant.character.escape

; Backslash escapes
(backslash_escape) @constant.character.escape
