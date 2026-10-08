x <- c(3, 1, 2)
print(sort(x))
print(rev(x))
print(order(x))
#==#
print(1:5 * 2)
print(c(1L, 2L) + 1L)
print(2^10)
print(7 %/% 2)
print(-5 %% 3)
print(1/0)
#==#
print(c(1, 2.5, 3))
print(c(TRUE, FALSE, NA))
print(c("a", NA))
print(c(1, "a", TRUE))
#==#
v <- c(a = 1, b = 2, c = 3)
print(v)
print(v["b"])
print(v[c(1, 3)])
print(names(v))
#==#
x <- 1:10
print(x[x > 5])
print(x[-(1:5)])
print(x[c(TRUE, FALSE)])
print(length(x))
#==#
f <- function(a, b = 10) a + b
print(f(1))
print(f(1, 2))
print(f(b = 3, a = 4))
#==#
counter <- function() {
  n <- 0
  function() {
    n <<- n + 1
    n
  }
}
step <- counter()
print(step())
print(step())
print(step())
#==#
print(sapply(1:5, function(i) i * i))
print(unlist(lapply(1:3, function(i) i + 1)))
print(Filter(function(x) x %% 2 == 0, 1:10))
print(Reduce(function(a, b) a * b, 1:5))
#==#
s <- "Hello, World"
print(nchar(s))
print(toupper(s))
print(substr(s, 1, 5))
print(strsplit(s, ", ")[[1]])
print(gsub("o", "0", s))
print(grepl("World", s))
#==#
print(paste("a", "b", "c"))
print(paste0("x", 1:3))
print(paste(c("a", "b"), collapse = "+"))
print(sprintf("%d items at %.2f", 3L, 1.5))
print(sprintf("%-6s|", "ab"))
#==#
m <- matrix(1:6, nrow = 2)
print(m)
print(dim(m))
print(m[2, 3])
print(m[, 2])
print(t(m))
#==#
l <- list(a = 1, b = "two", c = c(3, 4))
print(l$a)
print(l[["b"]])
print(l$c[2])
print(names(l))
print(length(l))
#==#
l <- list(1, 2)
l[[3]] <- 3
print(length(l))
l$name <- "x"
print(l$name)
#==#
total <- 0
for (i in 1:10) {
  if (i %% 2 == 0) next
  if (i > 7) break
  total <- total + i
}
print(total)
#==#
i <- 0
while (TRUE) {
  i <- i + 1
  if (i >= 5) break
}
print(i)
#==#
fib <- function(n) if (n < 2) n else fib(n - 1) + fib(n - 2)
print(sapply(0:10, fib))
#==#
print(sum(1:100))
print(mean(c(1, 2, 3, 4)))
print(median(c(3, 1, 2)))
print(max(c(1, 9, 5)))
print(range(c(4, 2, 8)))
print(prod(1:5))
#==#
print(sd(c(2, 4, 4, 4, 5, 5, 7, 9)))
print(var(c(1, 2, 3, 4)))
print(cumsum(1:5))
print(diff(c(1, 4, 9, 16)))
#==#
print(round(2.5))
print(round(3.14159, 2))
print(floor(-1.5))
print(ceiling(1.2))
print(abs(-3L))
print(sqrt(16))
#==#
print(is.na(c(1, NA, 3)))
print(sum(c(1, NA, 3), na.rm = TRUE))
print(NA > 1)
print(NA & FALSE)
print(NA | TRUE)
#==#
print(seq(1, 10, by = 2))
print(seq_len(5))
print(seq_along(c("a", "b", "c")))
print(rep(1:2, times = 3))
print(rep(1:2, each = 2))
#==#
print(unique(c(1, 2, 2, 3, 1)))
print(union(1:3, 2:5))
print(intersect(1:5, 3:8))
print(setdiff(1:5, 3:8))
print(1:5 %in% c(2, 4))
#==#
print(which(c(FALSE, TRUE, TRUE)))
print(which.max(c(1, 9, 3)))
print(any(c(FALSE, TRUE)))
print(all(c(TRUE, TRUE)))
#==#
print(head(1:10, 3))
print(tail(1:10, 3))
print(identical(c(1, 2), c(1, 2)))
print(ifelse(c(1, 2, 3) > 2, "big", "small"))
#==#
x <- c(1, 2, 3)
x[2] <- 20
print(x)
x[5] <- 50
print(x)
names(x) <- c("a", "b", "c", "d", "e")
print(x)
#==#
p <- list(name = "circle", r = 2)
class(p) <- "shape"
area <- function(s) UseMethod("area")
area.shape <- function(s) 3.14 * s$r^2
print(area(p))
print(class(p))
print(inherits(p, "shape"))
#==#
describe <- function(x) UseMethod("describe")
describe.default <- function(x) "unknown"
describe.numeric <- function(x) "a number"
print(describe(1))
print(describe("s"))
#==#
`%+%` <- function(a, b) paste0(a, b)
print("foo" %+% "bar")
#==#
add <- function(...) sum(...)
print(add(1, 2, 3))
count <- function(...) length(list(...))
print(count("a", "b"))
#==#
f <- function(x) {
  if (x < 0) return("negative")
  "non-negative"
}
print(f(-1))
print(f(1))
#==#
x <- 5
print(if (x > 3) "big" else "small")
print(TRUE && FALSE)
print(FALSE || TRUE)
print(!c(TRUE, FALSE))
#==#
print(as.integer("42"))
print(as.numeric("3.5"))
print(as.character(10))
print(as.logical("TRUE"))
print(typeof(1L))
print(typeof(1))
print(class(c("a")))
#==#
print(do.call(sum, list(1, 2, 3)))
print(do.call(paste, list("a", "b", sep = "-")))
#==#
print(Map(function(a, b) a + b, 1:3, 4:6))
#==#
x <- 1:20
print(x)
#==#
print(nchar(c("a", "bb", "ccc")))
print(trimws("  pad  "))
print(startsWith("prefix", "pre"))
print(sort(c("banana", "apple", "cherry")))
#==#
v <- 1:5
v[v > 3] <- 0
print(v)
#==#
lst <- list(a = 1, b = 2)
lst[["a"]] <- 100
print(lst$a)
lst$b <- NULL
print(length(lst))
#==#
print(vapply(1:3, function(i) i * 2, numeric(1)))
print(setNames(1:3, c("a", "b", "c")))
#==#
x <- list(1, 2, 3)
print(sapply(x, function(e) e * 10))
#==#
f <- function(n) {
  acc <- numeric(0)
  for (i in seq_len(n)) acc <- c(acc, i^2)
  acc
}
print(f(5))
#==#
print(seq(0, 1, length.out = 5))
print(1:3 |> sum())
#==#
print(20000100000)
print(1e-10)
print(1234567890123)
print(1e5)
print(0.0001)
print(c(1e10, 1))
print(2^31)
print(as.character(1e5))
#==#
cat(1/3, "\n")
cat(1e10, "\n")
cat(TRUE, NA, "\n")
#==#
print(c("tab\there", "quote\"q"))
cat(c("a", "b"), sep = "\n")
cat("X")
#==#
print(unlist(list(a = 1, b = list(2, 3))))
print(Reduce(`+`, 1:4))
print(sapply(1:3, `-`))
print(`[`(c(10, 20, 30), 2))
#==#
g <- function(n) if (n == 0) 0 else n + g(n - 1)
print(g(500))
#==#
l <- list(1, "a", c(TRUE, FALSE))
print(l)
print(list(x = 1, y = "two"))
print(list())
#==#
print(seq(0, 1, 0.25))
print(seq(2, 10, by = 2))
print(sprintf("%+d", 5))
print(sprintf("%05d", -5))
print(sprintf("%e", 1.5))
print(sprintf("%g", 100000))
print(formatC(42, width = 6, flag = "0"))
print(format(1.5, nsmall = 3))
print(prettyNum(1234567, big.mark = ","))
print(signif(123.456, 2))
#==#
print(10 %% 0.04)
print(10 %/% 0.04)
print(-7 %% 3)
print(round(0.15, 1))
print(round(2.675, 2))
print(0 * -2)
print(1e-17)
#==#
print(match(c(3, 1), c(1, 2, 3)))
print(rank(c(3, 1, 2, 2)))
print(duplicated(c(1, 2, 2, 3, 3)))
print(xor(TRUE, FALSE))
print(bitwAnd(12L, 10L))
print(mapply(function(a, b) a + b, 1:3, 3:1))
print(Reduce(`+`, 1:4, accumulate = TRUE))
#==#
g <- function(x = 3) x
print(g())
f <- function(x, y = 2) x * y
print(f(5))
#==#
print(rowSums(matrix(1:6, nrow = 2)))
print(colSums(matrix(1:6, nrow = 2)))
print(apply(matrix(1:6, nrow = 2), 1, sum))
print(diag(matrix(1:9, nrow = 3)))
print(matrix(1:6, nrow = 2) %*% diag(3))
#==#
print(factor(c("b", "a", "b")))
print(levels(factor(c("b", "a"))))
print(as.integer(factor(c("b", "a", "b"))))
print(table(c(1, 1, 2, 3, 3, 3)))
print(as.vector(table(c(10, 2, 10, 1))))
#==#
print(strsplit("fooBar", "o+"))
print(regmatches("fooBar", regexpr("[a-z]+", "fooBar")))
print(pi)
print(letters[1:3])
#==#
print(sin(1))
print(cos(0))
print(atan2(1, 2))
print(tanh(1))
print(expm1(0.001))
print(log1p(0.001))
print(factorial(5))
print(choose(6, 2))
print(gamma(5))
print(lgamma(10))
print(beta(2, 3))
print(sign(c(-3, 0, 5)))
#==#
print(pmax(c(1, 5, 2), c(3, 2, 4)))
print(pmin(c(1, 5), c(3, 2)))
print(cummax(c(1, 3, 2, 5)))
print(cummin(c(5, 2, 3, 1)))
print(tabulate(c(1, 2, 2, 3), 3))
print(findInterval(c(1.5, 3), c(1, 2, 3)))
#==#
print(outer(1:2, 1:3))
print(cbind(1:2, 3:4))
print(rbind(1:2, 3:4))
print(crossprod(matrix(1:4, 2)))
print(chartr("ab", "AB", "abcab"))
print(strtoi("ff", 16))
print(Position(function(x) x > 2, c(1, 3, 2)))
print(Find(function(x) x > 2, c(1, 3, 2)))
#==#
print(is.nan(c(1, NaN, NA)))
print(is.finite(c(1, Inf, NA, NaN)))
print(is.infinite(c(1, -Inf, Inf)))
print(anyNA(c(1, 2, NA)))
print(complete.cases(c(1, NA, 3)))
print(max(numeric(0)))
print(min(integer(0)))
print(sum(c(1, NaN, 3), na.rm = TRUE))
print(mean(c(2, NaN, 1, NA), na.rm = TRUE))
#==#
print(strrep("ab", 3))
print(trimws("  x  ", which = "left"))
print(substring("hello", 1:3))
print(encodeString("a\tb"))
x <- "hello"; substr(x, 1, 1) <- "H"; print(x)
print(.Machine$integer.max)
print(format(123.456, digits = 2))
#==#
print(split(1:5, c("a", "b", "a", "b", "c")))
print(tapply(c(1, 2, 3, 4), c("a", "b", "a", "b"), sum))
print(modifyList(list(a = 1, b = 2), list(b = 3)))
print(Reduce(`-`, 1:4, right = TRUE))
print(rapply(list(1, 2), function(x) x * 2, how = "unlist"))
print(vapply(1:3, function(x) c(x, x^2), numeric(2)))
#==#
m <- matrix(1:4, 2); m[2, 2] <- 9; print(m)
m <- matrix(1:6, 2); m[1, ] <- c(7, 8, 9); print(m)
print(cumsum(1:4))
#==#
print(switch("b", a = 1, b = 2, c = 3))
print(switch("z", a = 1, b = 2, 99))
print(switch(2, "x", "y", "z"))
print(switch("a", a = , b = 2))
print(is.null(switch("q", a = 1)))
f <- function(t) switch(t, int = "I", chr = "C", "other"); print(f("chr"))
print(switch("b", a = stop("unreached"), b = 42))
#==#
print(casefold("ABC"))
print(casefold("abc", upper = TRUE))
print(chartr("a-c", "A-C", "abcdef"))
print(chartr("a-z", "A-Z", "hello"))
fact <- function(n) if (n <= 1) 1 else n * Recall(n - 1); print(fact(6))
fib <- function(n) if (n < 2) n else Recall(n - 1) + Recall(n - 2); print(fib(10))
#==#
print(diff(1:10, lag = 2))
print(diff(c(1, 4, 9, 16), differences = 2))
print(grepl("ABC", "abcabc", ignore.case = TRUE))
print(sub("WORLD", "X", "hi world", ignore.case = TRUE))
print(deparse(1:3))
print(deparse(c(1.5, 2.5)))
print(deparse(c("a", "b")))
print(deparse(c(TRUE, NA)))
#==#
print(as.integer(cut(1:5, c(0, 2, 4, 6))))
print(nlevels(cut(1:10, c(0, 5, 10))))
print(cut(c(1, 5, 10), c(0, 3, 6, 11)))
print(droplevels(factor(c("a", "b"), levels = c("a", "b", "c"))))
print(factor(c("b", "a"), levels = c("a", "b"), ordered = TRUE))
print(mean(c(NaN, NA, NA), na.rm = TRUE))
print(mean(numeric(0)))
print(format(100.25 / 0.333, nsmall = 5))
print(sapply(1:3, function(x) x, USE.NAMES = TRUE))
#==#
print(sprintf("%o", 64))
print(rev(c(a = 1, b = 2, c = 3)))
print(rep_len(1:3, 7))
print(seq.int(2, 10, 2))
print(unname(c(a = 1, b = 2)))
print(all.equal(1, 1 + 1e-10))
print(isTRUE(all.equal(1, 2)))
print(all.equal(c(2.25, 3.14), c(2.25, 1.5)))
#==#
print(format(c(1, 10, 100)))
print(format(c(1.5, 10.25)))
print(format(c("a", "bb", "ccc")))
print(format(c(1.5, 22.25, 333.125)))
#==#
print(Negate(is.null)(NULL))
print(Negate(is.na)(c(1, NA, 3)))
print(Filter(Negate(is.na), c(1, NA, 3, NA, 5)))
print(Vectorize(function(x, y) x + y)(1:3, 4:6))
print(Vectorize(function(x) x^2)(1:4))
print(is.function(Negate(is.null)))
print(sapply(c(1, NA, 3), Negate(is.na)))
#==#
print(array(1:24, c(2, 3, 4))[2, 3, 4])
print(dim(array(1:24, c(2, 3, 4))))
print(length(array(0, c(2, 3, 4))))
print(apply(array(1:24, c(2, 3, 4)), 3, sum))
a <- array(1:8, c(2, 2, 2)); print(a[1, , ])
print(array(1:8, c(2, 2, 2)))
print(aperm(matrix(1:6, 2)))
print(startsWith("abc", c("a", "x")))
#==#
print(quantile(1:100, 0.5))
print(quantile(c(1, 2, 3, 4), c(0.25, 0.75)))
print(quantile(1:10))
print(cor(1:5, c(2, 4, 6, 8, 10)))
print(cor(c(1, 2, 3, 4), c(4, 3, 2, 1)))
#==#
print(rle(c(1, 1, 2, 3, 3, 3))$lengths)
print(rle(c(1, 1, 2, 3, 3, 3)))
print(inverse.rle(rle(c(1, 1, 2, 2, 2))))
print(sort(c(3, 1, 2), index.return = TRUE)$ix)
print(rowSums(array(1:8, c(2, 2, 2))))
#==#
print(cor(c(-2, -2, -2), c(1, 2, 3)))
print(cor(c(5, 5, 5), c(5, 5, 5)))
#==#
print(rep(1:3, times = c(1, 2, 3)))
print(rep(c("a", "b"), times = c(2, 3)))
print(rep(1:3, length.out = 5))
print(rep(1:3, times = 2, each = 2))
#==#
f <- factor(c("a", "b", "a", "c"))
print(table(f))
y <- c(1, 2, 2, 3)
print(table(y))
print(table(c(TRUE, FALSE, TRUE)))
print(table(c("a", "b", "a")))
print(levels(factor(c(TRUE, FALSE, TRUE))))
#==#
z <- c("x", "y", "x")
t <- table(z)
print(attributes(t))
print(names(t))
print(dim(t))
print(dimnames(t))
print(as.vector(t))
#==#
print(list(a = list(b = 1, c = 2)))
print(list(1, list(2, 3)))
print(list(p = list(q = list(r = 1))))
#==#
m <- matrix(1:6, nrow = 2, dimnames = list(c("r1", "r2"), c("c1", "c2", "c3")))
print(m["r1", ])
print(m[, "c1"])
print(m["r2", "c2"])
print(m[c("r1", "r2"), "c3"])
print(m["r1", , drop = FALSE])
print(m[, c("c1", "c3")])
m["r1", "c2"] <- 99L
print(m)
#==#
print(regexpr("an", c("apple", "banana", "cherry")))
print(gregexpr("a", "banana"))
print(regmatches("banana", regexpr("an", "banana")))
#==#
print(which(matrix(c(TRUE, FALSE, TRUE, TRUE), 2), arr.ind = TRUE))
a <- array(c(TRUE, FALSE, TRUE, TRUE, FALSE, FALSE, TRUE, FALSE), dim = c(2, 2, 2))
print(which(a, arr.ind = TRUE))
print(which(c(TRUE, FALSE, TRUE), arr.ind = TRUE))
mn <- matrix(c(TRUE, FALSE, TRUE, TRUE), 2, dimnames = list(c("a", "b"), c("x", "y")))
print(which(mn, arr.ind = TRUE))
#==#
print(format("a", width = 5))
print(format(c("a", "bb"), width = 4))
print(format(1.5, width = 8))
print(format(42L, width = 6))
print(format(c(TRUE, FALSE), width = 7))
#==#
x <- 1:3
attr(x, "foo") <- "bar"
print(x)
m <- matrix(1:4, 2)
attr(m, "k") <- "v"
print(m)
l <- list(1)
attr(l, "q") <- "z"
print(l)
#==#
print(sapply(1:3, function(i) c(a = i, b = i * 2)))
print(sapply(list(p = 1, q = 2), function(i) c(a = i, b = i * 2)))
print(sapply(c(p = 1, q = 2), function(i) c(i, i * 2)))
print(vapply(1:2, function(i) c(a = i, b = i), c(a = 0, b = 0)))
print(sapply(1:3, function(i) c(i, i * 2)))
#==#
a <- array(1:8, dim = c(2, 2, 2), dimnames = list(c("r1", "r2"), c("c1", "c2"), c("s1", "s2")))
print(dimnames(a))
print(a["r1", "c2", "s1"])
print(a)
print(a[, , "s2"])
print(a["r1", , ])
b <- array(1:8, dim = c(2, 2, 2))
print(b)
print(b[1, 2, 1])
#==#
f <- function(x) x + 1
print(f)
g <- function(x, y = 2) {
  z <- x * y
  if (y > 3) y else 0
  z
}
print(g)
print(deparse(f))
print(deparse(g))
h <- function(a, b) if (a > b) a else b
print(h)
#==#
print(deparse(function(x) x/2))
print(deparse(function(x) x^2))
print(deparse(function(x) x %% 3))
print(deparse(function(x) x %in% c(1, 2)))
print(deparse(function(x) (x + 1) * 2))
print(deparse(function(x) -(x + 1)))
print(deparse(function(x, ...) list(...)))
print(deparse(function(x = c(1, 2), y = "a", z = TRUE, w = NULL) x))
print(deparse(function(x) function(y) x + y))
#==#
print(deparse(function(x) { for (i in 1:3) print(i) }))
print(deparse(function(x) { while (x > 0) x <- x - 1 }))
print(deparse(function(x) { repeat break }))
print(deparse(function(x) { if (x) { 1 } else { 2 } }))
print(deparse(function(x) { if (x) 1 }))
print(deparse(function(x) { y <- 1; y <<- 2; y }))
print(deparse(function(x) {}))
print(deparse(function(x) { if (x > 1) 1 else if (x > 0) 2 else 3 }))
#==#
print(deparse(function(a) { a + 100000 + 200000 + 300000 + 400000 + 500000 + 600000 + 700000 + 800000 }))
print(deparse(function(a) longfunctionnamehere(aaaaaaaaaa, bbbbbbbbbb, cccccccccc, dddddddddd, eeeeeeeeee)))
print(deparse(function(a) { g <- function(b) { h <- function(c) { k <- function(d) { m <- function(e) e } } } }))
print(deparse(function(x) `my var` + 1))
print(deparse(function(x) x[i, j]))
print(deparse(function(x) x[, 1]))
print(deparse(function(x) names(x) <- 1))
print(deparse(sum))
print(format(function(x) x + 1))
#==#
x <- c(1, 2, 3)
y <- c(4, 5, 6)
print(rbind(x, y))
print(cbind(x, y))
print(rbind(x, c(9, 9, 9)))
print(rbind(a = x, y))
print(rbind(x, x))
print(cbind(x, 1:3))
print(rbind(1:3, 4:6))
print(rbind(x, y, deparse.level = 0))
print(rbind(x + 0, y, deparse.level = 2))
#==#
m <- matrix(1:4, 2, dimnames = list(c("r1", "r2"), c("c1", "c2")))
print(rbind(m, c(9, 9)))
print(rbind(m, z = c(9, 9)))
print(cbind(m, c(9, 9)))
print(rbind(matrix(1:4, 2), x = c(9, 9)))
print(rbind(c("a", "b"), c("c", "d")))
s <- c("p", "q")
print(rbind(s, s))
print(rbind(NULL, 1:2))
print(rbind(numeric(0), 1:2))
print(rbind(x, y)["x", ])
#==#
m <- matrix(1:6, 2)
dimnames(m) <- list(c("a", "b"), c("x", "y", "z"))
print(m)
rownames(m) <- c("p", "q")
print(m)
colnames(m) <- c("i", "j", "k")
print(m)
print(dimnames(m))
rownames(m) <- NULL
print(m)
dimnames(m) <- NULL
print(m)
#==#
m <- matrix(1:6, 2, dimnames = list(c("r1", "r2"), c("c1", "c2", "c3")))
print(apply(m, 1, sum))
print(apply(m, 2, sum))
print(apply(m, 1, function(r) r * 2))
print(apply(m, 2, range))
print(apply(m, c(1, 2), function(v) v + 1))
a <- array(1:8, c(2, 2, 2), dimnames = list(c("x", "y"), c("p", "q"), c("u", "v")))
print(apply(a, 3, sum))
print(apply(a, c(1, 3), sum))
#==#
print(sort(c(2, NA, 1), na.last = TRUE))
print(sort(c(2, NA, 1), na.last = FALSE))
print(sort(c("b", NA, "a"), na.last = TRUE))
print(order(c(3, 1, NA, 2)))
print(order(c(3, 1, NA, 2), na.last = FALSE))
print(order(c(1, 1, 2), decreasing = TRUE))
print(order(c(2, 1), c(1, 2)))
print(order(c(1, 1), c(2, 1)))
print(order(c(1, NA), c(NA, 1)))
print(sort(c(2, NA, 1), na.last = TRUE, decreasing = TRUE))
print(order(c(1, NA, NaN, 2)))
#==#
print(mean(c(1, NA)))
print(mean(c(1, NaN)))
print(mean(c(1, NA, NaN)))
print(mean(c(1, NaN, NA)))
print(median(c(NaN, 1, 2)))
print(sum(c(10, NaN)))
print(prod(c(2, NaN)))
print(sum(c(1, NA, NaN)))
print(mean(c(1, NA), na.rm = TRUE))
print(var(c(1, 2, NaN)))
#==#
print.myclass <- function(x, ...) cat("<myclass>\n")
obj <- structure(list(1), class = "myclass")
print(obj)
obj
format.myclass <- function(x, ...) "FMT"
print(format(obj))
as.character.myclass <- function(x, ...) "CHR"
print(as.character(obj))
zz <- structure(list(1), class = "zzz")
print(zz)
print(structure(1:3, class = "aa", myattr = "hi"))
#==#
cat(NULL, "x")
cat("\n")
cat("a", NULL, "b")
cat("\n")
cat(NULL, NULL, "x")
cat("\n")
cat(list())
cat(1:3, c("x", "y"), "\n")
cat(c("a", "b"), sep = "")
cat("\n")
#==#
print(paste("a", NULL, "b"))
print(paste("a", character(0), "b"))
print(paste0("a", NULL))
print(paste(NULL))
print(paste(NULL, collapse = "+"))
print(paste(1:2, 1:4))
print(paste(c("a", NA), "z"))
#==#
print(format(1e6))
print(format(1e5))
print(format(0.0001))
print(format(1e-10))
print(format(c(1e6, 1)))
print(format(1e6, big.mark = ","))
print(format(123456, big.mark = ","))
print(format(1e6, nsmall = 2))
print(format(1, nsmall = 5))
print(format(123456789, digits = 3))
print(format(1e6, scientific = FALSE))
print(format(123, scientific = TRUE))
print(format(1000000L))
#==#
(x <- 5)
y <- 3
(y)
(invisible(7))
f <- function() invisible(9)
(f())
((11))
#==#
print(seq(0))
print(seq(-3))
print(seq(0.5))
print(seq(2.7))
print(seq(5, length.out = 3))
print(seq(length.out = 4))
print(seq(2, by = 1, length.out = 4))
print(seq(5, by = -2))
print(seq(along.with = c(9, 9, 9)))
print(seq(5, 5, length.out = 4))
print(seq(1, 5, length.out = 0))
#==#
print(sprintf("%*d", 5, 42))
print(sprintf("%-*d|", 5, 42))
print(sprintf("%.*f", 2, 3.14159))
print(sprintf("%*s|", 6, "ab"))
print(sprintf("%*d", -5, 42))
#==#
f <- factor("a")
print(c(is.integer(f), is.numeric(f), is.double(f)))
print(c(is.integer(1L), is.integer(1), is.double(1)))
print(is.vector(matrix(1)))
print(is.vector(structure(1, foo = "x")))
print(is.vector(c(a = 1)))
print(is.vector(list(1)))
print(class(array(1, c(1, 1, 1))))
print(class(matrix(1)))
#==#
f <- factor(c("b", "a", "c", "a"))
print(f[1:2])
print(f[-1])
print(f[10])
print(f[c(TRUE, FALSE)])
print(f[[2]])
print(f[0])
print(levels(f[1:2]))
print(class(f[1:2]))
print(nlevels(f[1:2]))
#==#
f <- factor(c("b", "a", "c", "a"))
print(head(f, 3))
print(tail(f, 2))
print(rev(f))
print(sort(f))
print(sort(f, decreasing = TRUE))
print(unique(f))
print(rep(f, 2))
print(rep(f, each = 2))
#==#
f <- factor(c("b", "a", "c", "a"))
g <- factor(c("a", "d"))
print(c(f))
print(c(f, g))
print(c(f, "q"))
print(c(f, 1))
print(union(f, g))
print(intersect(f, g))
print(setdiff(f, g))
#==#
f <- factor(c("b", "a", "c", "a"))
print(f == "a")
print(f != "a")
print(f == "zzz")
print(f == 1)
print(f[f == "a"])
print(which(f == "a"))
print(sum(f == "a"))
print(f == factor(c("a", "b", "c", "a")))
#==#
o <- factor(c("lo", "hi", "mid"), levels = c("lo", "mid", "hi"), ordered = TRUE)
print(o)
print(o[1:2])
print(o < "hi")
print(o >= "mid")
print(sort(o))
print(max(o))
print(min(o))
print(o[0])
#==#
f <- factor(c("b", "a", "c", "a"))
print(as.vector(f))
print(as.character(f))
print(as.integer(f))
print(paste(f, collapse = "-"))
print(toString(f))
print(match(f, c("a", "b", "c")))
print(f %in% c("a", "c"))
print(duplicated(f))
#==#
f <- factor(c("b", "a", "c", "a"))
print(f[1:2, drop = TRUE])
print(droplevels(f[1:2]))
print(table(f[1:2]))
print(split(f, c(1, 1, 2, 2)))
print(split(1:4, f))
print(tapply(c(10, 20, 30, 40), f, sum))
#==#
h <- factor(c("a", NA, "b"))
print(h)
print(h == "a")
print(is.na(h))
print(h[1:2])
nm <- factor(c("x", "y"))
names(nm) <- c("n1", "n2")
print(nm)
print(rev(nm))
print(nm[1])
#==#
print(tryCatch(1 + 1, error = function(e) "caught"))
print(tryCatch(stop("boom"), error = function(e) conditionMessage(e)))
print(tryCatch(stop("boom"), error = function(e) class(e)))
print(tryCatch(stop("x"), condition = function(c) "cond"))
print(tryCatch(warning("w!"), warning = function(w) paste("W:", conditionMessage(w))))
print(tryCatch(message("m!"), message = function(m) paste("M:", conditionMessage(m))))
print(tryCatch(sqrt(-1), warning = function(w) conditionMessage(w)))
#==#
r <- tryCatch(stop("e"), error = function(e) "h", finally = cat("FIN\n"))
print(r)
r2 <- tryCatch("ok", finally = cat("FIN2\n"))
print(r2)
print(tryCatch(tryCatch(stop("inner"), error = function(e) stop("outer")),
               error = function(e) conditionMessage(e)))
