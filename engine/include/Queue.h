#pragma once

#include <queue>
#include "Deque.h"
#include "Array.h"

namespace BE
{
	/**
	 * @brief Templated queue.
	 * @todo Direct implementation.
	*/
	template <class T>
	using Queue = std::queue<T, Deque<T>>;

	/**
	 * @brief Templated priority queue.
	 * @todo Direct implementation.
	 * @warning Value type supports less or <, or requires custom comparator.
	*/
	template <class T, template<class>class Comp = std::less>
	using PriorityQueue = std::priority_queue<T, Array<T>, Comp<T>>;
}