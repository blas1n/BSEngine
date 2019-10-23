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

#define INTERFACE_BEGIN(name) \
class BS_API I##name abstract { \
	public:
#define INTERFACE_END };

#define INTERFACE_DEF(ret, name, ...) \
virtual ret name(__VA_ARGS__) = 0;

#if _DEBUG
#define check(expr) assert(expr); 
#else
#define check(expr) { if (!expr) { return; } }
#endif