print(tryCatch(tryCatch(stop("deep"), warning = function(w) "wrong"),
               error = function(e) conditionMessage(e)))
#==#
f <- function() { on.exit(cat("exit\n")); cat("body\n"); invisible("v") }
print(f())
g <- function() { on.exit(cat("a\n")); on.exit(cat("b\n"), add = TRUE); invisible(1) }
invisible(g())
h <- function() { on.exit(cat("cleanup\n")); stop("bad") }
print(tryCatch(h(), error = function(e) conditionMessage(e)))
#==#
print(local({ a <- 5; a * 2 }))
x <- 1
local({ x <- 99 })
print(x)
print(local({ q <- 2; local({ q + 1 }) }))
ff <- function() { y <- 10; local({ y * 3 }) }
print(ff())
#==#
print(class(simpleError("z")))
print(class(simpleCondition("z")))
print(simpleError("t"))
print(simpleWarning("w"))
e <- simpleCondition("msg")
print(conditionMessage(e))
print(conditionCall(e))
#==#
print(class(try(stop("t"), silent = TRUE)))
print(inherits(try(stop("t"), silent = TRUE), "try-error"))
print(try("fine", silent = TRUE))
#==#
print.foo <- function(x, ...) { cat("foo\n"); NextMethod() }
print.bar <- function(x, ...) { cat("bar\n"); NextMethod() }
print(structure(1:3, class = c("foo", "bar")))
summ <- function(x, ...) UseMethod("summ")
summ.a <- function(x, ...) c("a", NextMethod())
summ.b <- function(x, ...) c("b", NextMethod())
summ.default <- function(x, ...) "end"
print(summ(structure(1, class = c("a", "b"))))
as.character.money <- function(x, ...) paste0("$", NextMethod())
print(as.character(structure(5, class = "money")))
#==#
print((function() { invisible(1); 3 })())
print((function() { x <- 1; 3 })())
print((function() { x <- 1; if (TRUE) 3 })())
print((function() { x <- 1; if (FALSE) 3 else 4 })())
(function() { invisible(1); 3 })()
(function() { x <- 1; 3 })()
(function() { x <- 1; invisible(3) })()
{ y <- 2; 9 }
x <- 5
invisible(7)
for (i in 1:2) i
while (FALSE) 1
if (FALSE) 1
(function() { on.exit(cat("x\n")); 3 })()
(function() { on.exit(cat("a\n")); on.exit(cat("b\n"), add = TRUE); 4 })()
tryCatch(42, finally = cat("f\n"))
#==#
print(withCallingHandlers({ warning("w"); cat("resumed\n"); 7 },
  warning = function(x) { cat("H", conditionMessage(x), "\n"); invokeRestart("muffleWarning") }))
