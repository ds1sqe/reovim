; Context query for Rust
; Captures scope-defining nodes for sticky context headers

; Functions and methods
(function_item
  name: (identifier) @name) @context

; Impl blocks
(impl_item
  type: (_) @name) @context

; Struct definitions
(struct_item
  name: (type_identifier) @name) @context

; Enum definitions
(enum_item
  name: (type_identifier) @name) @context

; Trait definitions
(trait_item
  name: (type_identifier) @name) @context

; Module definitions
(mod_item
  name: (identifier) @name) @context
