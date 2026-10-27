## 0.4.0

> - Date: 2025/03/02
> - rustc 1.85.0 (4d91de4e4 2025-02-17)
> - macOS 15.3.1
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 1050 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 87/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 174/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 261/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 348/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 435/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 522/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 609/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 696/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 783/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 870/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 957/1050
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 1044/1050
iiiiii
test result: ok. 0 passed; 0 failed; 1050 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 4 tests
test bench_camel_case              ... bench:          81.64 ns/iter (+/- 3.69)
test bench_camel_case_with_keep    ... bench:          92.83 ns/iter (+/- 34.81)
test bench_camel_case_with_options ... bench:          87.87 ns/iter (+/- 3.66)
test bench_camel_case_with_sep     ... bench:          86.67 ns/iter (+/- 4.49)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 1.89s


running 5 tests
test bench_cobol_case                   ... bench:          88.78 ns/iter (+/- 2.70)
test bench_cobol_case_with_keep         ... bench:          94.41 ns/iter (+/- 3.06)
test bench_cobol_case_with_nums_as_word ... bench:          89.05 ns/iter (+/- 3.08)
test bench_cobol_case_with_options      ... bench:          94.79 ns/iter (+/- 3.06)
test bench_cobol_case_with_sep          ... bench:          96.23 ns/iter (+/- 3.52)

test result: ok. 0 passed; 0 failed; 0 ignored; 5 measured; 0 filtered out; finished in 3.66s


running 5 tests
test bench_kebab_case                   ... bench:          83.26 ns/iter (+/- 2.16)
test bench_kebab_case_with_keep         ... bench:          91.18 ns/iter (+/- 2.89)
test bench_kebab_case_with_nums_as_word ... bench:          82.01 ns/iter (+/- 4.32)
test bench_kebab_case_with_options      ... bench:          86.66 ns/iter (+/- 4.85)
test bench_kebab_case_with_sep          ... bench:          90.23 ns/iter (+/- 2.73)

test result: ok. 0 passed; 0 failed; 0 ignored; 5 measured; 0 filtered out; finished in 6.33s


running 5 tests
test bench_macro_case                   ... bench:          87.88 ns/iter (+/- 3.15)
test bench_macro_case_with_keep         ... bench:          93.46 ns/iter (+/- 2.83)
test bench_macro_case_with_nums_as_word ... bench:          88.85 ns/iter (+/- 2.64)
test bench_macro_case_with_options      ... bench:          94.76 ns/iter (+/- 2.52)
test bench_macro_case_with_sep          ... bench:          95.82 ns/iter (+/- 7.52)

test result: ok. 0 passed; 0 failed; 0 ignored; 5 measured; 0 filtered out; finished in 8.89s


running 4 tests
test bench_pascal_case              ... bench:          76.86 ns/iter (+/- 0.87)
test bench_pascal_case_with_keep    ... bench:          85.15 ns/iter (+/- 3.81)
test bench_pascal_case_with_options ... bench:          85.78 ns/iter (+/- 13.36)
test bench_pascal_case_with_sep     ... bench:          85.32 ns/iter (+/- 2.65)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 4.27s


running 5 tests
test bench_snake_case                   ... bench:          83.10 ns/iter (+/- 2.90)
test bench_snake_case_with_keep         ... bench:          88.14 ns/iter (+/- 2.99)
test bench_snake_case_with_nums_as_word ... bench:          81.79 ns/iter (+/- 10.83)
test bench_snake_case_with_options      ... bench:          91.27 ns/iter (+/- 6.66)
test bench_snake_case_with_sep          ... bench:          90.01 ns/iter (+/- 2.19)

test result: ok. 0 passed; 0 failed; 0 ignored; 5 measured; 0 filtered out; finished in 11.90s


running 5 tests
test bench_train_case                   ... bench:          88.54 ns/iter (+/- 2.99)
test bench_train_case_with_keep         ... bench:          96.21 ns/iter (+/- 3.66)
test bench_train_case_with_nums_as_word ... bench:          89.96 ns/iter (+/- 4.88)
test bench_train_case_with_options      ... bench:          94.68 ns/iter (+/- 2.90)
test bench_train_case_with_sep          ... bench:          97.82 ns/iter (+/- 19.20)

