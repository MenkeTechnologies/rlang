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
