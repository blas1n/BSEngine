#pragma once

#include <queue>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using Queue = std::queue<T, std::deque<T, Alloc<T>>>;

template <class T, template<class>class Alloc = Allocator>
using PriorityQueue = std::priority_queue<T, std::vector<T, Alloc<T>>>;