test result: ok. 0 passed; 0 failed; 0 ignored; 5 measured; 0 filtered out; finished in 2.24s
```

## 0.3.0

> - Date: 2024/08/12
> - rustc 1.80.1 (3f5fd8dd4 2024-08-06)
> - macOS 14.5
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 320 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 87/320
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 174/320
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 261/320
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
test result: ok. 0 passed; 0 failed; 320 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test bench_camel_case           ... bench:          74.57 ns/iter (+/- 5.88)
test bench_camel_case_with_keep ... bench:          81.65 ns/iter (+/- 8.80)
test bench_camel_case_with_sep  ... bench:         136.92 ns/iter (+/- 7.96)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 5.36s


running 4 tests
test bench_cobol_case                   ... bench:          81.12 ns/iter (+/- 22.88)
test bench_cobol_case_with_keep         ... bench:          90.14 ns/iter (+/- 3.47)
test bench_cobol_case_with_nums_as_word ... bench:          81.17 ns/iter (+/- 16.95)
test bench_cobol_case_with_sep          ... bench:         139.23 ns/iter (+/- 10.61)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 10.17s


running 4 tests
test bench_kebab_case                   ... bench:          73.24 ns/iter (+/- 8.60)
test bench_kebab_case_with_keep         ... bench:          88.79 ns/iter (+/- 6.19)
test bench_kebab_case_with_nums_as_word ... bench:          79.29 ns/iter (+/- 7.69)
test bench_kebab_case_with_sep          ... bench:         132.55 ns/iter (+/- 11.43)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 8.44s


running 4 tests
test bench_macro_case                   ... bench:          81.61 ns/iter (+/- 15.01)
test bench_macro_case_with_keep         ... bench:          90.84 ns/iter (+/- 5.38)
test bench_macro_case_with_nums_as_word ... bench:          78.95 ns/iter (+/- 5.14)
test bench_macro_case_with_sep          ... bench:         139.27 ns/iter (+/- 4.86)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 21.07s


running 3 tests
test bench_pascal_case           ... bench:          79.22 ns/iter (+/- 4.66)
test bench_pascal_case_with_keep ... bench:          86.73 ns/iter (+/- 3.84)
test bench_pascal_case_with_sep  ... bench:         147.03 ns/iter (+/- 7.23)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 11.00s


running 4 tests
test bench_snake_case                   ... bench:          71.92 ns/iter (+/- 7.66)
test bench_snake_case_with_keep         ... bench:          87.70 ns/iter (+/- 6.03)
test bench_snake_case_with_nums_as_word ... bench:          78.77 ns/iter (+/- 7.22)
test bench_snake_case_with_sep          ... bench:         130.50 ns/iter (+/- 6.55)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 8.48s


running 4 tests
test bench_train_case                   ... bench:          74.39 ns/iter (+/- 4.34)
test bench_train_case_with_keep         ... bench:          88.30 ns/iter (+/- 21.66)
test bench_train_case_with_nums_as_word ... bench:          74.07 ns/iter (+/- 5.50)
test bench_train_case_with_sep          ... bench:         117.55 ns/iter (+/- 13.56)
```

## 0.2.1 

> - Date: 2024/02/26
> - Rust: 1.76.0 (c84b36747 2024-01-18)
> - macOS
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 248 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 88/248
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 176/248
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
test result: ok. 0 passed; 0 failed; 248 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test bench_camel_case           ... bench:          82 ns/iter (+/- 7)
test bench_camel_case_with_keep ... bench:          88 ns/iter (+/- 6)
test bench_camel_case_with_sep  ... bench:         150 ns/iter (+/- 7)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 2.44s


running 3 tests
test bench_cobol_case           ... bench:          78 ns/iter (+/- 5)
test bench_cobol_case_with_keep ... bench:          91 ns/iter (+/- 6)
test bench_cobol_case_with_sep  ... bench:         146 ns/iter (+/- 12)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.94s


