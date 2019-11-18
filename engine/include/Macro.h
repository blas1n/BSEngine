#pragma once

#include <cassert>

#define BS_API

#define check(expr) { assert(expr); }

#define TEXT_PASTE(x) u ## x

#undef TEXT
#define TEXT(x) TEXT_PASTE(x)