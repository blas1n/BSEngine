#pragma once

#include <wchar.h>
#include "Type.h"

namespace BE
{
	inline Char* Strcpy(Char* dest, size_t destCount, const Char* src)
	{
		wcscpy_s(dest, destCount, src);
		return dest;
	}

	inline Char* Strncpy(Char* dest, size_t destCount, const Char* src, size_t srcCount)
	{
		wcsncpy_s(dest, destCount, src, srcCount);
		return dest;
	}

	inline Char* Strcat(Char* dest, size_t destCount, const Char* src)
	{
		wcscat_s(dest, destCount, src);
		return dest;
	}

	inline Char* Strncat(Char* dest, size_t destCount, const Char* src, size_t srcCount)
	{
		wcsncat_s(dest, destCount, src, srcCount);
		return dest;
	}

	inline size_t Strxfrm(Char* dest, const Char* src, size_t count)
	{
		return wcsxfrm(dest, src, count);
	}

	/// @todo Use safe function
	inline size_t Strlen(const Char* str)
	{
		return wcslen(str);
	}

	inline Int32 Strcmp(const Char* lhs, const Char* rhs)
	{
		return wcscmp(lhs, rhs);
	}

	inline Int32 Strncmp(const Char* lhs, const Char* rhs, size_t count)
	{
		return wcsncmp(lhs, rhs, count);
	}

	inline const Char* Strstr(const Char* str, const Char* subStr)
	{
		return wcsstr(str, subStr);
	}

	inline const Char* Strchr(const Char* str, Char ch)
	{
		return wcschr(str, ch);
	}

	inline const Char* Strrchr(const Char* str, Char ch)
	{
		return wcsrchr(str, ch);
	}

	inline Char* Strtok(Char* str, const Char* delim, Char** ptr)
	{
		return wcstok_s(str, delim, ptr);
	}

	inline Int32 StrToInt(const Char* start, Char** end, Int32 radix = 10)
	{
		return wcstol(start, end, radix);
	}

	inline Int64 StrToInt64(const Char* start, Char** end, Int32 radix = 10)
	{
		return wcstoll(start, end, radix);
	}

	inline Uint32 StrToUint(const Char* start, Char** end, Int32 radix = 10)
	{
		return wcstoul(start, end, radix);
	}

	inline Uint64 StrToUint64(const Char* start, Char** end, Int32 radix = 10)
	{
		return wcstoull(start, end, radix);
	}
}