#pragma once

#include <stack>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using Stack = std::stack<T, std::deque<T, Alloc<T>>>;