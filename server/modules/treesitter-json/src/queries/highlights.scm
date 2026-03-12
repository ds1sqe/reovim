; JSON highlight queries

; Strings (keys and values)
(pair key: (string) @property)
(string) @string
(escape_sequence) @string.escape

; Numbers
(number) @number

; Booleans
(true) @boolean
(false) @boolean

; Null
(null) @constant.builtin

; Punctuation
["{" "}"] @punctuation.bracket
["[" "]"] @punctuation.bracket
[":" ","] @punctuation.delimiter
