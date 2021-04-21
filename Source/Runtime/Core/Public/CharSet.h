#pragma once

#include <fstream>
#include <istream>
#include <ios>
#include <sstream>
#include <streambuf>
#include <string>
#include <ostream>
#include "BSBase/Base.h"

#ifdef STR
#	undef STR
#endif

#define STR_IMPL(x) u##x
#define STR(x) STR_IMPL(x)

using Char = char16_t;
using String = std::u16string;
using StringView = std::u16string_view;

using Ios = std::basic_ios<Char>;
using StreamBuf = std::basic_streambuf<Char>;
using FileBuf = std::basic_filebuf<Char>;
using StringBuf = std::basic_stringbuf<Char>;

using Istream = std::basic_istream<Char>;
using Ostream = std::basic_istream<Char>;
using IOStream = std::basic_iostream<Char>;

using IfStream = std::basic_ifstream<Char>;
using OfStream = std::basic_ofstream<Char>;
using FStream = std::basic_fstream<Char>;

using IStringStream = std::basic_istringstream<Char>;
using OStringStream = std::basic_ostringstream<Char>;
using StringStream = std::basic_stringstream<Char>;

namespace Impl
{
	NO_ODR std::string ToString(std::string_view from) { return std::string(from.begin(), from.end()); }
	CORE_API std::string ToString(std::wstring_view from);
	CORE_API std::string ToString(std::u16string_view from);
	CORE_API std::string ToString(std::u32string_view from);
	
	CORE_API std::wstring ToWString(std::string_view from);

	NO_ODR std::wstring ToWString(std::wstring_view from) { return std::wstring(from.begin(), from.end()); }
	
	// sizeof(wchar_t) must be 1byte or 2byte
	CORE_API std::wstring ToWString(std::u16string_view from);

	// sizeof(wchar_t) must be 1byte
	CORE_API std::wstring ToWString(std::u32string_view from);

	CORE_API std::u16string ToU16String(std::string_view from);

	// sizeof(wchar_t) must be 1byte
	CORE_API std::u16string ToU16String(std::wstring_view from);

	NO_ODR std::u16string ToU16String(std::u16string_view from) { return std::u16string(from.begin(), from.end()); }
	
	CORE_API std::u32string ToU32String(std::string_view from);

	// sizeof(wchar_t) must be 1byte
	CORE_API std::u32string ToU32String(std::wstring_view from);
}

template <class To, class From>
std::basic_string<To> CastCharSet(std::basic_string_view<From> from)
{
	static_assert(sizeof(From) < 2 || sizeof(To) < 2, "Mutual conversion between utf16 and utf32 is not supported");

	if constexpr (std::is_same_v<To, char>)
		return Impl::ToString(from);
	else if (std::is_same_v<To, wchar_t>)
		return Impl::ToWString(from);
	else if (std::is_same_v<To, char16_t>)
		return Impl::ToU16String(from);
	else if (std::is_same_v<To, char32_t>)
		return Impl::ToU32String(from);
	else
		static_assert(false, "Unknown type!");
}
