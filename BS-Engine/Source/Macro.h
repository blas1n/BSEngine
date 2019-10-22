#pragma once

#ifdef BUILD_ME_AS_STATICLIB
#define BS_API declspec(dllimport)
#else
#ifdef BUILD_ME_AS_DLL
#define BS_API declspec(dllexport)
#else
#define BS_API 
#endif
#endif

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