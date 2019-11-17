#pragma once

#include <list>
#include <forward_list>
#include "Allocator.h"

namespace BE
{
	/**
	 * @brief Templated doubly linked list.
	 * @todo Direct implementation.
	*/
	template <class T>
	using List = std::list<T, Allocator<T>>;
}