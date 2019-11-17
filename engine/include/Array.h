#pragma once

#include <vector>
#include "Allocator.h"

namespace BE
{
	/**
	 * @brief Templated dynamic array.
	 * @todo Direct implementation.
	*/
	template <class T>
	using Array = std::vector<T, Allocator<T>>;
}