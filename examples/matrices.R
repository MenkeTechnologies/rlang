# Matrices are vectors with a `dim` attribute, stored column-major.
m <- matrix(1:12, nrow = 3)
print(m)
cat("dim:", dim(m), "\n")
stopifnot(nrow(m) == 3, ncol(m) == 4)

cat("element [2,3]:", m[2, 3], "\n")
cat("row 1:", m[1, ], "\n")
cat("col 2:", m[, 2], "\n")

print(t(m))

doubled <- m * 2
print(doubled[, 1])
cat("total:", sum(m), "\n")
