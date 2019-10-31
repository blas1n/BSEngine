#pragma once

#include <vector>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using Array = std::vector<T, Alloc<T>>;