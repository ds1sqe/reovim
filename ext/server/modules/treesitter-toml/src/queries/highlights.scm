; TOML highlight queries

; Keys
(bare_key) @property
(quoted_key) @property
(dotted_key) @property

; Tables
(table (bare_key) @type)
(table (quoted_key) @type)
(table (dotted_key) @type)
(table_array_element (bare_key) @type)
(table_array_element (quoted_key) @type)
(table_array_element (dotted_key) @type)

; Strings
(string) @string
(escape_sequence) @string.escape

; Numbers
(integer) @number
(float) @number.float
(local_date) @number
(local_time) @number
(local_date_time) @number
(offset_date_time) @number

; Booleans
(boolean) @boolean

; Comments
(comment) @comment

; Punctuation
["[" "]" "[[" "]]"] @punctuation.bracket
["{" "}"] @punctuation.bracket
["=" "." ","] @punctuation.delimiter
