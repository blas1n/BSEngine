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

#if _DEBUG
#define check(expr) { if (!(expr)) { assert(false); } }
#else
#define check(expr) { if (!(expr)) { return; } }
#endif