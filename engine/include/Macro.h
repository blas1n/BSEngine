#pragma once

#include <cassert>

#define BS_API

#define check(expr) { assert(expr); }

#define TEXT_PASTE(x) L ## x

#undef TEXT
#define TEXT(x) TEXT_PASTE(x)