print(withCallingHandlers({ message("m"); cat("resumed\n"); 8 },
  message = function(x) { cat("H", conditionMessage(x)); invokeRestart("muffleMessage") }))
withCallingHandlers(withCallingHandlers({ warning("w2"); cat("resumed\n") },
  warning = function(x) cat("inner\n")),
  warning = function(x) { cat("outer\n"); invokeRestart("muffleWarning") })
withCallingHandlers(withCallingHandlers({ warning("w3"); cat("resumed\n") },
  warning = function(x) { cat("inner\n"); invokeRestart("muffleWarning") }),
  warning = function(x) cat("outer\n"))
print(tryCatch(withCallingHandlers({ warning("w4"); cat("NOT\n"); 1 },
  warning = function(x) cat("calling\n")),
  warning = function(x) paste("exiting", conditionMessage(x))))
print(withCallingHandlers(tryCatch({ warning("w5"); 1 }, warning = function(x) "exiting"),
  warning = function(x) cat("calling\n")))
withCallingHandlers({ warning("w6"); cat("resumed\n") },
  condition = function(x) cat("cond\n"),
  warning = function(x) { cat("warn\n"); invokeRestart("muffleWarning") })
print(tryCatch(withCallingHandlers(stop("e1"), error = function(e) cat("calling\n")),
  error = function(e) paste("exiting", conditionMessage(e))))
print(suppressWarnings({ warning("s1"); cat("resumed\n"); 3 }))
print(suppressMessages({ message("s2"); cat("resumed\n"); 4 }))
print(suppressWarnings(as.numeric("zz")))
#==#
print(withRestarts(invokeRestart("r1", 5), r1 = function(v) v * 2))
print(withRestarts({ cat("body\n"); invokeRestart("r1"); cat("NOT\n") }, r1 = function() "done"))
print(withRestarts({ cat("body\n"); 9 }, r1 = function() 0))
withRestarts(print(length(computeRestarts())), r1 = function() 1, r2 = function() 2)
withRestarts(for (x in computeRestarts()) cat(x$name, "\n"), r1 = function() 1, r2 = function() 2)
withRestarts(for (x in computeRestarts()) cat("[", restartDescription(x), "]\n"),
  r1 = list(handler = function() 1, description = "listed"), r2 = "plain")
print(withRestarts(withRestarts(invokeRestart("r1", 2), r1 = function(v) paste("inner", v)),
  r1 = function(v) paste("outer", v)))
print(withRestarts(withRestarts({ x <- computeRestarts()[[2]]; invokeRestart(x, 2) },
  r1 = function(v) paste("inner", v)), r1 = function(v) paste("outer", v)))
print(withRestarts(tryCatch(invokeRestart("r1", 3), error = function(e) "WRONG",
  finally = cat("fin\n")), r1 = function(v) paste("restart", v)))
print(withRestarts((function() { on.exit(cat("exit\n")); invokeRestart("r1", 4) })(),
  r1 = function(v) paste("restart", v)))
print(tryCatch(invokeRestart("nope"), error = function(e) conditionMessage(e)))
withRestarts(print(computeRestarts()), r1 = function() 1)
withCallingHandlers(warning("w"), warning = function(x) {
  for (y in computeRestarts()) cat(y$name, "\n"); invokeRestart("muffleWarning") })
print(withRestarts(withCallingHandlers({ warning("w"); "NOT" },
  warning = function(x) invokeRestart("r1", 6)), r1 = function(v) paste("jumped", v)))
print(isRestart(withRestarts(computeRestarts()[[1]], r1 = function() 1)))
#==#
print(suppressWarnings(factor(c("a", "b")) < "b"))
print(tryCatch(factor(c("a", "b")) < "b", warning = function(w) conditionMessage(w)))
withCallingHandlers(print(factor(c("a", "b")) < "b"),
  warning = function(w) { cat("H:", conditionMessage(w), "\n"); invokeRestart("muffleWarning") })
