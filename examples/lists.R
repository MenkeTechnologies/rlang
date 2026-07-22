# Lists, names, and copy-on-modify assignment.
person <- list(name = "ada", born = 1815, fields = c("math", "computing"))
cat(person$name, "born", person$born, "\n")
print(names(person))
cat("fields:", person$fields, "\n")

person$born <- 1816
person[["awake"]] <- TRUE
print(length(person))

# Assignment copies: modifying a copy leaves the original alone.
other <- person
other$name <- "grace"
cat(person$name, other$name, "\n")
stopifnot(person$name == "ada")

# Nested replacement rebuilds outward-in.
person$fields[2] <- "programming"
cat("fields:", person$fields, "\n")

nested <- list(inner = list(values = 1:3))
nested$inner$values[2] <- 99
cat("nested:", nested$inner$values, "\n")
