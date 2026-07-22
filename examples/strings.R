# String primitives, regular expressions, and formatting.
s <- "The quick brown fox"
cat("chars:", nchar(s), "\n")
cat("upper:", toupper(s), "\n")
cat("first word:", substr(s, 1, 3), "\n")

words <- strsplit(s, " ")[[1]]
print(words)
stopifnot(length(words) == 4)

cat("joined:", paste(words, collapse = "-"), "\n")
cat("numbered:", paste0(seq_along(words), ".", words), "\n")

cat(sprintf("%-8s|%5.2f|%04d\n", "left", 3.14159, 42L))
cat(gsub("o", "0", s), "\n")
cat("has fox:", grepl("fox$", s), "\n")

sorted <- sort(c("pear", "apple", "fig"))
print(sorted)