print(suppressWarnings(sqrt(-1)))
print(tryCatch(sqrt(-1), warning = function(w) conditionMessage(w)))
print(withCallingHandlers(sqrt(-1), warning = function(w) invokeRestart("muffleWarning")))
#==#
x <- "café"
print(c(nchar(x), nchar(x, type = "bytes"), nchar(x, type = "width")))
print(nchar(c("a", "éé", "日本語", ""), type = "bytes"))
print(nchar(c("→", "　", "😀", "́"), type = "width"))
print(nchar("a\tb", type = "width"))
print(c("naïve", "日本語", "ß"))
print(matrix(c("日本", "a", "bb", "ccc"), 2, 2))
print(c(a = "日本語", bb = "x"))
print(format(c("日本語", "ab")))
print(format("日本語", width = 8))
print(formatC(c("ab", "日本語"), width = 8))
print(formatC("ab", width = 8, flag = "-"))
print(strtrim(c("abcdef", "日本語"), c(3, 4)))
print(sprintf("[%6s][%-6s]", "café", "café"))
#==#
print(toupper("straße"))
print(tolower("ΣΑΣ"))
print(tolower("İstanbul"))
print(toupper(c("ŉ", "ΐ", "ﬅ", "ﬁ", "ᾀ", "ᾳ")))
print(nchar(toupper("straße"), type = "bytes"))
print(utf8ToInt("é"))
print(utf8ToInt("日本"))
print(intToUtf8(c(26085, 26412)))
print(intToUtf8(65:70, multiple = TRUE))
print(intToUtf8(utf8ToInt("Ωμέγα")))
#==#
print(utf8ToInt("\x41\x42"))
print(utf8ToInt("\xc3\xa9"))
print(utf8ToInt("\101\102"))
print(utf8ToInt("\1011"))
print(utf8ToInt("\a\b\f\v\t\n\r"))
print(nchar(c("a\0b", "a\x00b", "a\000b")))
print(utf8ToInt("　b"))
print(utf8ToInt("\u{e9}z"))
print(utf8ToInt("\U0001F600"))
print(utf8ToInt("\u0e9"))
print(utf8ToInt("\ "))
#==#
print(t(1:3))
print(t(t(1:3)))
print(t(c(a = 1, b = 2)))
print(t(matrix(1:6, 2, dimnames = list(c("r1", "r2"), c("a", "b", "c")))))
print(dim(t(1:3)))
print(t(matrix(1:6, 2)))
#==#
print(2147483647L + 1L)
print(2147483647L * 2L)
print(-2147483647L - 2L)
print(-2147483647L - 1L)
print(c(2147483647L, 1L) + c(1L, 1L))
print(2000000000L + 2000000000L)
print(typeof(2147483647L + 1L))
print(2147483647L + 0L)
print(-2147483647L - 0L)
print(2147483647 + 1)
print(1L + 1L)
print(1:5 * 1000L)
#==#
print(append(1:5, 0, after = 0))
print(append(1:3, 99, after = 1))
print(append(1:3, 4:5))
print(append(1:3, 99, after = 10))
print(append(list(1, 2), list(9), after = 1))
print(append(c(a = 1, b = 2, c = 3), c(z = 9), after = 1))
#==#
print(cut(1:10, 3))
print(cut(c(1, 5, 9), 3))
print(cut(c(1, 2, 3), breaks = c(0, 2, 4)))
print(cut(c(5, 5, 5), 2))
print(cut(1:20, 4))
print(cut(1:10, 3, labels = FALSE))
print(cut(c(1, 5, 9), 3, labels = FALSE))
#==#
print(vapply(character(0), nchar, integer(1)))
print(vapply(integer(0), function(x) "a", character(1)))
print(vapply(list(), function(x) x, numeric(1)))
print(names(vapply(character(0), nchar, integer(1))))
print(vapply(1:3, function(x) x * 1, numeric(1)))
print(sapply(1:2, function(x) list(x)))
print(sapply(1:3, function(x) c(x, x)))
#==#
print(c(a = 1, b = 2)[3])
print(c(a = 1, b = 2, c = 3)[c("a", "zz")])
print(list(a = 1, b = 2)["zz"])
print(list(a = 1, b = 2)[3])
print((1:2)[5])
print(names(list(a = 1, 2)))
print(list(a = 1, 2))
#==#
print(matrix(1:4, 2)[0, ])
print(matrix(1:4, 2)[, 0])
print(matrix(character(0), 0, 0))
print(matrix(0, 0, 3))
print(matrix(1:30, 15)[0, ])
print(matrix(1:4, 2, dimnames = list(c("rrrr1", "r2"), c("a", "b")))[0, ])
print(format(NULL))
print(format(character(0)))
print(format(NA))
print(format(c(1, NA)))
#==#
print(NA^0)
print(1^NA)
print(NA_integer_^0)
print(c(NA, 1)^0)
print((-1)^Inf)
print((-1)^-Inf)
print((-2)^Inf)
print((-Inf)^0.5)
print((-Inf)^-0.5)
print((-Inf)^3)
print((-Inf)^2)
print(Inf^0)
print(0^-1)
print((1:3)^0)
#==#
print(as.integer(2^31))
print(as.integer(-2^31))
print(as.integer(2147483647))
print(as.integer(1e10))
print(as.integer(c(2^31, 5, -2^31)))
print(as.integer(NaN))
print(as.integer(Inf))
print(as.integer("2147483648"))
print(as.vector(2^31, "integer"))
print(as.vector(1.5, "character"))
print(as.vector("3", "integer"))
#==#
print(as.logical(c("T", "F", "TRUE", "FALSE")))
print(as.logical(c("true", "false", "True", "False")))
print(as.logical(c("Tr", "yes", "t")))
#==#
m <- matrix(1:9, 3)
print(m[cbind(1:3, 1:3)])
print(m[cbind(c(1, 2), c(2, 3))])
print(m[cbind(c(1, NA), c(1, 1))])
print(m[cbind(c(0, 1), c(1, 1))])
m[cbind(c(1, 2), c(1, 2))] <- 0L
print(m)
a <- array(1:24, c(2, 3, 4))
print(a[cbind(1, 2, 3)])
print(a[cbind(c(1, 2), c(2, 3), c(3, 4))])
mn <- matrix(1:6, 2, dimnames = list(c("a", "b"), c("x", "y", "z")))
print(mn[cbind("a", "y")])
#==#
print(nchar(c(a = "xx", b = "yyy")))
print(toupper(c(a = "xx")))
print(tolower(c(a = "XX")))
print(trimws(c(a = " x ")))
print(is.na(c(a = 1, b = NA)))
print(is.nan(c(a = NaN)))
print(is.finite(c(a = 1, b = Inf)))
print(is.infinite(c(a = 1, b = Inf)))
print(substr(c(a = "abcdef"), 2, 4))
print(substring(c(a = "abcdef"), 2, 4))
print(gsub("a", "z", c(a = "abc")))
print(sub("a", "z", c(a = "aac")))
print(round(c(a = 1.55, b = 2.45), 1))
print(signif(c(a = 123.456), 2))
print(log(c(a = 1, b = exp(1))))
print(!c(a = TRUE, b = FALSE))
print(-c(a = 1, b = 2))
print(chartr("x", "z", c(a = "xx")))
print(casefold(c(a = "xx"), upper = TRUE))
print(cumsum(c(a = 1, b = 2)))
print(cumprod(c(a = 1, b = 2)))
print(cummax(c(a = 1, b = 3)))
print(rank(c(a = 3, b = 1)))
print(format(c(a = 1.5)))
print(formatC(c(a = 1.5), format = "f", digits = 1))
print(vapply(c(a = 1, b = 2), function(x) x * 2, numeric(1)))
print(mapply(function(x, y) x + y, c(a = 1, b = 2), c(3, 4)))
#==#
m <- matrix(c(1, -2, NA, 4), 2)
print(dim(is.na(m)))
print(dim(round(m)))
print(dim(-m))
print(dim(!(m > 0)))
print(dim(log(abs(m))))
print(dim(is.finite(m)))
s <- matrix(c("a", "bb", "ccc", "d"), 2)
print(nchar(s))
print(toupper(s))
print(gsub("a", "z", s))
#==#
dm <- matrix(1:4, 2, dimnames = list(c("r1", "r2"), c("c1", "c2")))
print(dimnames(dm + 1))
print(dimnames(-dm))
print(dimnames(is.na(dm)))
print(dimnames(dm > 2))
print(dm * 2)
print(t(dm))
#==#
print(outer(c(a = 1, b = 2), c(x = 1, y = 2)))
print(outer(1:2, 1:3))
#==#
print(formatC(3.14159))
print(formatC(3.14159, format = "f"))
print(formatC(3.14159, width = 10, format = "f"))
print(formatC(1L, format = "f"))
print(formatC(3.14159, format = "E"))
print(formatC(3.14159, digits = -1, format = "f"))
print(formatC(pi, 3))
print(formatC(pi, 3, 10))
print(formatC(3.9, format = "d"))
#==#
print(formatC(3.14159, format = "fg"))
print(formatC(0.000012345, format = "fg"))
print(formatC(123456, format = "fg"))
print(formatC(1.5, format = "fg", digits = 8))
print(formatC(0.5, format = "fg", flag = "#"))
print(formatC(c(1.5, NA, Inf), format = "f"))
print(formatC(-Inf, format = "f"))
print(formatC(NaN))
print(formatC(12345.678, format = "f", digits = 2, big.mark = ","))
print(formatC(c(1, 1234567), format = "d", width = 10, big.mark = ","))
#==#
print(sprintf("%f", Inf))
print(sprintf("%f", -Inf))
print(sprintf("%e", Inf))
print(sprintf("%g", -Inf))
print(sprintf("%E", NaN))
print(sprintf("[%010.2f]", Inf))
print(sprintf("[%-10.2f]", Inf))
print(sprintf("[%+.2f]", Inf))
print(sprintf("[%010.2f]", NaN))
#==#
print(seq(0.1, 3, by = 0.1))
print(length(seq(0.1, 3, by = 0.1)))
print(seq(0, 1, by = 0.1))
print(seq(0.3, 0.9, by = 0.1))
print(seq(1e-3, 1e-2, by = 1e-3))
print(seq(0, 2, by = 1/3))
print(seq(10, 1, by = -2))
#==#
print(format(c("a", "bb"), justify = "none"))
print(format(c("a", "bb"), justify = "none", width = 5))
print(format(c("a", "bbb"), justify = "centre"))
print(format(c("a", "bb"), justify = "right", width = 5))
print(format(c("ab", "c"), justify = "centre", width = 6))
print(format(1:2, justify = "right"))
#==#
print(exists("pi"))
print(exists("sum"))
print(exists("letters"))
print(exists("month.name"))
print(exists("no_such_object_zz"))
x <- 1
print(exists("x"))
#==#
x <- c(1, 2, 3)
y <- x
x[1] <- 9
print(y)
print(x)
f <- function(a) { a[1] <- 99; a }
print(f(x))
print(x)
L <- list(a = x, b = x)
L$a[1] <- 0
print(L$a)
print(L$b)
print(x)
#==#
mk <- function() { v <- c(1, 2, 3); function(i, val) { v[i] <<- val; v } }
g <- mk()
print(g(1, 9))
print(g(2, 8))
e <- new.env()
e$v <- c(1, 2, 3)
u <- e$v
e$v[1] <- 7
print(u)
print(e$v)
#==#
x <- matrix(1:4, 2)
x[5] <- 9L
print(x)
print(attributes(x))
y <- array(1:4, c(2, 2), dimnames = list(c("r1", "r2"), c("c1", "c2")))
y[[5]] <- 9L
print(y)
print(attributes(y))
z <- 1:3
dim(z) <- 3L
z[5] <- 9L
print(attributes(z))
#==#
x <- c(a = 1, b = 2)
x[4] <- 9
print(x)
print(names(x))
y <- c(1, 2)
y["z"] <- 9
print(y)
print(names(y))
l <- list(a = 1)
l[[3]] <- 2
print(names(l))
print(l)
#==#
x <- numeric(0)
for (i in 1:6) x[i] <- i * 2
print(x)
l <- list()
for (i in 1:4) l[[i]] <- i
print(l)
y <- c(1, 2)
y[5] <- 9
print(y)
z <- character(0)
z[3] <- "c"
print(z)
#==#
print(suppressWarnings(as.integer("x")))
print(tryCatch(as.numeric("x"), warning = function(w) conditionMessage(w)))
f <- function(v) tryCatch(sqrt(v), warning = function(w) conditionMessage(w))
print(f(-1))
g <- function() withCallingHandlers(warning("w"), warning = function(w) {
    cat("saw:", conditionMessage(w), "\n")
    invokeRestart("muffleWarning")
})
g()
print(pmin(c(1, 2, 3), c(1, 2)))
print(pmax(c(1, 2, 3), c(1, 2)))
print(log(-1))
print(log(c(-1, 1)))
#==#
print(matrix(1:60, 3, 20))
print(matrix(1:200, 10, 20))
#==#
print(matrix(paste0("value", 1:20), 2, 10))
print(matrix(1:60, 20, 3))
m <- matrix(1:40, 2, 20, dimnames = list(c("alpha", "beta"), paste0("col", 1:20)))
print(m)
#==#
str(factor(c("a", "b")))
str(structure(1:2, class = "foo"))
str(1:5)
str(list(a = 1, b = "x"))
#==#
print(quote(f(1)))
print(class(quote(f(1))))
print(typeof(quote(f(1))))
print(mode(quote(f(1))))
print(quote(x))
print(class(quote(x)))
print(mode(quote(x)))
print(quote(1))
#==#
print(quote(a + b))
print(quote(!a))
print(quote(if (a) b else c))
print(quote({
    a
    b
}))
print(quote(x[[1]]))
print(quote(x$y))
print(quote(a <- b))
#==#
print(deparse(quote(f(1, 2))))
print(as.character(quote(x)))
print(as.character(quote(f(1))))
print(length(quote(f(1, 2))))
print(length(quote(if (a) b else c)))
print(as.name("x"))
print(is.call(quote(f(1))))
print(is.name(quote(x)))
print(format(quote(f(1))))
#==#
f <- function(x) sys.call()
print(f(1))
g <- function() f(99)
print(g())
h <- function() print(sys.call())
h()
k <- function() deparse(sys.call())
print(k())
p <- function() g(sys.call())
q <- function(z) z
r <- function() q(sys.call())
print(r())
#==#
f <- function(x, y) match.call()
print(f(1, y = 2))
print(f(y = 2, 1))
g <- function(a, b, ...) match.call()
print(g(1, 2, 3, k = 4))
h <- function(alpha, beta) match.call()
print(h(al = 1, be = 2))
k <- function(x, y = 2) match.call()
print(k(1))
#==#
print(as.list(quote(a + b)))
print(as.list(quote(f(a = 1, 2))))
print(names(as.list(quote(f(1, 2)))))
print(as.name("+"))
print(deparse(as.name("+")))
print(quote(`my var` + 1))
print(quote(f(1, 2))[[1]])
print(quote(a + b)[[1]])
#==#
f <- function() stop("boom")
print(tryCatch(f(), error = function(e) conditionCall(e)))
print(tryCatch(f(), error = function(e) e))
r <- try(f(), silent = TRUE)
cat(r)
s <- try(stop("plain"), silent = TRUE)
cat(s)
#==#
g <- function(x) warning("w")
print(tryCatch(g(1), warning = function(w) conditionCall(w)))
print(tryCatch(g(1), warning = function(w) w))
h <- function(a) sqrt(a)
print(tryCatch(h("x"), error = function(e) conditionCall(e)))
print(conditionCall(simpleError("m")))
print(simpleError("m"))
#==#
x <- table(c(1, 2, 1))
str(x)
str(c(a = 1, b = 2))
str(list(a = 1, b = "x"))
print(dimnames(x))
print(names(x))
#==#
print(eval(quote(1 + 2)))
x <- 5
print(eval(as.name("x")))
f <- function() {
    v <- 1
    eval(quote(v + 1))
}
print(f())
g <- function() {
    eval(quote(w <- 7))
    w
}
print(g())
eval(quote(cat("hi\n")))
print(eval(quote(if (TRUE) "y" else "n")))
print(eval(quote({
    a <- 1
    a + 1
})))
q <- quote(x)
print(eval(q))
print(class(eval(quote(quote(f(1))))))
#==#
l <- list(a = 1:3, b = letters[1:2])
print(rapply(l, length, how = "unlist"))
print(rapply(l, length, how = "list"))
print(rapply(list(1, 2), function(x) x * 2))
f <- function() {
    on.exit(cat("x\n"), add = TRUE)
    on.exit(cat("y\n"), add = TRUE, after = FALSE)
    1
}
invisible(f())
g <- function() {
    on.exit(cat("a\n"))
    on.exit(cat("b\n"), add = TRUE)
    1
}
invisible(g())
#==#
e <- new.env()
assign("v", 9, envir = e)
print(exists("v"))
print(eval(quote(v), e))
assign("v", 1, envir = e)
eval(quote(v <- v + 1), e)
print(get("v", envir = e))
print(eval(quote(1 + 1), e))
x <- 1
f <- function() {
    x <- 2
    eval(quote(x))
}
print(f())
#==#
f <- function(x) substitute(x)
print(f(a + b))
print(class(f(a + b)))
print(f(1))
print(class(f(1)))
g <- function(x) deparse(substitute(x))
print(g(a + b))
print(substitute(a + b))
y <- 9
print(substitute(y + 1))
#==#
f <- function(x) {
    y <- 2
    substitute(x + y)
}
print(f(a))
g <- function(x) substitute(x + z)
print(g(a))
h <- function(x, y) substitute(list(x, y))
print(h(a, b))
k <- function(...) substitute(list(...))
print(k(a, b))
print(substitute(x, list(x = 1)))
m <- function(x = 5) substitute(x)
print(m())
n <- function(a, b) substitute(a + b)
print(n(b = 1, a = 2))
#==#
f <- function() parent.frame()
g <- function() {
    z <- 1
    f()
}
e <- g()
print(class(e))
print(ls(e))
h <- function() {
    p <- parent.frame()
    get("z", envir = p)
}
k <- function() {
    z <- 42
    h()
}
print(k())
#==#
e <- new.env()
e$b <- 1
e$a <- 2
print(ls(e))
assign("v", 9, envir = e)
print(exists("v"))
print(exists("v", envir = e))
print(get("v", envir = e))
e$.hidden <- 1
print(ls(e))
print(ls(e, all.names = TRUE))
print(environmentName(globalenv()))
#==#
print(regexec("(a)(b)", "zab"))
print(regmatches("zab", regexec("(a)(b)", "zab")))
print(regmatches("a1b22", gregexpr("[0-9]+", "a1b22")))
print(environment(sum))
print(environmentName(environment(sum)))
f <- function() 1
print(class(environment(f)))
g <- function() {
    x <- 1
    function() x
}
h <- g()
print(ls(environment(h)))
#==#
print(head(matrix(1:20, 4), 2))
print(tail(matrix(1:20, 4), 2))
m <- matrix(1:20, 4, dimnames = list(letters[1:4], NULL))
print(head(m, 2))
print(tail(m, -1))
n <- matrix(1:6, 3, dimnames = list(NULL, c("a", "b")))
print(tail(n, 2))
print(head(1:10, -7))
print(Map(function(k, v) paste0(k, v), c("a", "b"), 1:2))
print(mapply(function(x, y) x + y, c(a = 1, b = 2), c(3, 4)))
#==#
print(max(character(0)))
print(min(character(0)))
print(range(character(0)))
print(max(character(0), na.rm = TRUE))
print(suppressWarnings(max(character(0))))
print(max(c("a", "b")))
print(range(c("b", "a")))
#==#
f <- tempfile()
cat("x,y\n1,2\n", file = f)
print(readLines(f))
print(file.exists(f))
cat("3,4\n", file = f, append = TRUE)
print(readLines(f))
unlink(f)
print(file.exists(f))
cat("a", "b", sep = "-")
cat("\n")
#==#
# Recycling: the shorter operand repeats to the longer length, and the rule is
# per-element rather than per-vector, so a logical index recycles the same way.
print(c(1, 2, 3, 4) + c(10, 20))
print(c(1, 2, 3) * c(1, 2, 3, 4, 5, 6))
print(1:6 + 1:3)
print(c(TRUE, FALSE, TRUE, FALSE) & c(TRUE, TRUE))
print(paste(c("a", "b", "c"), 1:3, sep = "-"))
print(c(1, 2, 3, 4, 5)[c(TRUE, FALSE)])
#==#
# NA is typed. The bare NA is logical, and each vector's NA takes the type of
# the vector it lands in — which is what keeps class() stable under coercion.
print(class(NA))
print(class(NA_integer_))
print(class(NA_character_))
print(class(NA_real_))
print(c(1, NA, 3))
print(class(c(1L, NA)))
print(NA > 1)
print(NA & FALSE)
print(NA | TRUE)
print(sum(c(1, NA, 3)))
print(sum(c(1, NA, 3), na.rm = TRUE))
print(is.na(c(1, NA, NaN)))
print(is.nan(c(1, NA, NaN)))
print(NA_character_)
print(c("a", NA))
#==#
# Integer arithmetic: overflow yields NA rather than wrapping, integer division
# floors toward negative infinity, and %% takes the sign of the right operand.
print(.Machine$integer.max)
print(5L %/% 2L)
print(-5L %/% 2L)
print(5 %% -2)
print(-5 %% 2)
print(class(5L / 2L))
print(class(5L %/% 2L))
print(1:3 * 2L)
print(class(1:3 * 2L))
#==#
# missing() asks whether the CALLER supplied an argument, not whether the name
# is bound: a formal with a default is bound before the body runs and is still
# missing. R accepts the symbol and the quoted spelling alike.
f <- function(a, b) missing(b)
print(f(1))
print(f(1, 2))
g <- function(a, b = 5) missing(b)
print(g(1))
print(g(1, 2))
print((function(a, b) missing("b"))(1))
h <- function(a, b) if (missing(b)) "no b" else paste("b =", b)
print(h(1))
print(h(1, 2))
k <- function(a = 1, b = 2, c = 3) c(missing(a), missing(b), missing(c))
print(k())
print(k(b = 9))
fwd <- function(...) f(...)
print(fwd(1))
print(fwd(1, 2))
outer <- function(x, y) {
  inner <- function(p, q) missing(q)
  c(missing(y), inner(1))
}
print(outer(1))
#==#
# Matrix subscripting drops to a vector unless drop = FALSE keeps the dimension.
m <- matrix(1:6, nrow = 2)
print(m)
print(dim(m))
print(m[1, ])
print(m[, 1])
print(m[1, , drop = FALSE])
print(dim(m[1, , drop = FALSE]))
print(t(m))
print(m[m > 3])
#==#
# identical() is exact where == is not: it separates integer from double and
# sees names. Sorting carries names along and order() returns positions.
print(identical(1L, 1))
print(identical(1, 1))
print(identical(c(a = 1), c(1)))
print(identical(list(1, 2), list(1, 2)))
print(identical(NULL, NULL))
print(identical("a", "a"))
print(all.equal(1, 1 + 1e-10))
print(isTRUE(all.equal(1, 1.1)))
x <- c(b = 2, a = 1, c = 3)
print(sort(x))
print(order(x))
print(rev(x))
print(names(sort(x)))
print(rank(c(10, 20, 10, 30)))
#==#
# nargs() counts what the caller passed, not what the formals are: a default
# fills in without counting, and ... contributes one per element.
f <- function(x, ...) nargs()
print(f(1))
print(f(1, 2, 3))
g <- function(a, b, c) nargs()
print(g(1, 2))
print(g(1, 2, 3))
print(g())
h <- function(a = 1) nargs()
print(h())
print(h(9))
print((function(a, b) nargs())(b = 2))
d <- function(...) nargs()
print(d())
print(d(1, 2, 3, 4))
#==#
# A replacement function whose result is itself indexed. R unrolls
# `names(x)[1] <- "z"` into `x <- \`names<-\`(x, \`[<-\`(names(x), 1, "z"))`, so
# the inner assignment builds the new attribute and the outer one puts it back.
x <- c(a = 1, b = 2)
names(x)[1] <- "z"
print(x)
y <- c(a = 1, b = 2, c = 3)
names(y)[2:3] <- c("y", "w")
print(y)
print(names(y))
# The `[[` form of the same thing.
z <- c(a = 1, b = 2)
names(z)[[1]] <- "q"
print(names(z))
# It works through other replacement functions too.
v <- 1:3
attr(v, "k") <- c(1, 2)
attr(v, "k")[1] <- 9
print(attr(v, "k"))
l <- list(a = 1, b = 2)
names(l)[1] <- "z"
print(names(l))
f <- factor(c("a", "b"))
levels(f)[1] <- "z"
print(levels(f))
# The plain, un-indexed forms still assign the whole thing.
w <- c(a = 1, b = 2)
names(w) <- c("p", "q")
print(w)
names(w) <- NULL
print(w)
# And a plain `levels<-` replaces every level, which is what the indexed form
# above is built on.
g <- factor(c("a", "b", "a"))
levels(g) <- c("x", "y")
print(g)
print(as.integer(g))
#==#
# deparse / dput of values: names written inline (quoted when not syntactic),
# `names =` as an attribute where inline names cannot go (an `m:n` run, an NA
# name), `structure()` for every other attribute, typed NAs only when all are
# NA, doubles at 15 significant digits, and R's two wrap rules — a vector's
# continuation flush left, a list's indented.
print(deparse(list(a = 1:3, b = "z")))
print(deparse(c(a = 1.5, b = 2)))
print(deparse(c(`a b` = 1, 2)))
print(deparse(c(a = 1L, b = 2L)))
print(deparse(setNames(1:3, c("a", "", NA))))
print(deparse(matrix(c(1.5, 2, 3, 4), 2, dimnames = list(c("r1", "r2"), NULL))))
print(deparse(factor(c("lo", "hi", "lo"))))
print(deparse(structure(1:3, myattr = "q", `my at` = 2)))
print(deparse(c(NA_real_, NA_real_)))
print(deparse(c(1, NA)))
print(deparse(c(NA_integer_)))
print(deparse(c("a", NA)))
print(deparse(NA_character_))
print(deparse(character(0)))
print(deparse(list()))
print(deparse(list(NULL, a = list(), list(1, "q"))))
print(deparse(c(1/3, 1e5, 123456, 1e-20, Inf, -Inf, NaN, 0.1 + 0.2)))
print(deparse(as.numeric(1:30)))
print(deparse(as.list(1:25)))
print(deparse(letters))
print(deparse(as.numeric(1:30), width.cutoff = 20L))
print(deparse(3:1))
print(deparse(-1:0))
dput(list(a = 1:3, b = "z"))
dput(c(x = 2.5))
y <- dput(1:3)
print(y)
#==#
# c() names: an untagged element with no names of its own gets "", not NA.
print(names(c(a = 1, 2)))
print(names(c(1, b = 2)))
print(names(c(1, 2)))
print(names(c(setNames(1:2, c("x", NA)), 3)))
print(c(a = 1, 2, c = 3))
#==#
# identical() on environments compares the reference, not the handle.
print(identical(globalenv(), globalenv()))
e1 <- new.env()
e2 <- e1
print(identical(e1, e2))
print(identical(e1, new.env()))
print(identical(list(globalenv()), list(globalenv())))
f <- function() environment()
print(identical(f(), f()))
#==#
# A forced promise runs in the environment its expression was written in, so
# environment() inside an argument is the writer's frame, and parent.frame(n)
# walks calling generations — through promise frames — not stack positions.
print(environmentName(environment()))
print(identical(environment(), globalenv()))
f <- function() parent.frame()
print(identical(f(), globalenv()))
g <- function() f()
print(identical(g(), globalenv()))
h <- function() { e <- environment(); k <- function() parent.frame(); identical(k(), e) }
print(h())
id <- function(v) v
p2 <- function() parent.frame(2)
q2 <- function() id(p2())
r2 <- function() { tag <- "r2"; e <- q2(); exists("tag", envir = e, inherits = FALSE) }
print(r2())
s2 <- function() { tag <- "s2"; id(id(q2())) }
print(exists("tag", envir = s2(), inherits = FALSE))
print(identical(id(parent.frame()), globalenv()))
v3 <- function() parent.frame(3)
print(identical(id(id(v3())), globalenv()))
w <- function() { x <- 1; v <- function() get("x", envir = parent.frame()); v() }
print(w())
k2 <- function() { id(q <- 3); q }
print(k2())
#==#
# The global environment prints by name, alone, inside a list, and under str().
print(globalenv())
environment()
print(list(globalenv(), 2))
str(globalenv())
str(list(a = globalenv(), b = list(e = globalenv())))
#==#
# Operator results carry R's attributes: arithmetic copies every attribute of
# each same-length operand (the left one's win), comparison and logic only
# the shape; names come from the left operand when they fit, else the right.
x <- structure(1, class = "money")
print(unclass(x + 1))
print(attributes(x + 1))
print(attributes(structure(1:2, foo = "bar") * 2))
print(attributes(1 + structure(1:2, foo = "bar")))
print(attributes(structure(1:2, a = 1) + structure(3:4, b = 2)))
print(attributes(structure(1:2, a = 1) + structure(3:4, a = 2)))
print(attributes(structure(1:4, a = 1) + structure(3:4, b = 2)))
print(attributes(-structure(1:2, foo = "bar")))
print(attributes(structure(1:2, foo = "bar") == 1))
print(attributes(!structure(c(TRUE,FALSE), foo = "bar")))
print(attributes(!structure(c(1,0), foo = "bar")))
print(1:2 + c(a = 1, b = 2))
print(c(1, 2) == c(a = 1, b = 3))
print(c(TRUE, FALSE) & c(a = TRUE, b = TRUE))
print(1 + c(a = 1, b = 2))
print(c(x = 1, y = 2) + c(a = 1, b = 2))
m <- matrix(1:4, 2, dimnames = list(c("a","b"), c("p","q")))
print(m + 1); print(1:4 + m); print(m > 2); print(m & TRUE)
print(structure(1:3, units = "cm") * 2)
x + 1
#==#
# nargs(), missing(), Recall() and sys.function() written inside an argument
# answer for the closure that wrote it, however deep the promise is forced.
f <- function(a, b) paste(nargs())
print(f(1, 2))
g <- function(a, b = 2) c(missing(b), nargs())
print(g(1))
h <- function(a, b = 2) paste(missing(b))
print(h(1))
k <- function(n) if (n <= 1) 1 else n * identity(Recall(n - 1))
print(k(4))
s <- function() identity(sys.function())
print(is.function(s()))
outer_f <- function(a, b, c) identity(identity(nargs()))
print(outer_f(1, 2, 3))
#==#
# User-defined S3 group generics: operator-specific and Ops methods (with
# .Generic, unary calls, NextMethod to the internal operator, the right
# operand's method when the left has none), and Math / Summary methods.
x <- structure(1, class = "money")
"+.money" <- function(e1, e2) "plus"
print(x + x)
print(1 + x)
"==.money" <- function(e1, e2) "eq"
print(x == 1)
Ops.cash <- function(e1, e2) paste(.Generic, class(e1), nargs())
y <- structure(2, class = "cash")
print(y * 3)
print(3 - y)
print(-y)
print(!y)
Math.cash <- function(x, ...) paste("math", .Generic)
print(sqrt(y))
print(cumsum(y))
Summary.cash <- function(..., na.rm = FALSE) paste("sum", .Generic)
print(max(y))
print(range(y, 5))
Ops.temp <- function(e1, e2) {
  v <- get(.Generic)(unclass(e1), unclass(e2))
  if (.Generic %in% c("+", "-", "*", "/")) structure(v, class = "temp") else v
}
print.temp <- function(x, ...) cat(unclass(x), "degrees\n")
t1 <- structure(20, class = "temp")
print(t1 + 5)
t1 * 2
print(t1 > 10)
Ops.meters <- function(e1, e2) {
  r <- NextMethod()
  if (.Generic %in% c("<", ">", "==", "!=", "<=", ">=")) unclass(r) else r
}
m <- structure(c(1, 5), class = "meters")
print(unclass(m + 1))
print(class(m * 2))
print(m > 2)
format.meters <- function(x, ...) paste0(unclass(x), "m")
print.meters <- function(x, ...) cat(format(x), "\n")
m + 10
a <- structure(1, class = "A"); b <- structure(2, class = "B")
Ops.A <- function(e1, e2) "A"
Ops.B <- function(e1, e2) "B"
print(unclass(a + b))
print(b + 1)
print(1 + a)
Ops.vec <- function(e1, e2) { if (missing(e2)) paste("unary", .Generic) else paste("binary", .Generic) }
v <- structure(1, class = "vec")
print(-v)
print(v - 1)
print(sum(structure(1:3, class = "plain")))
#==#
# print(quote = FALSE) and noquote(): strings bare but still escaped, a missing
# string as <NA>, the layout otherwise unchanged.
print(noquote(c("a","bb",NA)))
print(c(x="a",y="bbb"), quote=FALSE)
print(c("a","bb",NA), quote=FALSE)
print(matrix(c("a","bb","c","d"),2), quote=FALSE)
print(letters, quote=FALSE)
print(c('say "hi"', "plain"), quote = FALSE)
x <- noquote(c(a = "x", b = "yy"))
x
print(class(x))
print(class(noquote(noquote("a"))))
print(noquote(matrix(c("p", NA, "r", "s"), 2)))
print(unclass(x))
print(c(TRUE, NA), quote = FALSE)
print(list("a", 1), quote = FALSE)
#==#
# A default argument is a promise: evaluated at first use, in the callee's
# frame, after the body may have rebound what it reads, and never if unread.
f <- function(a, b = a * 2) { a <- 10; b }
print(f(1))
g <- function(x, n = length(x)) { x <- c(x, 0); n }
print(g(1:3))
h <- function(a, b = stop("never")) a
print(h(5))
k <- function(n = nargs()) n
print(k())
m <- function(a, b = missing(a)) b
print(m())
p <- function(x, y = x + 1) { if (missing(y)) "defaulted" else y }
print(p(1))
q <- function(...) { r <- function(v = sum(...)) v; r() }
print(q(1, 2, 3))
s <- function(z = { cat("forced\n"); 7 }) { cat("before\n"); z + z }
print(s())
u <- function(a = b, b = 3) a
print(u())
w <- function(e = environment()) identical(e, environment())
print(w())
#==#
# A top-level name bound to a function and called by that name, in a unit
# with no closure (the native-slot path).
g <- sum
print(g(1, 2))
f <- Negate(is.null)
print(f(1))
h <- paste
print(do.call("h", list("a", "b")))
k <- max
for (i in 1:3) print(k(i, 2))
x <- 5
print(x + 1)
#==#
# The apply family, Reduce, Map and Filter take FUN as a string, as match.fun does.
print(sapply(1:3, "sum", 10))
print(lapply(1:2, "-"))
print(Map("+", 1:2, 3:4))
print(Reduce("+", 1:4))
print(vapply(1:2, "sqrt", numeric(1)))
print(mapply("rep", 1:2, 2:1))
print(apply(matrix(1:4,2), 1, "max"))
print(Filter("is.numeric", list(1,"a")))
#==#
print(sprintf("%2$s %1$s", "a", "b"))
print(sprintf("%1$d-%1$d", 3L))
print(sprintf("%#x %#X %#o %#x", 255L, 255L, 8L, 0L))
print(sprintf("%s", c(1/3, pi, 1e6, 0.1 + 0.2)))
print(sprintf("%s=%d", character(0), 1L))
print(r <- tryCatch(sprintf("%3$s", 1), error = function(e) "rejected"))
#==#
y <- 5
z <- quote(a + b)
print(bquote(f(.(y), k)))
print(bquote(.(z) * 2))
print(bquote(2^.(z)))
print(bquote(x - .(z)))
print(bquote(.(z) - x))
print(bquote(-.(z)))
print(bquote(.(quote(a^b))^c))
print(bquote(.(quote(!a)) + b))
print(bquote(.(quote(x <- 1)) + 1))
print(bquote(.(z)[1]))
f <- function(x, y) substitute(x * y)
print(f(a + b, c - d))
print(eval(bquote(.(y) * 2)))
#==#
e <- quote(a + !b == c)
print(e[[1]])
print(e)
print(quote(x * -!y))
print(1 + !0)
print(2 * !TRUE == FALSE)
x <- 3
print(x == !FALSE)
#==#
msg <- function(expr) tryCatch(expr, error = function(e) conditionMessage(e))
print(msg(list(1) + 1))
print(msg(sum + 1))
print(msg(-quote(a)))
print(msg(-"a"))
print(msg(!"a"))
print(msg(sum == 1))
print(NULL + 1)
print(msg(!NULL))
print(msg(-NULL))
print(quote(a) == "a")
print(quote(f(x)) == "f(x)")
print(list(1, "a") == c("1", "a"))
#==#
writeLines(c("alpha", "beta"))
writeLines(c("p", NA), sep = "|")
writeLines("")
print(file.path("a", c("b", "c"), "d.txt"))
print(file.path("a", character(0)))
print(basename(c("/a/b/", "a", "", "/", "a//b//", "x/y.tar.gz")))
print(dirname(c("/a/b/", "a", "", "/", "a//b//", "/a", "a/b/c")))
#==#
print(sQuote("x"))
print(dQuote(c("a", "b")))
print(sQuote("x", FALSE))
print(dQuote("y", q = FALSE))
print(shQuote(c("a", "b c")))
print(shQuote(c("it's", "$HOME")))
#==#
print(Sys.getenv("RLANG_PARITY_UNSET_VAR"))
print(Sys.getenv("RLANG_PARITY_UNSET_VAR", unset = NA))
print(Sys.getenv(c("RLANG_PARITY_U1", "RLANG_PARITY_U2"), unset = "-"))
print(Sys.setenv(RLANG_PARITY_SET = "on"))
print(Sys.getenv("RLANG_PARITY_SET"))
x <- 1; y <- "a"
print(mget(c("x", "y")))
e <- new.env(); assign("v", 2, envir = e)
print(mget("v", envir = e))
print(c(is.atomic(NULL), is.atomic(list()), is.atomic(1:2), is.atomic("a")))
m <- matrix(1:6, 2)
print(c(NROW(m), NCOL(m), NROW(1:5), NCOL(1:5), NROW(NULL), NCOL(NULL), NROW(list(1, 2))))
#==#
print(weighted.mean(c(1, 2, 3), c(3, 2, 1)))
print(weighted.mean(c(1, NA, 3), c(1, 1, 2), na.rm = TRUE))
print(weighted.mean(c(1, NA), c(1, 0)))
print(weighted.mean(c(1, NA), c(1, 1)))
print(weighted.mean(1:4))
print(prop.table(c(a = 1, b = 3)))
m <- matrix(1:4, 2, dimnames = list(c("r1", "r2"), c("A", "B")))
print(prop.table(m))
print(prop.table(m, 1))
print(proportions(m, 2))
#==#
print(sapply(c("a", "b"), toupper))
print(sapply(c("a", "b"), toupper, USE.NAMES = FALSE))
print(sapply(list(a = 1, b = 2), function(v) v * 10, USE.NAMES = FALSE))
print(sapply(1:2, function(i) i, simplify = FALSE))
print(vapply(1:3, function(x, y) x + y, numeric(1), y = 10))
print(vapply(c(a = "x", b = "yy"), nchar, integer(1), USE.NAMES = FALSE))
print(vapply(c("x", "yy"), nchar, FUN.VALUE = integer(1)))
#==#
print(regmatches("a-b-c", regexpr("-", "a-b-c"), invert = TRUE))
x <- c("a1b22c", "xyz", NA)
print(regmatches(x, gregexpr("[0-9]+", x), invert = TRUE))
print(regmatches("1a2", gregexpr("[0-9]", "1a2"), invert = TRUE))
#==#
print(as.character(list(1:2, "b", c(1.5, 2), TRUE, NULL, 3L, list(1))))
print(as.character(list(c(a = 1, b = 2), NA_integer_, c(1L, NA), 1/3, c(TRUE, NA))))
print(as.character(list(quote(x), quote(f(y)), c("a", "b"), character(0), c(`a b` = 1))))
print(paste(list(1:2, "b")))
print(paste("x", list(1, 2:3), sep = "_"))
#==#
cat(NA + NaN, NaN + NA, NaN * NA, NaN / NA, NaN ^ NA, NaN %/% NA, NaN %% NA, NA_integer_ + NaN, "\n")
print(c(NA, NaN, 1) - c(NaN, NA, NA))
print(sum(c(1, NaN, NA)))
print(sum(c(NaN, 1), c(NA, 2)))
print(sum(NaN, TRUE, NA))
print(typeof(sum(1L, NA)))
print(prod(c(NaN, 2), NA_real_))
print(prod(c(NA, NaN)))
print(mean(c(NaN, NA)))
print(mean(c(NaN, Inf, NA)))
#==#
sq <- \(x) x^2
print(sq(5))
print(sapply(1:3, \(i) i * 10))
print((\(a, b = 2) a + b)(1))
f <- \(x) x + 1
f
compose <- function(f, g) function(x) g(f(x))
inc <- function(x) x + 1
dbl <- function(x) x * 2
print(compose(inc, dbl)(3))
mk <- function(f) function(y) f(y)
print(mk(dbl)(5))
print(sapply(1:3, mk(inc)))
`%then%` <- function(f, g) function(x) g(f(x))
print((inc %then% dbl)(1))
#==#
x <- c(a = 3, b = NA, c = 5, d = 5)
print(which.max(x))
print(which.min(c(z = NaN, y = 2, w = 1)))
print(which.max(c(NA, NA)))
print(head(c(a = 1, b = 2, c = 3), 2))
print(tail(c(a = 1, b = 2, c = 3), -2))
#==#
t <- table(c(1, 1, 2, 3))
print(t[2:1])
print(t[-1])
print(t["2"])
print(rev(t))
print(sort(t))
print(sort(t, decreasing = TRUE))
print(head(sort(table(c("x", "y", "y", "z", "z", "z")), decreasing = TRUE), 2))
print(round(t / 3, 2))
print(sqrt(t))
x <- array(1:3, 3, list(c("a", "b", "c")))
print(x[2:3])
print(x[2])
#==#
x <- structure(c(a = 1.234, b = -2), extra = "e")
print(round(x, 1))
print(signif(x, 1))
print(abs(structure(-1L, extra = 1)))
#==#
print(na.omit(c(1, 2, NA, 4)))
y <- na.omit(c(a = 1, b = NA, c = NaN, d = 3))
print(y)
print(attr(y, "na.action"))
print(na.omit(c("a", NA)))
print(na.omit(1:3))
print(na.omit(matrix(c(1, NA, 3, 4, 5, NA), 3)))
print(sum(na.omit(c(1, NA, 2))))
#==#
r <- tryCatch(vapply(1:2, function(i) 1:2, numeric(1)), error = function(e) conditionMessage(e))
print(r)
r <- tryCatch(vapply(1:2, function(i) "a", numeric(1)), error = function(e) conditionMessage(e))
print(r)
r <- tryCatch(vapply(1:2, function(i) 1.5, integer(1)), error = function(e) conditionMessage(e))
print(r)
print(vapply(1:2, function(i) TRUE, numeric(1)))
print(typeof(vapply(1:2, function(i) TRUE, integer(1))))
print(vapply(c(a = 1, b = 2), function(i) c(x = i, y = i * 2), numeric(2)))
#==#
print(ifelse(c(a = TRUE, b = FALSE), "y", "n"))
print(ifelse(matrix(c(TRUE, FALSE, NA, TRUE), 2), 1, 0))
print(ifelse(c(x = 1, y = 0) > 0, 10L, 2.5))
print(nchar(c("a", NA), keepNA = FALSE))
print(nchar(c("a", NA), keepNA = TRUE))
print(nchar(c("a", NA), type = "width"))
print(nchar(c("a", NA)))
#==#
print(sprintf("%#.3g", 1))
print(sprintf("%#g", 2.5))
print(sprintf("%#.0f", 3))
print(sprintf("%#.0e", 3))
print(sprintf("%#5.2g", 0.0001))
print(sprintf("%g", 9.9999999))
print(sprintf("%.3g", 99.96))
print(sprintf("%g", 0))
#==#
print(body(function(x) x + 1))
f <- function(x, y) { z <- x + y; z * 2 }
print(body(f))
print(class(body(f)))
print(body(function() 42))
print(is.null(body(sum)))
print(eval(body(function() 1 + 2)))
for (e in list(quote((1)), quote(if (a) b), quote(for (i in 1) 1), quote(while (TRUE) 1), quote(x <- 1), quote(x <<- 1), quote(f(1)))) print(class(e))
#==#
t2 <- table(c("a", "b", "a", "b"), c("x", "x", "y", "y"))
print(t2)
print(dim(t2))
print(t2["a", "y"])
g <- c("m", "f", "m", "m")
s <- c(TRUE, FALSE, TRUE, NA)
table(g, s)
table(g, survived = s)
z <- c(1, 2, 2)
f <- function(v) table(v)
f(z)
table(z)
print(t(t2))
print(t2 * 2)
print(names(dimnames(table(g, s))))
#==#
m <- matrix(1:4, 2, dimnames = list(r = c("a", "b"), c = c("x", "y")))
print(m)
m2 <- matrix(1:4, 2, dimnames = list(c("a", "b"), c("x", "y")))
names(dimnames(m2)) <- c("", "")
print(m2)
print(matrix(1:4, 2, dimnames = list(longname = c("a", "b"), NULL)))
#==#
tc <- function(expr) tryCatch(expr, error = function(e) paste("E:", conditionMessage(e)))
print(tc(if (c(TRUE, FALSE)) 1))
print(tc(if (logical(0)) 1))
print(tc(while (c(TRUE, TRUE)) break))
print(tc(if ("yes") 1))
print(tc(if (list(TRUE)) 1))
print(if ("TRUE") 1)
print(if (2) "two")
print(tc(c(TRUE, TRUE) && TRUE))
print(tc(TRUE && c(TRUE, FALSE)))
print(tc(FALSE && c(TRUE, FALSE)))
print(tc(logical(0) && TRUE))
print(tc("a" && TRUE))
print(tc(TRUE || "a"))
print(tc(FALSE || "a"))
print(tc(NULL || TRUE))
print(tc(1 && 2))
print(tc(0 || NA))
print(tc(NA && FALSE))
print(tc(factor("a") && TRUE))
#==#
x <- 5
f <- function(x) x
print(tryCatch(f(), error = function(e) conditionMessage(e)))
g <- function(a, b) if (missing(b)) "no b" else b
print(g(1))
h <- function(a, b) a
print(h(1))
#==#
tc <- function(expr) tryCatch(expr, error = function(e) paste("E:", conditionMessage(e)))
m <- matrix(1:4, 2, dimnames = list(c("a", "b"), c("x", "y")))
print(tc(m[3, 1]))
print(tc(m["z", 1]))
print(tc(m[, 3]))
print(tc(m[0, 1]))
print(tc(m[-3, 1]))
print(tc({ m[3, 1] <- 9L; m }))
print(tc(m[c(TRUE, FALSE, TRUE), 1]))
print(tc(array(1:8, c(2, 2, 2))[1, 1, 3]))
t2 <- table(c("a", "b", "a"), c("x", "x", "y"))
print(t2[1:2, ])
#==#
tc <- function(expr) tryCatch(expr, error = function(e) paste("E:", conditionMessage(e)), warning = function(w) paste("W:", conditionMessage(w)))
print(tc(sum("a")))
print(tc(sum(1, "a")))
print(tc(prod(list(1))))
print(tc(sum(NULL)))
print(tc(sum(factor("a"))))
print(tc(strsplit(1, "a")))
print(tc(rep(1:2, times = -1)))
print(tc(rep(1:2, each = -1)))
print(tc(rep(1:2, times = NA)))
print(tc(mean("a")))
print(tc(factorial(-1)))
print(tc(gamma(0)))
print(gamma(-1.5))
print(factorial(0:5))
print(tc(integer(-1)))
print(tc(seq_len(-1)))
print(tc(seq_len(NA)))
print(numeric(length = 3))
print(character(length = 2))
print(vector("list", length = 1))
#==#
e <- new.env()
assign("a", 1, e)
print(get("a", e))
print(exists("a", e))
x <- 3
print(exists("x", envir = e))
print(exists("x", envir = e, inherits = FALSE))
f <- function(y) get("y")
print(f(4))
g <- function() { z <- 1; h <- function() exists("z", inherits = FALSE); h() }
print(g())
print(tryCatch(get("x", e, inherits = FALSE), error = function(err) conditionMessage(err)))
y <- 2
z <- 3
rm(x)
print(exists("x"))
rm("y", list = c("z"))
print(c(exists("y"), exists("z")))
f2 <- function() { a <- 1; rm(a); exists("a", inherits = FALSE) }
print(f2())
w <- 5
g2 <- function() rm(w)
suppressWarnings(g2())
print(exists("w"))
#==#
print(Reduce(`+`, 1:4, accumulate = TRUE, simplify = FALSE))
g <- function(...) ..1
print(g(5, 6))
g2 <- function(...) ..2
print(g2("a", "b"))
tc <- function(expr) tryCatch(expr, error = function(e) conditionMessage(e))
print(tc(g2(1)))
k <- function(a, ...) a + ..1
print(k(1, 10))
#==#
for (s in list(seq(1, 3), seq(1, 3, by = 1), seq(1L, 3L, by = 1L), seq(1L, 3L, by = 1), seq(2, 11, 3), seq(5), seq(1, 10, length.out = 4), seq(1L, 10L, length.out = 4L), seq(1L, 10L, length.out = 4), seq(length.out = 3), seq(2L, length.out = 3L), seq(2, length.out = 3), seq(10, 1), seq(1.5, 4), seq(5, by = -2), seq(1L, 9L, by = 2L), seq(1L, 10L, length.out = 3L), seq(to = 5, length.out = 3), seq(1, 1, length.out = 3))) cat(typeof(s), ":", s, "\n")
#==#
m <- matrix(1:12, 3)
print(upper.tri(m))
print(lower.tri(m, diag = TRUE))
print(m[upper.tri(m)])
m2 <- matrix(1:9, 3)
m2[lower.tri(m2)] <- 0
print(m2)
print(lower.tri(1:3))
#==#
print(mapply(function(a, b) a:b, 1:2, 3:4, SIMPLIFY = FALSE))
print(mapply(rep, times = 1:3, x = 3:1))
print(mapply(function(x, y) x + y, 1:3, MoreArgs = list(y = 10)))
print(mapply(function(a, b) paste(a, b), c(p = "x", q = "y"), "z"))
print(mapply(function(a, b) a * b, 1:4, 1:2))
print(Map(`+`, 1:4, 1:2))
print(mapply(function(...) sum(...), 1:2, 3:4, USE.NAMES = FALSE))
print(mapply(function(a) a, character(0)))
#==#
print(rank(c(10, 20, 10, 30, NA, 10), ties.method = "first"))
print(rank(c(10, 20, 10, 30, NA, 10), ties.method = "last"))
print(rank(c(10, 20, 10, 30, NA, 10), ties.method = "min"))
print(rank(c(b = 2, a = 1, c = 2), ties.method = "max"))
print(rank(c(10, 20, 10)))
print(var(1:5, 5:1))
print(cov(c(1, 2, 3, 4), c(2, 4, 5, 9)))
print(var(c(1, NA), c(2, 3)))
print(cov(1:10, (1:10)^2))
#==#
x <- 1:5
print(replace(x, 2, 99L))
print(x)
print(replace(c(1, NA), is.na(c(1, NA)), 0))
print(replace(c(a = 1, b = 2), "b", 5))
print(replace(letters[1:3], 2, 1))
print(append(c(a = 1, b = 2), c(z = 9), after = 1))
#==#
x <- c(1, 2, NA, 4, 5, 6)
g <- c("a", "b", "a", "b", "a", "c")
r <- tapply(x, g, mean)
print(r)
print(names(r))
print(dim(r))
print(tapply(x, g, mean, na.rm = TRUE))
print(tapply(x, g, function(v) v * 2))
print(sort(tapply(c(3, 1, 2), c("p", "q", "r"), sum)))
print(tapply(c(10, 20, 30, 40), list(c("a", "a", "b", "b"), c("x", "y", "x", "y")), sum))
print(tapply(1:4, list(g1 = c(1, 1, 2, 2), g2 = c("u", "v", "u", "u")), sum))
print(tapply(1:3, factor(c("a", "a", "b"), levels = c("a", "b", "z")), sum))
print(tapply(1:4, c(1, NA, 2, 2), length))
print(r[["a"]])
#==#
print(strtoi(c("10", "077", "0xff", "zz", " 7", "", "-12", "7 ", "0x", NA)))
print(strtoi("ff", 16L))
print(strtoi("777", 8L))
print(strtoi("11", 2))
print(strtoi("99999999999"))
r <- tryCatch(strtoi("1", 1), error = conditionMessage)
print(r)
#==#
f <- function(x) { if (x > 0) { y <- 1; y + 2 } else { 3 } }
print(tryCatch(f(1:2), error = function(e) conditionCall(e)))
print(tryCatch(if (NA) 1, error = function(e) conditionCall(e)))
print(tryCatch(while (c(TRUE, FALSE)) 1, error = function(e) conditionCall(e)))
cat(try(if (logical(0)) 1, silent = TRUE))
#==#
m <- matrix(list(1, "a", TRUE, NULL, 1:3, c("x", "y"), list(1, 2), factor("a"),
                 2.5, NA, sum, quote(x + y), c(1.5, 2), 3L, NA_character_, -1), 4)
