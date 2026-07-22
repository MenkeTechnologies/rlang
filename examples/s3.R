# S3: dispatch on the `class` attribute through UseMethod.
circle <- function(r) structure(list(r = r), class = c("circle", "shape"))
square <- function(side) structure(list(side = side), class = c("square", "shape"))

area <- function(s) UseMethod("area")
area.circle <- function(s) 3.141593 * s$r^2
area.square <- function(s) s$side^2
area.default <- function(s) stop("not a shape")

describe <- function(s) UseMethod("describe")
describe.shape <- function(s) paste("a shape of area", round(area(s), 2))

shapes <- list(circle(1), square(3))
for (s in shapes) {
  cat(class(s)[1], "->", round(area(s), 3), "\n")
}

cat(describe(circle(2)), "\n")
stopifnot(inherits(circle(1), "shape"))
