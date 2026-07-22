# Closures, default arguments, `...`, and lexical scope through `<<-`.
power <- function(exp = 2) {
  function(x) x^exp
}
square <- power()
cube <- power(3)
cat("square(4) =", square(4), "\n")
cat("cube(3) =", cube(3), "\n")
stopifnot(square(4) == 16, cube(3) == 27)

counter <- function() {
  n <- 0
  function() {
    n <<- n + 1
    n
  }
}
tick <- counter()
invisible(tick())
invisible(tick())
cat("counter:", tick(), "\n")

describe <- function(label, ..., sep = ", ") {
  parts <- c(...)
  paste0(label, ": ", paste(parts, collapse = sep))
}
cat(describe("letters", "a", "b", "c"), "\n")

# Arguments match by exact tag, then unique prefix, then position.
plot_point <- function(x, y, colour = "black") paste0("(", x, ",", y, ") ", colour)
cat(plot_point(1, 2), "\n")
cat(plot_point(y = 2, x = 1, col = "red"), "\n")
