#pragma once

#include <map>
#include <unordered_map>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using TreeMap = std::map<T, Alloc<T>>;

template <class T, template<class>class Alloc = Allocator>
using HashMap = std::unordered_map<T, Alloc<T>>;