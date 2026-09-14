; NeuroMesh extract profile — tree-sitter-python
(function_definition
  name: (identifier) @function.name) @function

(class_definition
  name: (identifier) @class.name) @class

(import_statement) @import

(import_from_statement) @import

(call) @call

; `MAX_ATTEMPTS = 5` at module level — a SCREAMING_SNAKE constant is a symbol a
; question points at by name (F54). Nested assignments are not captured.
(module
  (expression_statement
    (assignment
      left: (identifier) @symbol.name
      (#match? @symbol.name "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)+$"))) @symbol)
