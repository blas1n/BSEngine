#pragma once

#include "Allocator.h"
#include <string>

namespace BE
{
	/**
	 * @brief Operable string class.
	 * @todo Direct implementation.
	*/
	using String =
		std::basic_string<char, std::char_traits<char>, Allocator<char>>;
}