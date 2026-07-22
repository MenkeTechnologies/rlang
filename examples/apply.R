# The apply family and functional helpers.
squares <- sapply(1:6, function(n) n^2)
print(squares)
stopifnot(squares[6] == 36)

evens <- Filter(function(n) n %% 2 == 0, 1:10)
cat("evens:", evens, "\n")

total <- Reduce(function(a, b) a + b, 1:10)
cat("reduce sum:", total, "\n")
stopifnot(total == 55)

lengths <- sapply(c("a", "bb", "ccc"), nchar)
print(lengths)

pairs <- Map(function(a, b) a * b, 1:3, 4:6)
print(unlist(pairs))

cat("do.call:", do.call(sum, list(1, 2, 3)), "\n")
