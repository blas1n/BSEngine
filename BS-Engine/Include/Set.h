#pragma once

#include <set>
#include <unordered_set>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using TreeSet = std::set<T, Alloc<T>>;

template <class T, template<class>class Alloc = PoolAllocator>
using HashSet = std::unordered_set<T, Alloc<T>>;