#pragma once

#include <cstddef>
#include <wchar.h>

namespace BE
{
	using Uint8 = unsigned char;
	using Uint16 = unsigned short;
	using Uint32 = unsigned int;
	using Uint64 = unsigned long long;

	using Int8 = signed int;
	using Int16 = signed short;
	using Int32 = signed int;
	using Int64 = signed long long;

	using Char = wchar_t;

	using SizeType = std::size_t;
}