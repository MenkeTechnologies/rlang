# Vectorized arithmetic, recycling, and NA handling.
x <- c(10, 20, 30, 40)
stopifnot(length(x) == 4)

cat("doubled:", x * 2, "\n")
cat("recycled:", x + c(1, 2), "\n")
cat("sum:", sum(x), "mean:", mean(x), "\n")

# NA is part of every atomic type and propagates through arithmetic.
y <- c(1, NA, 3)
stopifnot(is.na(y[2]))
cat("sum with NA:", sum(y), "\n")
cat("sum na.rm:", sum(y, na.rm = TRUE), "\n")

# Logical subsetting, negative subsetting, and subsetting by name.
big <- x[x > 15]
cat("big:", big, "\n")
cat("dropped first:", x[-1], "\n")

named <- c(first = 1, second = 2, third = 3)
print(named)
cat("by name:", named["second"], "\n")
stopifnot(named[["third"]] == 3)
