#pragma once

#include <map>
#include <unordered_map>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using TreeMap = std::map<T, Alloc<T>>;

template <class T, template<class>class Alloc = PoolAllocator>
using HashMap = std::unordered_map<T, Alloc<T>>;