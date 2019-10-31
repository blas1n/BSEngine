#pragma once

#include <queue>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using Queue = std::queue<T, std::deque<T, Alloc<T>>>;

template <class T, template<class>class Alloc = PoolAllocator>
using PriorityQueue = std::priority_queue<T, std::vector<T, Alloc<T>>>;