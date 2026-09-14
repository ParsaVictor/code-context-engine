; NeuroMesh extract profile — tree-sitter-r
; `name <- function(...)` / `name = function(...)`
(binary_operator
  lhs: (identifier) @function.name
  rhs: (function_definition)) @function

; library(pkg) / require(pkg) / source("file.R")
((call
  function: (identifier) @_loader
  arguments: (arguments (argument value: (_)))) @import
  (#match? @_loader "^(library|require|source|requireNamespace)$"))

(call) @call
