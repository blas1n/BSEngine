#pragma once

#include <cassert>

#define BS_API

#ifdef _UNICODE
using tchar = wchar_t;
#define TEXT(x) L##x
#else
using tchar = char;
#define TEXT(x) x
#endif

#define static_check(expr) { static_assert(expr); }

#define check(expr) { assert(expr); }