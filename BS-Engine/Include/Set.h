#pragma once

#include <set>
#include <unordered_set>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using TreeSet = std::set<T, Alloc<T>>;

template <class T, template<class>class Alloc = Allocator>
using HashSet = std::unordered_set<T, Alloc<T>>;