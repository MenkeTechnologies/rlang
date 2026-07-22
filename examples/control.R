# Control flow: for/while/repeat, break/next, and if as an expression.
total <- 0
for (i in 1:10) {
  if (i %% 2 == 0) next
  if (i > 7) break
  total <- total + i
}
cat("odd total below 8:", total, "\n")
stopifnot(total == 16)

n <- 1
while (n < 100) n <- n * 3
cat("first power of 3 over 100:", n, "\n")

count <- 0
repeat {
  count <- count + 1
  if (count == 5) break
}
cat("repeat ran", count, "times\n")

grade <- function(score) {
  if (score >= 90) "A" else if (score >= 80) "B" else "C"
}
cat(sapply(c(95, 85, 70), grade), "\n")

fizzbuzz <- function(n) {
  if (n %% 15 == 0) "FizzBuzz" else if (n %% 3 == 0) "Fizz" else if (n %% 5 == 0) "Buzz" else as.character(n)
}
cat(sapply(1:15, fizzbuzz), "\n")
