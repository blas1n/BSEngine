#pragma once

#include "Core.h"
#include <wctype.h>

namespace BE
{
	inline bool IsAlnum(Char ch) noexcept { return iswalnum(ch) != 0; }
	inline bool IsAlpha(Char ch) noexcept { return iswalpha(ch) != 0; }

	inline bool IsUpper(Char ch) noexcept { return ::iswupper(ch) != 0; }
	inline bool IsLower(Char ch) noexcept { return ::iswlower(ch) != 0; }

	inline bool IsDigit(Char ch) noexcept { return ::iswdigit(ch) != 0; }
	inline bool IsHexDigit(Char ch) noexcept { return ::iswxdigit(ch) != 0; }

	inline bool IsGraph(Char ch) noexcept { return ::iswgraph(ch) != 0; }
	inline bool IsSpace(Char ch) noexcept { return ::iswspace(ch) != 0; }
	inline bool IsBlack(Char ch) noexcept { return ::iswblank(ch) != 0; }

	inline bool IsPrint(Char ch) noexcept { return ::iswprint(ch) != 0; }
	inline bool IsPunct(Char ch) noexcept { return ::iswpunct(ch) != 0; }

	inline Char ToUpper(Char ch) noexcept { return ::towupper(ch); }
	inline Char ToLower(Char ch) noexcept { return ::towlower(ch); }
	
	constexpr inline Int32 ConvertCharDigitToInt(Char ch) noexcept
	{
		return static_cast<Int32>(ch) - static_cast<Int32>('0');
	}

	inline bool IsIdentifier(Char ch) noexcept
	{
		return IsAlnum(ch) || ch == TEXT('_');
	}
};