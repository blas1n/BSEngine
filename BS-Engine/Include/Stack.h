#pragma once

#include <stack>
#include "Allocator.h"

template <class T, template<class>class Alloc = Allocator>
using Stack = std::stack<T, std::deque<T, Alloc<T>>>;