print(m)
print(matrix(list(1, "a"), 1, dimnames = list("r", c("A", "B"))))
print(array(list(1, 2, 3, 4, 5, 6, 7, 8), c(2, 2, 2)))
print(matrix(list(factor(c("a", "b")), factor(NA), 1e10, 123456.789), 2))
print(matrix(list(), 0, 2))
#==#
x <- NULL; x$a <- 1; str(x)
x <- NULL; x$a <- NULL; print(x)
x <- NULL; x[["a"]] <- "s"; str(x)
x <- NULL; x[[2]] <- 1; print(x)
x <- NULL; x[[1]] <- NULL; print(x)
x <- NULL; x["a"] <- 1; print(x)
x <- list(); x$a$b$c <- 1; str(x)
y <- suppressWarnings({ y <- c(b = 1); y$a <- 2; y })
print(y)
#==#
print(sapply(character(0), nchar))
print(sapply(c("a", "bb"), nchar))
print(sapply(c("a", "bb"), nchar, USE.NAMES = FALSE))
print(sapply(c(x = "a", y = "bb"), nchar))
print(setNames(integer(0), character(0)))
print(setNames(list(1), "z"))
#==#
print(is.environment(globalenv()))
print(is.environment(list()))
print(is.character(getwd()))
#==#
str(c(a = 1))
str(structure(1, class = "foo"))
str(structure(c(a = 1), class = "foo"))
str(structure(1:2, class = c("integer", "x")))
str(structure(1:3, class = c("foo", "bar")))
x <- c(a = 1, b = 2); attr(x, "z") <- "q"; str(x)
str(structure(list(1), foo = "y"))
str(structure(list(1), class = "foo"))
str(matrix(list(1, 2), 1))
str(list(a = structure(1:2, u = "v")))
#==#
print(inherits(structure(1, class = c("a", "b")), "b", which = TRUE))
print(inherits(structure(1, class = c("a", "b")), c("z", "a", "b"), TRUE))
print(inherits(1, "numeric", which = TRUE))
print(oldClass(1))
print(oldClass(structure(1, class = c("p", "q"))))
print(oldClass(factor("a")))
#==#
print(make.names(c(NA, "", "if", "TRUE", ".1a", "._", "a_b", "a-b", "...", "..1", "in", "NA_real_", "x")))
print(make.names(c("a", "a", "a.1"), unique = TRUE))
print(make.names("a_b", allow_ = FALSE))
print(make.unique(c("a", "a", "a.1", "a", NA, NA), sep = "_"))
print(make.unique(c("b", "b", "b.1", "b")))
print(make.unique(c("a", "a", "b", "a")))
#==#
print(encodeString(c("a", NA), quote = "\""))
print(encodeString("a\"b", quote = "\""))
print(encodeString("x", width = 5, justify = "right"))
print(encodeString(NA))
print(encodeString(NA, na.encode = FALSE))
print(encodeString(c("a", "bbb", NA), width = NA))
print(encodeString(c("a", "bb"), width = NA, justify = "centre"))
print(encodeString("a", width = 4, justify = "c"))
writeLines(encodeString(c("a\"b", "\a\b\f\v\001", "a\tb\n")))
writeLines(encodeString("a\"'b", quote = "'"))
print(encodeString(c("a", NA), width = NA, quote = "\""))
print(encodeString(c("a", NA), width = NA, na.encode = FALSE))
#==#
print(colSums(matrix(c(1, NA, 3, 4), 2), na.rm = TRUE))
print(colSums(matrix(c(1, NA, 3, 4), 2)))
print(rowMeans(matrix(c(1, NA, 3, 4), 2), na.rm = TRUE))
print(rowMeans(matrix(c(NA, NA, 3, 4), 2, byrow = TRUE), na.rm = TRUE))
print(colMeans(matrix(c(1, NaN, 3, 4), 2), na.rm = TRUE))
print(rowSums(matrix(1:6, 2)))
#==#
print(table(c(NA, 1, 1), useNA = "ifany"))
print(table(c(2, 1, 1), useNA = "always"))
print(table(c("a", NA, "b", NA), useNA = "no"))
print(table(c(1, NA), c("x", "y"), useNA = "ifany"))
t <- table(c(NA, "q"), useNA = "a")
print(names(t))
#==#
m <- matrix(1:4, 2, dimnames = list(c("a", NA), c(NA, "z")))
print(m)
#==#
print(formatC(TRUE, format = "d"))
print(formatC(c(TRUE, NA), width = 3, format = "d"))
print(tryCatch(formatC(TRUE), error = conditionMessage))
print(tryCatch(formatC(FALSE, format = "f"), error = conditionMessage))
#==#
print(format(list(1, "a", TRUE)))
print(format(list(1:3, c(1.5, 2), "a", NULL, list(1, "b"), c(a = 1))))
print(format(list(1, 22), width = 5))
print(format(list(a = 1, b = "x")))
print(format(list(c("a", "bbb"))))
print(format(list(c(1, 10), 3), nsmall = 2))
#==#
print(format(c(1, 22), trim = TRUE))
print(format(c(1, 22), trim = TRUE, width = 5))
print(format(c(TRUE, FALSE), trim = TRUE))
print(format(c("a", "bb"), trim = TRUE))
print(format(c(1.5, 10), trim = TRUE))
print(format(c(-1, 10)))
#==#
print(1:3 %*% 1:3)
print(2 %*% 1:3)
print(1:3 %*% matrix(1:3, 1))
print(matrix(1:2, 2) %*% 1:3)
print(crossprod(1:3))
print(crossprod(2, 1:3))
print(tcrossprod(1:2))
r <- try(matrix(1:4, 2) %*% 1:3, silent = TRUE)
cat(r)
r <- try(crossprod(matrix(1:6, 3), 1:2), silent = TRUE)
cat(r)
#==#
m <- matrix(1:4, 2, dimnames = list(r = c("a", "b"), c = c("x", "y")))
print(m %*% m)
print(crossprod(m))
print(tcrossprod(m))
print(m %*% 1:2)
print(1:2 %*% m)
u <- matrix(1:4, 2, dimnames = list(c("a", "b"), NULL))
print(u %*% u)
print(crossprod(u))
#==#
`%+%` <- function(a, b) stop("no")
r <- try(1 %+% 2, silent = TRUE)
cat(r)
`%s%` <- function(a, b) sys.call()
print(1 %s% 2)
`%w%` <- function(a, b) { warning("careful"); a + b }
withCallingHandlers(print(1 %w% 2), warning = function(w) { print(conditionCall(w)); invokeRestart("muffleWarning") })
#==#
x <- c(3, 1, 4, 1, 5, 9, 2, 6)
for (t in 1:9) print(quantile(x, c(0.1, 0.5, 0.9), type = t))
print(quantile(1:10, 1/3))
print(quantile(c(0.1, 0.7, 0.3), 0.33))
print(quantile(1:5, c(0.1, NA)))
print(quantile(numeric(0)))
print(quantile(x, 0.25, names = FALSE))
r <- try(quantile(c(1, NA)), silent = TRUE)
cat(r)
print(quantile(c(1, NA, 3), na.rm = TRUE))
print(quantile(factor(c("a", "b", "c"), ordered = TRUE), 0.5, type = 1))
#==#
print(round(0.15, 1))
print(round(2.675, 2))
print(round(-1.5))
print(round(2.5))
print(round(c(1.234, 5.678), 1:2))
print(round(123.456, -1))
print(round(1234.5678, 2.6))
print(round(c(a = 1.26), 1))
print(round(1:3 / 7, c(1, 2, 3)))
print(signif(123456, 2))
print(signif(0.0034219, 3))
print(signif(pi, 0))
print(signif(pi, 1:4))
print(signif(-2.5e-7, 1))
print(round(1e300 * 3.3, -299))
print(signif(1.09e308, 2))
#==#
print(fivenum(c(1, 2, 3, 4, 5, 6, 7, 8, 100)))
print(fivenum(1:4))
print(fivenum(c(1, NA, 3)))
print(fivenum(c(1, NA), na.rm = FALSE))
print(IQR(1:10))
print(IQR(c(2, 9, 4, 7), type = 6))
print(mad(c(1, 2, 3, 4, 100)))
print(mad(1:6, low = TRUE))
print(mad(1:6, high = TRUE))
print(mad(c(1, NA, 4)))
print(mad(c(1, NA, 4, 8), na.rm = TRUE))
print(mad(1:5, center = 0, constant = 1))
#==#
print(zapsmall(c(1, 1e-20, pi)))
print(zapsmall(123.456789), digits = 10)
print(zapsmall(c(1e-10, NA, 1)))
print(zapsmall(c(NA_real_, NA_real_)))
print(zapsmall(c(-5.5, 1e-9), digits = 3))
#==#
print(tabulate(c(2, 3, 5)))
print(tabulate(integer(0)))
print(tabulate(c(-1, 0)))
print(tabulate(c(1, 2, NA, 2), nbins = 4))
print(tabulate(c(2.7, 3.1), 2))
#==#
m <- matrix(1:6, 2)
print(sweep(m, 2, colSums(m)))
print(sweep(m, 1, 1:2, "*"))
print(sweep(m, 2, 1:3, FUN = `+`))
d <- matrix(1:4, 2, dimnames = list(rows = c("a", "b"), cols = c("x", "y")))
print(sweep(d, "cols", c(10, 20)))
#==#
print(scale(matrix(c(1, 2, 3, 4, 5, 9), 3)))
print(scale(1:3, center = FALSE))
print(scale(matrix(1:4, 2), center = c(1, 2), scale = FALSE))
print(as.matrix(c(a = 1, b = 2)))
print(as.matrix(1:3))
m <- matrix(0, 2, 3)
print(row(m))
print(col(m))
#==#
print(is.unsorted(c(1, 2, 2)))
print(is.unsorted(c(1, 2, 2), strictly = TRUE))
print(is.unsorted(c(1, NA, 0)))
print(is.unsorted(c(1, NA, 0), na.rm = TRUE))
print(is.unsorted(c("b", "a")))
print(is.unsorted(5))
print(anyDuplicated(c(1, 2, 1, 2)))
print(anyDuplicated(c(1, 2, 1, 2), fromLast = TRUE))
print(anyDuplicated(1:3))
print(anyDuplicated(c("x", NA, NA)))
#==#
b <- outer(matrix(1:4, 2), 1:2)
print(dim(b))
print(b)
m <- matrix(1:4, 2, dimnames = list(r = c("a", "b"), c = c("x", "y")))
print(dimnames(outer(m, c(p = 1, q = 2))))
print(outer(c(a = 1, b = 2), 1:2, "+"))
print(dim(outer(array(1:2, 2), array(1:3, 3))))
#==#
a <- array(1:24, c(2, 3, 4))
print(dim(aperm(a)))
print(aperm(a, c(2, 1, 3))[, , 1])
m <- matrix(1:6, 2, dimnames = list(r = c("a", "b"), c = c("x", "y", "z")))
print(aperm(m))
print(aperm(m, c("c", "r")))
r <- try(aperm(array(1:8, c(2, 2, 2)), c(1, 2)), silent = TRUE)
cat(r)
r <- try(aperm(1:3), silent = TRUE)
cat(r)
r <- try(aperm(matrix(1:4, 2), c(1, 1)), silent = TRUE)
cat(r)
print(attributes(aperm(structure(matrix(1:4, 2), extra = 1))))
print(attributes(aperm(structure(matrix(1:4, 2), extra = 1), c(1, 2))))
print(aperm(array(1:8, c(2, 2, 2)), c(2, 1, 3), resize = FALSE)[1:8])
#==#
print(kronecker(diag(2), matrix(1:4, 2)))
print(kronecker(1:2, 1:3))
m <- matrix(1:4, 2, dimnames = list(c("a", "b"), c("x", "y")))
print(kronecker(m, diag(2), make.dimnames = TRUE))
print(kronecker(1:2, matrix(1:4, 2), "+"))
#==#
v <- c(1, 2, 2, 5)
x <- c(0, 1, 1.5, 2, 4, 5, 6, NA)
print(findInterval(x, v))
print(findInterval(x, v, rightmost.closed = TRUE))
print(findInterval(x, v, all.inside = TRUE))
print(findInterval(x, v, left.open = TRUE))
print(findInterval(x, v, left.open = TRUE, rightmost.closed = TRUE))
print(findInterval(3, numeric(0)))
r <- try(findInterval(1, c(2, 1)), silent = TRUE)
cat(r)
#==#
print(rowsum(c(1, 2, 3, 4), c("b", "a", "b", "a")))
m <- matrix(1:6, 3, dimnames = list(NULL, c("p", "q")))
print(rowsum(m, c(2, 1, 2)))
print(rowsum(c(1, NA, 3), c(1, 1, 2), na.rm = TRUE))
print(rowsum(c(1, NA, 3), c(1, 1, 2)))
print(rowsum(1:3, c("x", "y", "x"), reorder = FALSE))
print(rowsum(c(1, 2), factor(c("u", "t"))))
#==#
print(split(1:6, c(1, 2)))
print(split(c(a = 1, b = 2, c = 3), c("x", "y", "x")))
print(split(1:2, c(1, 2, 3)))
print(split(factor(c("p", "q", "p")), c(1, 1, 2)))
print(split(letters[1:4], c("u", "v")))
withCallingHandlers(split(1:5, 1:2), warning = function(w) { print(conditionMessage(w)); invokeRestart("muffleWarning") })
#==#
cat(typeof(sum), typeof(`[`), typeof(paste), typeof(function(x) x), "\n")
cat(is.primitive(sum), is.primitive(`[`), is.primitive(paste), is.primitive(function(x) x), is.primitive(1), "\n")
cat(mode(`[`), mode(sum), mode(paste), "\n")
r <- tryCatch(cat(`[`), error = function(e) conditionMessage(e)); print(r)
r <- tryCatch(cat(paste), error = function(e) conditionMessage(e)); print(r)
#==#
# A condition object passed to stop/warning/message/signalCondition is signalled
# as itself: exiting handlers receive every field it carries, and its own call.
cnd <- structure(class = c("myError", "error", "condition"), list(message = "m", call = NULL, data = 42))
print(tryCatch(stop(cnd), myError = function(e) e$data))
cnd2 <- structure(class = c("myError", "error", "condition"), list(message = "m", call = quote(f(x))))
r <- tryCatch(stop(cnd2), error = function(e) e); print(r); print(conditionCall(r))
w <- structure(class = c("myW", "warning", "condition"), list(message = "wm", call = NULL, extra = "E"))
print(tryCatch(warning(w), myW = function(w) w$extra))
c1 <- structure(class = c("custom", "condition"), list(message = "cm", call = NULL, v = 1:3))
print(tryCatch(signalCondition(c1), custom = function(c) c$v))
print(tryCatch(message(c1), custom = function(c) c$v))
e <- errorCondition("ec msg", class = "ecls", data = list(k = 1))
print(tryCatch(stop(e), ecls = function(e) e$data)); print(class(e))
print(names(unclass(errorCondition("m", class = "a", x = 1, 2))))
print(tryCatch(warning(warningCondition("wc", class = "wcls")), wcls = function(w) class(w)))
r <- try(stop(errorCondition("tt", class = "x")), silent = TRUE)
print(class(attr(r, "condition"))); cat(r)
print(withCallingHandlers(tryCatch(stop(errorCondition("q", class = "z", data = 5)), z = function(e) e$data), z = function(e) cat("calling\n")))
x <- tryCatch(stop("plain"), error = function(e) e)
print(tryCatch(stop(x), error = function(e) identical(e, x)))
#==#
# A logical matrix subscript is an ordinary logical index, not a coordinate
# table, even when its column count equals the array's rank.
m <- matrix(1:4, 2); print(m[m > 2])
m[m > 2] <- 0L; print(m); print(which(m == 0))
a <- matrix(c(5, 6, 7, 8), 2); a[matrix(c(TRUE, FALSE, FALSE, TRUE), 2)] <- NA; print(a)
#==#
# factor(): positional levels, labels (merging duplicates, or a single stem),
# exclude, NA levels, names, and the invalid-labels error.
print(factor(c("x", "y"), labels = c("X", "Y")))
print(factor(c("b", "a", "c"), c("c", "b", "a")))
print(factor(c(a = "u", b = "v")))
print(factor(c(1, 2, 3, 1), labels = "L"))
print(factor(c(1, 2, 3, 1), levels = 1:3, labels = c("lo", "mid", "lo")))
print(factor(c("a", NA, "b"), exclude = NULL))
print(factor(c("a", "b", "c"), exclude = "b"))
print(factor(c(3, 1, 2), levels = 3:1, ordered = TRUE))
print(tryCatch(factor(1:3, labels = c("a", "b")), error = function(e) conditionMessage(e)))
f <- factor(c("a", "b")); print(factor(f, levels = c("b", "a"), labels = c("B", "A")))
print(factor(c("a", "b"), levels = c("a", "b", NA), exclude = NULL))
print(levels(factor(factor(c("p", "q", "r"))[c(1, 3)])))
print(as.character(factor(c("a", NA), exclude = NULL)))
#==#
# The body of tryCatch / try / withCallingHandlers / withRestarts / suppress*
# is a promise: it runs in the caller's environment, so its assignments land
# there, and sys.call() / parent.frame() inside it answer for the caller.
tryCatch(x <- 5, error = function(e) 1); print(x)
f <- function() { suppressWarnings(y <- 2); y }; print(f())
f2 <- function() { tryCatch({ z <- 3 }, finally = { z <- z + 1 }); z }; print(f2())
f3 <- function() { try(w <- 9, silent = TRUE); environment() }
print(exists("w", envir = f3(), inherits = FALSE))
g <- function() tryCatch(sys.call(), error = function(e) 1); print(g())
g2 <- function() tryCatch(parent.frame(), error = function(e) 1); print(identical(g2(), globalenv()))
g3 <- function() suppressWarnings(sys.call()); print(g3())
g4 <- function() withCallingHandlers(sys.call()); print(g4())
g5 <- function() try(sys.call()); print(g5())
withCallingHandlers({ a <- 1; warning("w"); a <- 2 }, warning = function(w) invokeRestart("muffleWarning")); print(a)
r <- withRestarts({ b <- 10; invokeRestart("rr", 3) }, rr = function(v) v); print(c(r, b))
h <- function() { v <- 0; tryCatch({ v <- 1; stop("e") }, error = function(e) v <<- v + 10); v }; print(h())
k <- function(n) tryCatch(if (n > 0) k(n - 1) else sys.call(), error = function(e) e); print(k(2))
#==#
# tryCatch and try are R code: a condition raised directly in expr names the
# doTryCatch() frame tryCatch's helpers make (tryCatchList() with no
# handlers), a handler runs as value[[3L]](cond), an unhandled condition keeps
# its own call on the way out, and try() reports a doTryCatch call as its own.
e <- tryCatch(stop("boom"), error = function(e) e); print(conditionCall(e)); print(e)
print(tryCatch(warning("ww"), warning = function(w) w))
tryCatch(stop("a"), error = function(e) print(sys.call()))
tryCatch(stop("m"), warning = function(w) 1, error = function(e) print(sys.call()))
f <- function() stop("in f"); print(tryCatch(f(), error = function(e) conditionCall(e)))
r <- try(tryCatch(stop("x"), finally = 1), silent = TRUE); cat(r)
r <- try(tryCatch(stop("x"), warning = function(w) 1), silent = TRUE); cat(r)
print(tryCatch(tryCatch(stop("inner"), warning = function(w) "w"), error = function(e) conditionCall(e)))
r <- try(stop("zz"), silent = TRUE); print(conditionCall(attr(r, "condition"))); cat(r)
#==#
# `%in%` is a function value like any other operator.
f <- `%in%`; print(f(1, 1:2)); print(Reduce(`%in%`, list(1:3, 2:5))); print(sapply(list(1, 5), `%in%`, 1:3))
print(Map(`%in%`, list(1, 9), list(1:3, 1:3))); print(do.call(`%in%`, list(c("a", "z"), letters)))
print(`%in%`(factor(c("a", "q")), c("a", "b"))); print(is.primitive(`%in%`)); print(typeof(`%in%`))
#==#
# cat(): fill wraps at the given (or 80-column) width and ends with a newline,
# labels head each line, and sep is recycled along the elements written.
cat("a", "b", fill = TRUE); cat("x\n")
cat(paste("word", 1:30), fill = 40)
cat(paste("item", 1:12), fill = 30, labels = paste0("(", letters[1:4], ")"))
cat(1:10, sep = c(",", ";")); cat("\n")
cat(1:3, c("a", "b"), sep = c("-", "+", "|")); cat("\n")
cat("a", character(0), "b\n"); cat(NULL, "x\n"); cat("a", NULL, "b\n")
cat(1:5, fill = 6)
#==#
# ...length(), ...elt() and ...names() read the calling frame's `...`.
g <- function(...) ...length(); print(g(1, 2, 3)); print(g())
h <- function(...) ...names(); print(h(a = 1, 2)); print(h(1, 2)); print(h())
e <- function(...) ...elt(2); print(e(1, "two", 3))
print(tryCatch(...length(), error = function(e) conditionMessage(e)))
print(tryCatch(e(1), error = function(e) conditionMessage(e)))
k <- function(...) ...elt(0); print(tryCatch(k(1), error = function(e) conditionMessage(e)))
#==#
# A call subassigns and subsets like the list of its parts; call() and
# as.call() build one, and eval() takes a list as its environment.
e <- quote(f(x, y = 2)); e[[1]] <- as.name("g"); print(e)
e[[3]] <- 10; print(e); e$y <- 5; print(e); e[["n"]] <- TRUE; print(e); print(names(e))
e <- quote(a + b); e[[1]] <- as.name("-"); print(e); print(eval(e, list(a = 5, b = 2)))
cl <- call("sum", 1, 2); print(cl); cl[[4]] <- 3; print(cl); print(eval(cl))
e <- quote(f(x)); e[[2]] <- quote(y + 1); print(e); print(as.list(e))
e <- quote(f(1, 2, 3)); print(e[-2]); print(e[2:3])
e <- quote(if (a) b else c); e[[2]] <- quote(x > 1); print(e); e[[4]] <- NULL; print(e)
print(call("+", 1, 2)); print(as.call(list(as.name("max"), 1, 5))); print(names(quote(f(1))))
f <- function() { z <- 7; eval(quote(a * z), list(a = 3)) }; print(f())
#==#
# A call with a backtick-quoted keyword or operator head is the call its
# syntax spells: `if`(a, b, c) is if (a) b else c, and evaluates lazily.
print(`if`(TRUE, "yes", stop("never"))); print(`if`(FALSE, 1, 2)); print(is.null(`if`(FALSE, 1)))
print(`(`(5)); print(`{`(x <- 1, x + 1)); `<-`(y, 3); print(y); `<<-`(z, 4); print(z)
print(`&&`(TRUE, FALSE)); print(`||`(TRUE, stop("never"))); print(`[`(c(5, 6), 2)); print(`[[`(list(7, 8), 2))
for (i in 1:2) `for`(j, 1:2, cat(i * j, "")); cat("\n"); n <- 0; `while`(n < 3, n <- n + 1); print(n)
`repeat`({ n <- n + 1; if (n > 5) break }); print(n); print(`+`(1, 2)); print(`-`(5)); print(`%%`(7, 3))
print(quote(`if`(a, b, c))); print(quote(`+`(x, 1))); print(quote(`(`(x))); print(quote(`{`(a, b)))
print(quote(`<-`(x, 1))); print(quote(`[`(x, 1))); print(quote(`for`(i, s, b))); print(quote(`-`(a)))
#==#
# A primitive prints the formals R keeps for it in front of the .Primitive
# call; one with none prints the bare call. args() is a closure with those
# formals and a NULL body.
print(sum); print(length); print(c); print(`+`); print(round); print(log); print(rep); print(`[`)
print(max); print(is.na); print(sqrt); print(`%*%`); print(seq_len)
args(sum); args(length); print(args("if")); print(args("max")); print(is.function(args(sum)))
f <- function(a, b = 2, ...) a + b; args(f); g <- args(f); print(formals(g)$b); print(body(args(f)))
print(environmentName(environment(args(sum)))); print(args(1)); print(deparse(sum)); print(deparse(args(f)))
#==#
# The language keywords are function values: they print as their .Primitive
# call, answer typeof/is.primitive as R's specials and builtins do, and called
# through a value evaluate their arguments only as the keyword would.
print(`if`); print(`for`); print(`{`); print(`(`); print(`<-`); print(`&&`); print(`||`); print(`while`)
print(sapply(list(`if`, `for`, `{`, `(`, `<-`, `&&`, `repeat`), typeof)); print(is.primitive(`if`)); print(is.function(`(`))
f <- `if`; print(f(TRUE, "a", stop("never"))); print(f(FALSE, 1, 2)); print(is.null(f(FALSE, 1)))
p <- `(`; print(p(3)); print(sapply(1:3, `(`)); print(do.call(`{`, list(1, 2, 3))); print(do.call(`{`, list()))
a <- `&&`; o <- `||`; print(a(TRUE, NA)); print(a(FALSE, stop("never"))); print(a(NA, FALSE)); print(o(NA, TRUE)); print(o(FALSE, NA))
print(Reduce(`&&`, list(TRUE, TRUE, FALSE))); print(Map(`if`, c(TRUE, FALSE), "y", "n"))
print(tryCatch(f(NA, 1, 2), error = function(e) conditionMessage(e))); print(tryCatch(a(1:2, TRUE), error = function(e) conditionMessage(e)))
print(tryCatch(p(1, 2), error = function(e) conditionMessage(e))); print(exists("if")); print(exists("("))
#==#
# identical() on functions and language: a primitive and a symbol are one
# object per name, calls compare part by part, and closures by formals, body
# and environment.
print(identical(sum, sum)); print(identical(sum, max)); g <- sum; print(identical(g, sum)); print(identical(`if`, `if`))
print(identical(`+`, `+`)); print(identical(quote(a), quote(a))); print(identical(quote(a), quote(b)))
print(identical(quote(a + b), quote(a + b))); print(identical(quote(a + b), quote(a - b))); print(identical(quote(f(x = 1)), quote(f(x = 2))))
f <- function(x) x; h <- function(x) x; k <- function(x) x + 1; m <- local(function(x) x)
print(identical(f, f)); print(identical(f, h)); print(identical(f, k)); print(identical(f, m)); print(identical(f, sum))
mk <- function() function(y) y; print(identical(mk(), mk())); a <- mk(); print(identical(a, a))
print(identical(as.name("x"), quote(x))); print(identical(call("sum", 1), quote(sum(1)))); print(identical(list(sum, quote(z)), list(sum, quote(z))))
#==#
# geterrmessage(): "" before any error; an error raised from a message
# writes its text as it is raised, even when a tryCatch handler takes it; try()
# replaces it with the Error in <call> : line it made; stop(cond) leaves it.
print(geterrmessage()); r <- tryCatch(stop("first"), error = function(e) 1); print(geterrmessage())
try(stop("boom"), silent = TRUE); print(geterrmessage()); f <- function() stop("deep"); try(f(), silent = TRUE); cat(geterrmessage())
r <- tryCatch(stop(simpleError("obj")), error = function(e) 2); cat(geterrmessage()); try(stop(), silent = TRUE); print(geterrmessage())
r <- tryCatch(log("a"), error = function(e) 3); print(geterrmessage()); try(stop(simpleError("cnd")), silent = TRUE); print(geterrmessage())
r <- tryCatch(warning("w"), warning = function(w) 4); print(geterrmessage())
#==#
# sys.nframe(): the depth of the frame it was written in, counting every
# function context beneath it — a base closure like print included.
print(sys.nframe()); f <- function() sys.nframe(); print(f()); x <- f(); print(x)
g <- function() f(); print(g()); h <- function() { n <- sys.nframe(); n }; y <- h(); print(y)
k <- function(v) v; z <- k(f()); print(z); w <- k(sys.nframe()); print(w); m <- function() k(sys.nframe()); print(m())
r <- function(n) if (n == 0) sys.nframe() else r(n - 1); print(r(3)); print(sapply(1:2, function(i) sys.nframe()))
print(tryCatch(sys.nframe(), error = function(e) -1)); q <- function() tryCatch(sys.nframe(), error = function(e) -1); print(q())
print(do.call(f, list())); print(Reduce(function(a, b) sys.nframe(), 1:2))
