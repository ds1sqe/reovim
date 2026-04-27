; Rust scope context query
; Captures @context for scope boundaries and @name for display text

(function_item) @context
(function_item name: (identifier) @name)

(impl_item) @context

(struct_item) @context
(struct_item name: (type_identifier) @name)

(enum_item) @context
(enum_item name: (type_identifier) @name)

(trait_item) @context
(trait_item name: (type_identifier) @name)

(mod_item) @context
(mod_item name: (identifier) @name)

(closure_expression) @context
