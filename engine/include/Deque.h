#pragma once

#include <deque>
#include "Allocator.h"

namespace BE
{
	/**
	 * @brief Templated deque.
	 * @todo Direct implementation.
	*/
	template <class T>
	using Deque = std::deque<T, Allocator<T>>;
}