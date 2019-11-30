#pragma once

#include <cwchar>
#include "Type.h"

namespace BE
{
	inline Char* Strcpy(Char* dest, const Char* src, const size_t count) noexcept
	{
		return std::wcsncpy(dest, src, count);
	}

	inline Char* Strcat(Char* dest, const Char* src, const size_t count) noexcept
	{
		return std::wcsncat(dest, src, count);
	}

	inline size_t Strxfrm(Char* dest, const Char* src, const size_t count) noexcept
	{
		return std::wcsxfrm(dest, src, count);
	}

	inline size_t Strlen(const Char* str) noexcept
	{
		return std::wcslen(str);
	}

	inline Int32 Strcmp(const Char* lhs, const Char* rhs, const size_t count) noexcept
	{
		return std::wcsncmp(lhs, rhs, count);
	}

	inline Char* Strstr(Char* str, const Char* subStr) noexcept
	{
		return std::wcsstr(str, subStr);
	}

	inline const Char* Strstr(const Char* str, const Char* subStr) noexcept
	{
		return std::wcsstr(str, subStr);
	}

	inline Char* Strchr(Char* str, const Char ch) noexcept
	{
		return std::wcschr(str, ch);
	}

	inline const Char* Strchr(const Char* str, const Char ch) noexcept
	{
		return std::wcschr(str, ch);
	}

	inline Char* Strrchr(Char* str, const Char ch) noexcept
	{
		return std::wcsrchr(str, ch);
	}

	inline const Char* Strrchr(const Char* str, const Char ch) noexcept
	{
		return std::wcsrchr(str, ch);
	}

	inline Char* Strtok(Char* str, const Char* delim, Char** ptr) noexcept
	{
		return std::wcstok(str, delim, ptr);
	}

	inline Int32 StrToInt(const Char* start, Char** end, Int32 radix = 10) noexcept
	{
		return std::wcstol(start, end, radix);
	}

	inline Int64 StrToInt64(const Char* start, Char** end, Int32 radix = 10) noexcept
	{
		return std::wcstoll(start, end, radix);
	}

	inline Uint32 StrToUint(const Char* start, Char** end, Int32 radix = 10) noexcept
	{
		return std::wcstoul(start, end, radix);
	}

	inline Uint64 StrToUint64(const Char* start, Char** end, Int32 radix = 10) noexcept
	{
		return std::wcstoull(start, end, radix);
	}

	inline float StrToFloat(const Char* str, Char** strEnd) noexcept
	{
		return std::wcstof(str, strEnd);
	}
}