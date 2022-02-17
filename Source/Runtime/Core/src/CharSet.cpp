#include "CharSet.h"
#include "utf8cpp/utf8.h"

namespace Impl
{
	std::string ToString(std::wstring_view from)
	{
		std::string to;

		if constexpr (sizeof(wchar_t) == 1)
			to = std::string(from.cbegin(), from.cend());
		else if (sizeof(wchar_t) == 2)
			utf8::utf16to8(from.cbegin(), from.cend(), std::back_inserter(to));
		else if (sizeof(wchar_t) == 4)
			utf8::utf32to8(from.cbegin(), from.cend(), std::back_inserter(to));
		
		return to;
	}

	std::string ToString(std::u16string_view from)
	{
		std::string to;
		utf8::utf16to8(from.cbegin(), from.cend(), std::back_inserter(to));
		return to;
	}

	std::string ToString(std::u32string_view from)
	{
		std::string to;
		utf8::utf32to8(from.cbegin(), from.cend(), std::back_inserter(to));
		return to;
	}

	std::wstring ToWString(std::string_view from)
	{
		std::wstring to;

		if constexpr (sizeof(wchar_t) == 1)
			to = std::wstring(from.cbegin(), from.cend());
		else if (sizeof(wchar_t) == 2)
			utf8::utf8to16(from.cbegin(), from.cend(), std::back_inserter(to));
		else if (sizeof(wchar_t) == 4)
			utf8::utf8to32(from.cbegin(), from.cend(), std::back_inserter(to));

		return to;
	}

	std::wstring ToWString(std::u16string_view from)
	{
		if constexpr (sizeof(wchar_t) == sizeof(char16_t))
		{
			return std::wstring(from.cbegin(), from.cend());
		}
		else
		{
			std::wstring to;
			utf8::utf16to8(from.cbegin(), from.cend(), std::back_inserter(to));
			return to;
		}
	}

	std::wstring ToWString(std::u32string_view from)
	{
		if constexpr (sizeof(wchar_t) == sizeof(char))
		{
			std::wstring to;
			utf8::utf32to8(from.cbegin(), from.cend(), std::back_inserter(to));
			return to;
		}
		else
			return std::wstring{};
	}

	std::u16string ToU16String(std::string_view from)
	{
		std::u16string to;
		utf8::utf8to16(from.cbegin(), from.cend(), std::back_inserter(to));
		return to;
	}

	std::u16string ToU16String(std::wstring_view from)
	{
		if constexpr (sizeof(wchar_t) == sizeof(char16_t))
		{
			return std::u16string(from.cbegin(), from.cend());
		}
		else
		{
			std::u16string to;
			utf8::utf8to16(from.cbegin(), from.cend(), std::back_inserter(to));
			return to;
		}
	}

	std::u32string ToU32String(std::string_view from)
	{
		std::u32string to;
		utf8::utf8to32(from.cbegin(), from.cend(), std::back_inserter(to));
		return to;
	}

	std::u32string ToU32String(std::wstring_view from)
	{
		if constexpr (sizeof(wchar_t) == sizeof(char))
		{
			std::u32string to;
			utf8::utf8to32(from.cbegin(), from.cend(), std::back_inserter(to));
			return to;
		}
		else
			return std::u32string{};
	}
}