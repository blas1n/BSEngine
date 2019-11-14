#pragma once

#include <list>
#include <forward_list>
#include "Allocator.h"

/**
 * @brief Templated doubly linked list.
 * @todo Direct implementation.
*/
template <class T>
using List = std::list<T, Allocator<T>>;


/**
 * @brief Templated singly linked list.
 * @todo Direct implementation.
*/
template <class T>
using ForwardList = std::forward_list<T, Allocator<T>>;