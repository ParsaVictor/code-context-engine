; NeuroMesh extract profile — tree-sitter-typescript
(function_declaration
  name: (identifier) @function.name) @function

(method_definition
  name: (_) @function.name) @function

(variable_declarator
  name: (identifier) @function.name
  value: [(arrow_function) (function_expression)]) @function

(class_declaration
  name: (_) @class.name) @class

(interface_declaration
  name: (_) @symbol.name) @symbol

(type_alias_declaration
  name: (_) @symbol.name) @symbol

(enum_declaration
  name: (_) @symbol.name) @symbol

(import_statement) @import

(call_expression) @call

; `app.handle = function handle(req, res) {...}` — the module pattern of
; Express, Connect and much of the CommonJS world. Without this the file has
; no symbols, nothing resolves to it, and its bodies never reach a packet.
(assignment_expression
  left: (member_expression
    object: (identifier) @function.parent
    property: (property_identifier) @function.name)
  right: [(function_expression) (arrow_function)]) @function

; `View.prototype.lookup = function lookup(name) {...}` — the pre-class
; prototype pattern, still how express's View is written.
(assignment_expression
  left: (member_expression
    object: (member_expression
      object: (identifier) @function.parent
      property: (property_identifier) @_proto)
    property: (property_identifier) @function.name)
  right: [(function_expression) (arrow_function)]
  (#eq? @_proto "prototype")) @function
