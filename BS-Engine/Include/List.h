#pragma once

#include <list>
#include <forward_list>
#include "PoolAllocator.h"

template <class T, template<class>class Alloc = PoolAllocator>
using List = std::list<T, Alloc<T>>;

template <class T, template<class>class Alloc = PoolAllocator>
using ForwardList = std::forward_list<T, Alloc<T>>;