running 3 tests
test bench_kebab_case           ... bench:          80 ns/iter (+/- 6)
test bench_kebab_case_with_keep ... bench:          87 ns/iter (+/- 5)
test bench_kebab_case_with_sep  ... bench:         151 ns/iter (+/- 10)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.92s


running 3 tests
test bench_macro_case           ... bench:         104 ns/iter (+/- 6)
test bench_macro_case_with_keep ... bench:         105 ns/iter (+/- 33)
test bench_macro_case_with_sep  ... bench:         143 ns/iter (+/- 9)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 1.20s


running 3 tests
test bench_pascal_case           ... bench:          79 ns/iter (+/- 9)
test bench_pascal_case_with_keep ... bench:          95 ns/iter (+/- 2)
test bench_pascal_case_with_sep  ... bench:         146 ns/iter (+/- 36)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 7.45s


running 3 tests
test bench_snake_case           ... bench:          84 ns/iter (+/- 4)
test bench_snake_case_with_keep ... bench:          90 ns/iter (+/- 7)
test bench_snake_case_with_sep  ... bench:         158 ns/iter (+/- 214)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.98s


running 3 tests
test bench_train_case           ... bench:          82 ns/iter (+/- 5)
test bench_train_case_with_keep ... bench:          89 ns/iter (+/- 3)
test bench_train_case_with_sep  ... bench:         140 ns/iter (+/- 10)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 6.83s
```

## 0.2.0

> - Date: 2024/02/25
> - Rust: 1.76.0 (c84b36747 2024-01-18)
> - macOS
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 248 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 88/248
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 176/248
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
test result: ok. 0 passed; 0 failed; 248 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test bench_camel_case           ... bench:          78 ns/iter (+/- 3)
test bench_camel_case_with_keep ... bench:          88 ns/iter (+/- 6)
test bench_camel_case_with_sep  ... bench:         149 ns/iter (+/- 12)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.91s


running 3 tests
test bench_cobol_case           ... bench:         108 ns/iter (+/- 21)
test bench_cobol_case_with_keep ... bench:         103 ns/iter (+/- 8)
test bench_cobol_case_with_sep  ... bench:         146 ns/iter (+/- 12)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.94s


running 3 tests
test bench_kebab_case           ... bench:          82 ns/iter (+/- 24)
test bench_kebab_case_with_keep ... bench:          87 ns/iter (+/- 2)
test bench_kebab_case_with_sep  ... bench:         155 ns/iter (+/- 12)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 9.63s


running 3 tests
test bench_macro_case           ... bench:          78 ns/iter (+/- 4)
test bench_macro_case_with_keep ... bench:          90 ns/iter (+/- 2)
test bench_macro_case_with_sep  ... bench:         144 ns/iter (+/- 8)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 9.58s


running 3 tests
test bench_pascal_case           ... bench:          77 ns/iter (+/- 3)
test bench_pascal_case_with_keep ... bench:          93 ns/iter (+/- 12)
test bench_pascal_case_with_sep  ... bench:         145 ns/iter (+/- 9)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 4.26s


running 3 tests
test bench_snake_case           ... bench:          80 ns/iter (+/- 7)
test bench_snake_case_with_keep ... bench:          89 ns/iter (+/- 27)
test bench_snake_case_with_sep  ... bench:         151 ns/iter (+/- 11)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.78s


running 3 tests
test bench_train_case           ... bench:          82 ns/iter (+/- 6)
test bench_train_case_with_keep ... bench:          86 ns/iter (+/- 8)
test bench_train_case_with_sep  ... bench:         142 ns/iter (+/- 14)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 1.40s
```


## 0.1.1

> - Date: 2024/02/23
> - Rust: 1.76.0 (c84b36747 2024-01-18)
> - macOS
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 206 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 88/206
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 176/206
iiiiiiiiiiiiiiiiiiiiiiiiiiiiii
test result: ok. 0 passed; 0 failed; 206 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test bench_camel_case           ... bench:          79 ns/iter (+/- 4)
test bench_camel_case_with_keep ... bench:          86 ns/iter (+/- 5)
test bench_camel_case_with_sep  ... bench:         129 ns/iter (+/- 10)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 3.99s


