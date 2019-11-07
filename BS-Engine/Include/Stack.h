#pragma once

#include <stack>
#include "Deque.h"
#include "Allocator.h"

/**
 * @brief Templated stack.
 * @todo Direct implementation.
*/
template <class T>
using Stack = std::stack<T, Deque<T>>;