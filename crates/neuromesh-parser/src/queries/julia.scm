; NeuroMesh extract profile — tree-sitter-julia
(function_definition
  (signature
    (call_expression
      (identifier) @function.name))) @function

; short form: name(args) = body
(assignment
  (call_expression
    (identifier) @function.name)
  (operator)
  (_)) @function

; struct Foo / struct Foo{T} / struct Foo{T} <: Base
(struct_definition
  (type_head (identifier) @class.name)) @class

(struct_definition
  (type_head
    (parametrized_type_expression (identifier) @class.name))) @class

(struct_definition
  (type_head
    (binary_expression
      (parametrized_type_expression (identifier) @class.name)))) @class

(struct_definition
  (type_head
    (binary_expression (identifier) @class.name))) @class

(abstract_definition
  (type_head (identifier) @symbol.name)) @symbol

(abstract_definition
  (type_head
    (binary_expression (identifier) @symbol.name))) @symbol

(using_statement) @import
(import_statement) @import

; include("file.jl")
((call_expression
  (identifier) @_inc
  (argument_list (string_literal))) @import
  (#eq? @_inc "include"))

(call_expression) @call
