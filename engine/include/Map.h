#pragma once

#include <map>
#include <unordered_map>
#include "Allocator.h"

namespace BE
{
	/**
	 * @brief Templated map using tree.
	 * @detail Data is always sorted, but slightly slow.
	 * @todo Direct implementation.
	 * @warning Key type supports less or <, or requires custom comparator.
	*/
	template <class K, class T, template<class>class Comp = std::less>
	using TreeMap = std::map<K, T, Comp<K>, Allocator<std::pair<const K, T>>>;

	/**
	 * @brief Templated map using hash.
	 * @detail Very fast, but the data is not sorted.
	 * @todo Direct implementation.
	 * @warning Key type supports std::hash. And supports std::equal_to or ==.
	*/
	template <class K, class T>
	using HashMap = std::unordered_map<K, T, std::hash<K>,
		std::equal_to<K>, Allocator<std::pair<const K, T>>>;
}