running 3 tests
test bench_cobol_case           ... bench:         104 ns/iter (+/- 10)
test bench_cobol_case_with_keep ... bench:         117 ns/iter (+/- 15)
test bench_cobol_case_with_sep  ... bench:         165 ns/iter (+/- 13)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 1.03s


running 3 tests
test bench_kebab_case           ... bench:          79 ns/iter (+/- 6)
test bench_kebab_case_with_keep ... bench:          91 ns/iter (+/- 7)
test bench_kebab_case_with_sep  ... bench:         135 ns/iter (+/- 13)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.85s


running 3 tests
test bench_macro_case           ... bench:          81 ns/iter (+/- 6)
test bench_macro_case_with_keep ... bench:          92 ns/iter (+/- 6)
test bench_macro_case_with_sep  ... bench:         142 ns/iter (+/- 17)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 10.21s


running 3 tests
test bench_pascal_case           ... bench:          86 ns/iter (+/- 7)
test bench_pascal_case_with_keep ... bench:          86 ns/iter (+/- 6)
test bench_pascal_case_with_sep  ... bench:         126 ns/iter (+/- 10)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 1.16s


running 3 tests
test bench_snake_case           ... bench:          79 ns/iter (+/- 7)
test bench_snake_case_with_keep ... bench:          94 ns/iter (+/- 6)
test bench_snake_case_with_sep  ... bench:         136 ns/iter (+/- 12)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.86s


running 3 tests
test bench_train_case           ... bench:          86 ns/iter (+/- 10)
test bench_train_case_with_keep ... bench:          86 ns/iter (+/- 11)
test bench_train_case_with_sep  ... bench:         134 ns/iter (+/- 8)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 10.44s
```

## 0.1.0

> - Date: 2024/01/18
> - Rust: 1.76.0 (c84b36747 2024-01-18)
> - macOS
> - Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz

```
% ./build.sh bench

running 206 tests
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 88/206
iiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii 176/206
iiiiiiiiiiiiiiiiiiiiiiiiiiiiii
test result: ok. 0 passed; 0 failed; 206 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test bench_camel_case           ... bench:         118 ns/iter (+/- 7)
test bench_camel_case_with_keep ... bench:         131 ns/iter (+/- 13)
test bench_camel_case_with_sep  ... bench:         174 ns/iter (+/- 8)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 5.84s


running 3 tests
test bench_cobol_case           ... bench:         216 ns/iter (+/- 29)
test bench_cobol_case_with_keep ... bench:         235 ns/iter (+/- 10)
test bench_cobol_case_with_sep  ... bench:         295 ns/iter (+/- 27)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 5.56s


running 3 tests
test bench_kebab_case           ... bench:         198 ns/iter (+/- 11)
test bench_kebab_case_with_keep ... bench:         214 ns/iter (+/- 11)
test bench_kebab_case_with_sep  ... bench:         257 ns/iter (+/- 19)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 4.17s


running 3 tests
test bench_macro_case           ... bench:         190 ns/iter (+/- 15)
test bench_macro_case_with_keep ... bench:         200 ns/iter (+/- 12)
test bench_macro_case_with_sep  ... bench:         235 ns/iter (+/- 26)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.55s


running 3 tests
test bench_pascal_case           ... bench:         131 ns/iter (+/- 12)
test bench_pascal_case_with_keep ... bench:         126 ns/iter (+/- 12)
test bench_pascal_case_with_sep  ... bench:         180 ns/iter (+/- 3)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 6.29s


running 3 tests
test bench_snake_case           ... bench:         198 ns/iter (+/- 15)
test bench_snake_case_with_keep ... bench:         208 ns/iter (+/- 14)
test bench_snake_case_with_sep  ... bench:         258 ns/iter (+/- 6)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 5.45s


running 3 tests
test bench_train_case           ... bench:         202 ns/iter (+/- 19)
test bench_train_case_with_keep ... bench:         219 ns/iter (+/- 61)
test bench_train_case_with_sep  ... bench:         243 ns/iter (+/- 19)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 0.99s
```
