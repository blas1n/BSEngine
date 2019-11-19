#pragma once

#include "Type.h"
#include "Macro.h"
#include <wctype.h>

namespace BE
{
	inline bool IsAlnum(Char ch) { return iswalnum(ch) != 0; }
	inline bool IsAlpha(Char ch) { return iswalpha(ch) != 0; }

	inline bool IsUpper(Char ch) { return ::iswupper(ch) != 0; }
	inline bool IsLower(Char ch) { return ::iswlower(ch) != 0; }

	inline bool IsDigit(Char ch) { return ::iswdigit(ch) != 0; }
	inline bool IsHexDigit(Char ch) { return ::iswxdigit(ch) != 0; }

	inline bool IsGraph(Char ch) { return ::iswgraph(ch) != 0; }
	inline bool IsSpace(Char ch) { return ::iswspace(ch) != 0; }
	inline bool IsBlack(Char ch) { return ::iswblank(ch) != 0; }

	inline bool IsPrint(Char ch) { return ::iswprint(ch) != 0; }
	inline bool IsPunct(Char ch) { return ::iswpunct(ch) != 0; }

	inline Char ToUpper(Char ch) { return ::towupper(ch); }
	inline Char ToLower(Char ch) { return ::towlower(ch); }
	
	inline Int32 ConvertCharDigitToInt(Char ch)
	{
		return static_cast<Int32>(ch) - static_cast<Int32>('0');
	}

	inline bool IsIdentifier(Char ch)
	{
		return IsAlnum(ch) || ch == TEXT('_');
	}
};