#pragma once

#include <set>
#include <unordered_set>
#include "Allocator.h"

/**
 * @brief Templated set using tree.
 * @detail Data is always sorted, but slightly slow.
 * @todo Direct implementation.
 * @warning Value type supports less or <, or requires custom comparator.
*/
template <class T, template<class>class Comp = std::less>
using TreeSet = std::set<T, Comp<T>, Allocator<T>>;

/**
 * @brief Templated set using hash.
 * @detail Very fast, but the data is not sorted.
 * @todo Direct implementation.
 * @warning Key type supports std::hash. And supports std::equal_to or ==.
*/
template <class T, template<class>class Comp = std::less>
using HashSet = std::unordered_set<T, std::hash<T>, std::equal_to<T>, Allocator<T>>;