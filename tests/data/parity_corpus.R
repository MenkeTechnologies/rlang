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
